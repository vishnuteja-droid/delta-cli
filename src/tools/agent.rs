//! The tool loop: a multi-turn conversation between a `Provider` and the
//! gated tool executor (`tools::execute`), driving `dlt build`.
//!
//! **Tool-call protocol.** `PLAN.md` names no new provider dependency for
//! this prompt, and `dlt` explicitly targets arbitrary `openai_compatible`
//! backends — "most local servers" — many of which don't implement
//! either vendor's native function-calling wire format. Rather than add
//! streaming tool-call parsing to both `provider/anthropic.rs` and
//! `provider/openai_compatible.rs` (accumulating partial JSON argument
//! deltas keyed by content-block index or tool-call index — a real
//! protocol difference between the two), the model is instead asked to
//! emit a tool call as a fenced code block inside its ordinary streamed
//! text:
//!
//! ```text
//! ```tool_call
//! {"tool": "read_file", "input": {"path": "src/main.rs"}}
//! ```
//! ```
//!
//! and to respond with plain prose (no such block) once it has a final
//! answer. This works identically against every backend `Provider`
//! already supports, with zero changes to `provider.rs`'s request/delta
//! types — `Request`/`Message`/`Role`/`Delta` are exactly what prompt 3
//! left them. Documented as a deliberate scope decision in `PROGRESS.md`.

use std::path::Path;

use futures::StreamExt;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::AgentError;
use crate::provider::{Message, Provider, Request, Role};
use crate::tools::{self, Approver, ToolCall, ToolOutcome, ToolSpec};
use crate::workspace::Store;

/// Hard cap on tool-call round-trips before the loop stops and reports
/// rather than continuing indefinitely.
pub const DEFAULT_MAX_ITERATIONS: u32 = 20;

/// Tokens reserved for the model's own output each turn, subtracted
/// from `provider.context_window()` to get the running conversation's
/// budget — mirrors `stage::context::assemble`'s reserve.
const RESERVED_OUTPUT_TOKENS: u32 = 4_096;
const TURN_MAX_TOKENS: u32 = 4_096;

/// Observes the loop's progress without owning any of its decisions —
/// `cli.rs`'s `dlt build` implements this to print to stdout/stderr as
/// it goes; tests use a no-op implementation via the trait's defaults.
/// Keeps `tools::agent` itself UI-agnostic, the same seam
/// `verify::watch_and_rerun`'s callback established in prompt 4.
pub trait AgentObserver {
    fn on_text_delta(&mut self, _text: &str) {}
    fn on_reasoning_delta(&mut self, _text: &str) {}
    fn on_tool_call(&mut self, _call: &ToolCall) {}
    fn on_tool_result(&mut self, _outcome: &ToolOutcome) {}
}

#[derive(Debug)]
pub struct AgentOutcome {
    /// Not read anywhere outside tests today — `cmd_build` already
    /// streamed this text live via `on_text_delta` as it arrived, so
    /// re-printing it would just duplicate stdout. Kept on the outcome
    /// (rather than dropped) because it's the natural place a future
    /// caller — a non-streaming caller, or prompt 6's TUI — would want
    /// the final answer without re-deriving it from observer callbacks.
    #[allow(dead_code)]
    pub final_answer: String,
    pub iterations: u32,
}

/// Drive the loop: assemble a request, stream the model's turn, look
/// for a `tool_call` block in the accumulated text, execute it and feed
/// the result back as the next turn — until the model responds with
/// plain text (no tool call), the iteration cap is hit, or the running
/// conversation would exceed the token budget. The last two stop and
/// report via `AgentError`, never by silently truncating the
/// conversation — the literal requirement from `PLAN.md`. `cancel` is
/// handed to every `provider.stream(...)` call unchanged (cloned per
/// turn) — the caller owns cancelling it, e.g. `dlt tui build`'s `esc`
/// handler, or a fresh unused token for the plain-CLI path that never
/// interrupts.
#[allow(clippy::too_many_arguments)]
pub async fn run_loop(
    provider: &impl Provider,
    system_prompt: String,
    initial_user_message: String,
    repo_root: &Path,
    store: &dyn Store,
    config: &Config,
    approver: &dyn Approver,
    max_iterations: u32,
    observer: &mut dyn AgentObserver,
    cancel: CancellationToken,
) -> Result<AgentOutcome, AgentError> {
    let budget = provider
        .context_window()
        .saturating_sub(RESERVED_OUTPUT_TOKENS);
    let mut messages = vec![Message {
        role: Role::User,
        content: initial_user_message,
    }];

    for iteration in 1..=max_iterations {
        let tokens = provider.count_tokens(&system_prompt)
            + messages
                .iter()
                .map(|m| provider.count_tokens(&m.content))
                .sum::<u32>();
        if tokens > budget {
            return Err(AgentError::TokenBudgetExceeded { tokens, budget });
        }

        let request = Request {
            system: Some(system_prompt.clone()),
            messages: messages.clone(),
            max_tokens: TURN_MAX_TOKENS,
        };
        let mut stream = provider.stream(request, cancel.clone()).await?;
        let mut text = String::new();
        while let Some(delta) = stream.next().await {
            // Reasoning never enters the conversation transcript — it
            // would otherwise pollute both the tool_call parse and the
            // Assistant turn replayed back to the model next iteration.
            match delta? {
                crate::provider::Delta::Text(chunk) => {
                    observer.on_text_delta(&chunk);
                    text.push_str(&chunk);
                }
                crate::provider::Delta::Reasoning(chunk) => {
                    observer.on_reasoning_delta(&chunk);
                }
            }
        }
        messages.push(Message {
            role: Role::Assistant,
            content: text.clone(),
        });

        match parse_tool_call(&text) {
            None => {
                return Ok(AgentOutcome {
                    final_answer: text,
                    iterations: iteration,
                });
            }
            Some(Ok(call)) => {
                observer.on_tool_call(&call);
                let outcome = tools::execute(&call, repo_root, store, config, approver)?;
                observer.on_tool_result(&outcome);
                messages.push(Message {
                    role: Role::User,
                    content: format_tool_result(&call, &outcome),
                });
            }
            Some(Err(reason)) => {
                messages.push(Message {
                    role: Role::User,
                    content: format!(
                        "Your tool_call block could not be parsed: {reason}. \
                         Retry with a single valid JSON object: \
                         {{\"tool\": \"<name>\", \"input\": {{ ... }}}}."
                    ),
                });
            }
        }
    }
    Err(AgentError::IterationCapReached {
        cap: max_iterations,
    })
}

fn format_tool_result(call: &ToolCall, outcome: &ToolOutcome) -> String {
    format!(
        "Tool `{}` {}:\n{}",
        call.tool,
        if outcome.success {
            "succeeded"
        } else {
            "failed"
        },
        outcome.output
    )
}

/// The system prompt: optional `AGENTS.md` content, the tool catalog,
/// and the `tool_call` protocol description.
pub fn build_system_prompt(agents_md: &str) -> String {
    let mut prompt = String::new();
    if !agents_md.is_empty() {
        prompt.push_str(agents_md);
        prompt.push_str("\n\n");
    }
    prompt.push_str(
        "You are an autonomous coding agent with tools that act on this repository.\n\n\
         Available tools:\n",
    );
    for spec in tools::TOOL_SPECS {
        prompt.push_str(&tool_spec_line(spec));
    }
    prompt.push_str(
        "\nTo call a tool, respond with ONLY a fenced code block tagged `tool_call` \
         containing a single JSON object: {\"tool\": \"<name>\", \"input\": { ... }}. \
         Do not include any other text in a response that calls a tool.\n\
         When you are done and have no more tools to call, respond with your final \
         answer as plain text with no tool_call block.\n",
    );
    prompt
}

fn tool_spec_line(spec: &ToolSpec) -> String {
    format!(
        "- {}: {} Input: {}\n",
        spec.name, spec.description, spec.input_shape
    )
}

/// `None` means the text is a final answer (no tool call found).
/// `Some(Err(reason))` means a `tool_call` block was present but
/// couldn't be parsed — fed back to the model rather than crashing the
/// loop, the same "malformed input becomes a visible failure, not a
/// silent drop" pattern `verify.rs`'s check parser uses.
fn parse_tool_call(text: &str) -> Option<Result<ToolCall, String>> {
    const MARKER: &str = "```tool_call";
    let start = text.find(MARKER)?;
    let after_marker = &text[start + MARKER.len()..];
    let body_start = after_marker.find('\n').map_or(0, |i| i + 1);
    let body = &after_marker[body_start..];
    let json_text = match body.find("```") {
        Some(end) => &body[..end],
        None => body,
    };
    Some(parse_tool_call_json(json_text.trim()))
}

fn parse_tool_call_json(json_text: &str) -> Result<ToolCall, String> {
    let value: Value = serde_json::from_str(json_text).map_err(|e| format!("invalid JSON: {e}"))?;
    let tool = value
        .get("tool")
        .and_then(Value::as_str)
        .ok_or("missing string field \"tool\"")?
        .to_string();
    let input = value
        .get("input")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    Ok(ToolCall { tool, input })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Delta;
    use crate::workspace::{FsStore, JOURNAL_DIR};
    use futures::stream::BoxStream;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tempfile::TempDir;

    struct FakeProvider {
        responses: Mutex<VecDeque<&'static str>>,
        context_window: u32,
    }

    impl FakeProvider {
        fn scripted(responses: &[&'static str]) -> Self {
            FakeProvider {
                responses: Mutex::new(responses.iter().copied().collect()),
                context_window: 100_000,
            }
        }
    }

    impl Provider for FakeProvider {
        fn name(&self) -> &str {
            "fake"
        }

        fn context_window(&self) -> u32 {
            self.context_window
        }

        fn count_tokens(&self, text: &str) -> u32 {
            text.len() as u32
        }

        async fn stream(
            &self,
            _request: Request,
            cancel: CancellationToken,
        ) -> Result<
            BoxStream<'static, Result<Delta, crate::error::ProviderError>>,
            crate::error::ProviderError,
        > {
            if cancel.is_cancelled() {
                return Err(crate::error::ProviderError::Cancelled {
                    name: "fake".to_string(),
                });
            }
            let text = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or("")
                .to_string();
            Ok(Box::pin(futures::stream::iter(vec![Ok(Delta::Text(text))])))
        }
    }

    struct NullObserver;
    impl AgentObserver for NullObserver {}

    struct CapturingObserver {
        tool_calls: Vec<String>,
    }
    impl AgentObserver for CapturingObserver {
        fn on_tool_call(&mut self, call: &ToolCall) {
            self.tool_calls.push(call.tool.clone());
        }
    }

    fn setup() -> (TempDir, FsStore, Config) {
        let dir = TempDir::new().unwrap();
        let store_root = dir.path().join(".delta");
        std::fs::create_dir_all(store_root.join(JOURNAL_DIR)).unwrap();
        let config = Config::load(dir.path()).unwrap();
        (dir, FsStore::new(store_root), config)
    }

    struct AlwaysApprove;
    impl Approver for AlwaysApprove {
        fn approve(&self, _tool: &str, _preview: &str) -> bool {
            true
        }
    }

    #[test]
    fn parse_tool_call_returns_none_for_plain_text() {
        assert!(parse_tool_call("Here is my final answer, no tools needed.").is_none());
    }

    #[test]
    fn parse_tool_call_extracts_a_well_formed_block() {
        let text = "I'll read the file.\n```tool_call\n{\"tool\": \"read_file\", \"input\": {\"path\": \"a.txt\"}}\n```\n";
        let call = parse_tool_call(text).unwrap().unwrap();
        assert_eq!(call.tool, "read_file");
        assert_eq!(call.input["path"], "a.txt");
    }

    #[test]
    fn parse_tool_call_reports_malformed_json_without_panicking() {
        let text = "```tool_call\nnot json\n```\n";
        let err = parse_tool_call(text).unwrap().unwrap_err();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn build_system_prompt_lists_every_tool() {
        let prompt = build_system_prompt("");
        for spec in tools::TOOL_SPECS {
            assert!(prompt.contains(spec.name), "missing {}", spec.name);
        }
        assert!(prompt.contains("tool_call"));
    }

    #[tokio::test]
    async fn final_text_with_no_tool_call_ends_the_loop() {
        let (dir, store, config) = setup();
        let provider = FakeProvider::scripted(&["All done, nothing to do."]);
        let mut observer = NullObserver;
        let outcome = run_loop(
            &provider,
            build_system_prompt(""),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            DEFAULT_MAX_ITERATIONS,
            &mut observer,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(outcome.final_answer, "All done, nothing to do.");
        assert_eq!(outcome.iterations, 1);
    }

    #[tokio::test]
    async fn executes_a_tool_call_and_continues_to_a_final_answer() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let provider = FakeProvider::scripted(&[
            "```tool_call\n{\"tool\": \"read_file\", \"input\": {\"path\": \"a.txt\"}}\n```",
            "Read the file, it says hello. Done.",
        ]);
        let mut observer = CapturingObserver {
            tool_calls: Vec::new(),
        };
        let outcome = run_loop(
            &provider,
            build_system_prompt(""),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            DEFAULT_MAX_ITERATIONS,
            &mut observer,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(outcome.iterations, 2);
        assert_eq!(observer.tool_calls, vec!["read_file".to_string()]);
        assert!(outcome.final_answer.contains("Done"));
    }

    #[tokio::test]
    async fn iteration_cap_is_reported_not_looped_forever() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        // Always calls a tool, never gives a final answer.
        let script: Vec<&'static str> = std::iter::repeat_n(
            "```tool_call\n{\"tool\": \"read_file\", \"input\": {\"path\": \"a.txt\"}}\n```",
            5,
        )
        .collect();
        let provider = FakeProvider::scripted(&script);
        let mut observer = NullObserver;
        let err = run_loop(
            &provider,
            build_system_prompt(""),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            3,
            &mut observer,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AgentError::IterationCapReached { cap: 3 }));
    }

    #[tokio::test]
    async fn token_budget_exceeded_stops_before_calling_the_provider() {
        let (dir, store, config) = setup();
        let provider = FakeProvider {
            responses: Mutex::new(VecDeque::new()),
            context_window: 10, // any non-trivial prompt already exceeds this
        };
        let mut observer = NullObserver;
        let err = run_loop(
            &provider,
            build_system_prompt("some agents.md content"),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            DEFAULT_MAX_ITERATIONS,
            &mut observer,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AgentError::TokenBudgetExceeded { .. }));
    }

    #[tokio::test]
    async fn malformed_tool_call_is_fed_back_instead_of_aborting() {
        let (dir, store, config) = setup();
        let provider = FakeProvider::scripted(&[
            "```tool_call\nnot valid json\n```",
            "Sorry, let me just answer directly instead.",
        ]);
        let mut observer = NullObserver;
        let outcome = run_loop(
            &provider,
            build_system_prompt(""),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            DEFAULT_MAX_ITERATIONS,
            &mut observer,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(outcome.iterations, 2);
    }

    /// The caller's `cancel` token must actually reach every
    /// `provider.stream(...)` call, not a fresh internal one the loop
    /// creates for itself — otherwise nothing (e.g. `dlt tui build`'s
    /// `esc` handler) could ever interrupt an in-flight turn.
    #[tokio::test]
    async fn callers_cancellation_token_reaches_the_provider() {
        let (dir, store, config) = setup();
        let provider = FakeProvider::scripted(&["irrelevant, cancelled before use"]);
        let mut observer = NullObserver;
        let cancel = CancellationToken::new();
        cancel.cancel();

        let err = run_loop(
            &provider,
            build_system_prompt(""),
            "implement the change".to_string(),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
            DEFAULT_MAX_ITERATIONS,
            &mut observer,
            cancel,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            AgentError::Provider(crate::error::ProviderError::Cancelled { .. })
        ));
    }
}
