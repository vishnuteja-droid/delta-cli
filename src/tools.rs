//! Gated tool execution for the agent loop (`tools::agent`): the six
//! tools `PLAN.md`'s prompt 5 asks for — `read_file`, `write_file`,
//! `apply_patch`, `list_dir`, `search`, `run_command` — each behind a
//! configurable approval policy (`auto`/`prompt`/`deny`, set per tool
//! via `[tools.<name>] policy = "..."` in config; writes and commands
//! default to `prompt`, read-only tools default to `auto`, matching
//! PLAN.md's "writes and commands default to prompt"). Writes are
//! journalled (`tools::journal`) so `dlt undo` reverts the most recent
//! one. `run_command` never runs through a shell and refuses outright
//! for any program not on its allowlist — checked before the approval
//! gate, so an unlisted program is never even prompted for.

pub mod agent;
pub mod apply_patch;
pub mod journal;
pub mod search;

use std::io::Write as _;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use chrono::Utc;
use serde_json::Value;

use crate::config::Config;
use crate::error::ToolError;
use crate::workspace::Store;

/// A tool invocation as the model expresses it: a name plus a JSON
/// input object — the `{"tool": "...", "input": {...}}` shape
/// `tools::agent`'s protocol parses out of the model's response.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool: String,
    pub input: Value,
}

/// The result fed back into the conversation. `success` is whether the
/// *tool* considers its own operation to have succeeded — a denied
/// approval, a missing file, or a non-allowlisted command are all
/// `success: false` with an explanatory `output`, not an `Err`: the
/// agent loop always gets something concrete to react to and keep
/// going, rather than the whole build aborting on a recoverable refusal.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub success: bool,
    pub output: String,
}

impl ToolOutcome {
    fn ok(output: impl Into<String>) -> Self {
        ToolOutcome {
            success: true,
            output: output.into(),
        }
    }

    fn fail(output: impl Into<String>) -> Self {
        ToolOutcome {
            success: false,
            output: output.into(),
        }
    }
}

/// Static description of one of the six tools, embedded in the agent
/// loop's system prompt so the model knows what's available and the
/// input shape each expects.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_shape: &'static str,
}

pub const TOOL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        name: "read_file",
        description: "Read a file's contents.",
        input_shape: r#"{"path": "relative/path"}"#,
    },
    ToolSpec {
        name: "write_file",
        description: "Create or overwrite a file with new content.",
        input_shape: r#"{"path": "relative/path", "content": "..."}"#,
    },
    ToolSpec {
        name: "apply_patch",
        description: "Apply a unified-diff patch to an existing file. \
            The diff body may start directly with @@ hunks, or include \
            --- a/... / +++ b/... headers (ignored — \"path\" is what's \
            actually targeted).",
        input_shape: r#"{"path": "relative/path", "diff": "@@ -1,1 +1,1 @@\n-old\n+new\n"}"#,
    },
    ToolSpec {
        name: "list_dir",
        description: "List the immediate entries of a directory (\".\" if path omitted).",
        input_shape: r#"{"path": "relative/path"}"#,
    },
    ToolSpec {
        name: "search",
        description: "Regex search across the repository tree (respects .gitignore).",
        input_shape: r#"{"pattern": "regex", "path": "relative/path (optional, default \".\")"}"#,
    },
    ToolSpec {
        name: "run_command",
        description: "Run a program (no shell) with arguments. The program \
            must be on the configured [tools.run_command] allowlist.",
        input_shape: r#"{"program": "cargo", "args": ["test"]}"#,
    },
];

/// `auto` runs without asking, `prompt` shows a preview and asks,
/// `deny` refuses outright without prompting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    Auto,
    Prompt,
    Deny,
}

/// Decides whether a `Prompt`-gated tool call proceeds. Abstracted so
/// the loop is testable without real stdin/stdout interaction — the
/// real CLI wires `StdinApprover`.
pub trait Approver {
    /// `preview` is the diff (for writes/patches) or the exact command
    /// line (for `run_command`) the caller should show before asking.
    fn approve(&self, tool: &str, preview: &str) -> bool;
}

/// Prints `preview` to stderr and asks on stdin — the real, interactive
/// approver `dlt build` uses.
pub struct StdinApprover;

impl Approver for StdinApprover {
    fn approve(&self, tool: &str, preview: &str) -> bool {
        eprintln!("{preview}");
        eprint!("Approve '{tool}'? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    }
}

fn policy_for(config: &Config, tool: &str) -> Approval {
    match config.get_str(&format!("tools.{tool}.policy")) {
        Some("auto") => Approval::Auto,
        Some("prompt") => Approval::Prompt,
        Some("deny") => Approval::Deny,
        _ => default_policy(tool),
    }
}

fn default_policy(tool: &str) -> Approval {
    match tool {
        "read_file" | "list_dir" | "search" => Approval::Auto,
        _ => Approval::Prompt, // write_file, apply_patch, run_command
    }
}

fn run_command_allowlist(config: &Config) -> Vec<String> {
    config
        .get("tools.run_command.allowlist")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// `None` if the call should proceed; `Some(outcome)` (always a failing
/// one) if it was denied or declined and execution should stop here.
fn gate(
    tool: &str,
    preview: &str,
    config: &Config,
    approver: &dyn Approver,
) -> Option<ToolOutcome> {
    match policy_for(config, tool) {
        Approval::Auto => None,
        Approval::Deny => Some(ToolOutcome::fail(format!(
            "tool {tool:?} is denied by policy"
        ))),
        Approval::Prompt => {
            if approver.approve(tool, preview) {
                None
            } else {
                Some(ToolOutcome::fail(format!("tool {tool:?} was not approved")))
            }
        }
    }
}

fn diff_preview(path: &str, old: &str, new: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .header(path, path)
        .to_string()
}

fn str_field<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or non-string field {key:?}"))
}

/// Run one tool call end to end: resolve its approval policy, ask if
/// needed, execute, and journal the mutation if it's a write. `Err` is
/// reserved for failures the loop can't recover from (I/O reaching the
/// journal itself); everything else — bad input, a denied approval, a
/// file that doesn't exist — comes back as a failing `ToolOutcome` so
/// the model can see what happened and adapt.
pub fn execute(
    call: &ToolCall,
    repo_root: &Path,
    store: &dyn Store,
    config: &Config,
    approver: &dyn Approver,
) -> Result<ToolOutcome, ToolError> {
    match call.tool.as_str() {
        "read_file" => Ok(read_file(repo_root, &call.input)),
        "list_dir" => Ok(list_dir(repo_root, &call.input)),
        "search" => Ok(run_search(repo_root, &call.input)),
        "write_file" => write_file(&call.input, repo_root, store, config, approver),
        "apply_patch" => run_apply_patch(&call.input, repo_root, store, config, approver),
        "run_command" => Ok(run_command(&call.input, repo_root, config, approver)),
        other => Ok(ToolOutcome::fail(format!("unknown tool {other:?}"))),
    }
}

/// Files larger than this are truncated in the tool result rather than
/// blown whole into the conversation — this is a per-call safety cap,
/// distinct from (and in addition to) the loop-level token budget.
const MAX_READ_BYTES: usize = 200_000;

fn read_file(repo_root: &Path, input: &Value) -> ToolOutcome {
    let path = match str_field(input, "path") {
        Ok(p) => p,
        Err(e) => return ToolOutcome::fail(e),
    };
    match std::fs::read_to_string(repo_root.join(path)) {
        Ok(content) if content.len() > MAX_READ_BYTES => {
            let truncated: String = content.chars().take(MAX_READ_BYTES).collect();
            ToolOutcome::ok(format!(
                "{truncated}\n\n[truncated: file is {} bytes, showing the first {MAX_READ_BYTES}]",
                content.len()
            ))
        }
        Ok(content) => ToolOutcome::ok(content),
        Err(e) => ToolOutcome::fail(format!("failed to read {path}: {e}")),
    }
}

fn list_dir(repo_root: &Path, input: &Value) -> ToolOutcome {
    let path = input.get("path").and_then(Value::as_str).unwrap_or(".");
    let entries = match std::fs::read_dir(repo_root.join(path)) {
        Ok(e) => e,
        Err(e) => return ToolOutcome::fail(format!("failed to list {path}: {e}")),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => return ToolOutcome::fail(format!("failed to list {path}: {e}")),
        };
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let name = entry.file_name().to_string_lossy().into_owned();
        names.push(if is_dir { format!("{name}/") } else { name });
    }
    names.sort();
    ToolOutcome::ok(if names.is_empty() {
        "(empty)".to_string()
    } else {
        names.join("\n")
    })
}

fn run_search(repo_root: &Path, input: &Value) -> ToolOutcome {
    let pattern = match str_field(input, "pattern") {
        Ok(p) => p,
        Err(e) => return ToolOutcome::fail(e),
    };
    let dir = input.get("path").and_then(Value::as_str).unwrap_or(".");
    match search::search(repo_root, dir, pattern) {
        Ok(matches) if matches.is_empty() => ToolOutcome::ok("no matches"),
        Ok(matches) => ToolOutcome::ok(
            matches
                .iter()
                .map(|m| format!("{}:{}: {}", m.path, m.line_number, m.line))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Err(e) => ToolOutcome::fail(e.to_string()),
    }
}

fn write_file(
    input: &Value,
    repo_root: &Path,
    store: &dyn Store,
    config: &Config,
    approver: &dyn Approver,
) -> Result<ToolOutcome, ToolError> {
    let path = match str_field(input, "path") {
        Ok(p) => p,
        Err(e) => return Ok(ToolOutcome::fail(e)),
    };
    let content = match str_field(input, "content") {
        Ok(c) => c,
        Err(e) => return Ok(ToolOutcome::fail(e)),
    };

    let full = repo_root.join(path);
    let previous = std::fs::read_to_string(&full).ok();

    let preview = diff_preview(path, previous.as_deref().unwrap_or(""), content);
    if let Some(outcome) = gate("write_file", &preview, config, approver) {
        return Ok(outcome);
    }

    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ToolError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&full, content).map_err(|source| ToolError::Io {
        path: full.display().to_string(),
        source,
    })?;
    journal::record(store, "write_file", path, previous, Utc::now())?;
    Ok(ToolOutcome::ok(format!("wrote {path}")))
}

fn run_apply_patch(
    input: &Value,
    repo_root: &Path,
    store: &dyn Store,
    config: &Config,
    approver: &dyn Approver,
) -> Result<ToolOutcome, ToolError> {
    let path = match str_field(input, "path") {
        Ok(p) => p,
        Err(e) => return Ok(ToolOutcome::fail(e)),
    };
    let diff_text = match str_field(input, "diff") {
        Ok(d) => d,
        Err(e) => return Ok(ToolOutcome::fail(e)),
    };

    let full = repo_root.join(path);
    let previous = std::fs::read_to_string(&full).ok();

    let new_content = match apply_patch::apply(previous.as_deref(), diff_text) {
        Ok(c) => c,
        Err(e) => return Ok(ToolOutcome::fail(e.to_string())),
    };

    let preview = diff_preview(path, previous.as_deref().unwrap_or(""), &new_content);
    if let Some(outcome) = gate("apply_patch", &preview, config, approver) {
        return Ok(outcome);
    }

    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ToolError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&full, &new_content).map_err(|source| ToolError::Io {
        path: full.display().to_string(),
        source,
    })?;
    journal::record(store, "apply_patch", path, previous, Utc::now())?;
    Ok(ToolOutcome::ok(format!("patched {path}")))
}

/// Default timeout for `run_command`. Not model-configurable via the
/// tool's own input — `PLAN.md` only asks for a configurable timeout on
/// `dlt verify`'s checks; this is a fixed safety net so a hung process
/// can't stall the whole agent loop forever.
const RUN_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

fn run_command(
    input: &Value,
    repo_root: &Path,
    config: &Config,
    approver: &dyn Approver,
) -> ToolOutcome {
    let program = match str_field(input, "program") {
        Ok(p) => p,
        Err(e) => return ToolOutcome::fail(e),
    };
    let args: Vec<String> = input
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    if !run_command_allowlist(config).iter().any(|p| p == program) {
        return ToolOutcome::fail(format!(
            "program {program:?} is not on the run_command allowlist"
        ));
    }

    let command_line = format!("{program} {}", args.join(" "));
    if let Some(outcome) = gate("run_command", &command_line, config, approver) {
        return outcome;
    }

    match spawn_with_timeout(program, &args, repo_root, RUN_COMMAND_TIMEOUT) {
        Ok(output) => {
            let text = format!(
                "exit code: {}\nstdout:\n{}\nstderr:\n{}",
                output
                    .status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "terminated by signal".to_string()),
                output.stdout,
                output.stderr,
            );
            ToolOutcome {
                success: output.status.success(),
                output: text,
            }
        }
        Err(e) => ToolOutcome::fail(e),
    }
}

#[derive(Debug)]
struct CommandOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

/// Spawn `program` with `args` directly — never through a shell — in
/// its own process group so a timeout can kill the whole subtree, not
/// just the immediate child. Mirrors `verify.rs`'s `run_cmd`, minus the
/// shell wrapping, which `run_command` must never use.
fn spawn_with_timeout(
    program: &str,
    args: &[String],
    repo_root: &Path,
    timeout: Duration,
) -> Result<CommandOutput, String> {
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let child = command
        .spawn()
        .map_err(|e| format!("failed to spawn {program:?}: {e}"))?;
    let pid = child.id();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(CommandOutput {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        Ok(Err(e)) => Err(format!("failed to run {program:?}: {e}")),
        Err(_) => {
            kill_process_group(pid);
            Err(format!("{program:?} timed out after {timeout:?}"))
        }
    }
}

#[cfg(unix)]
fn kill_process_group(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg("-9")
        .arg(format!("-{pid}"))
        .status();
}

#[cfg(windows)]
fn kill_process_group(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{FsStore, JOURNAL_DIR};
    use tempfile::TempDir;

    struct AlwaysApprove;
    impl Approver for AlwaysApprove {
        fn approve(&self, _tool: &str, _preview: &str) -> bool {
            true
        }
    }

    struct AlwaysDeny;
    impl Approver for AlwaysDeny {
        fn approve(&self, _tool: &str, _preview: &str) -> bool {
            false
        }
    }

    fn setup() -> (TempDir, FsStore, Config) {
        let dir = TempDir::new().unwrap();
        let store_root = dir.path().join(".delta");
        std::fs::create_dir_all(store_root.join(JOURNAL_DIR)).unwrap();
        let config = Config::load(dir.path()).unwrap();
        (dir, FsStore::new(store_root), config)
    }

    fn call(tool: &str, input: Value) -> ToolCall {
        ToolCall {
            tool: tool.to_string(),
            input,
        }
    }

    #[test]
    fn read_file_returns_contents() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let outcome = execute(
            &call("read_file", serde_json::json!({"path": "a.txt"})),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert_eq!(outcome.output, "hello");
    }

    #[test]
    fn read_file_missing_path_field_fails_cleanly() {
        let (dir, store, config) = setup();
        let outcome = execute(
            &call("read_file", serde_json::json!({})),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(!outcome.success);
    }

    #[test]
    fn read_file_truncates_oversized_files() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("big.txt"), "x".repeat(MAX_READ_BYTES + 500)).unwrap();
        let outcome = execute(
            &call("read_file", serde_json::json!({"path": "big.txt"})),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert!(outcome.output.contains("[truncated"));
    }

    #[test]
    fn list_dir_marks_subdirectories() {
        let (dir, store, config) = setup();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        let outcome = execute(
            &call("list_dir", serde_json::json!({"path": "."})),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert!(outcome.output.contains("sub/"));
        assert!(outcome.output.contains("file.txt"));
    }

    #[test]
    fn write_file_is_denied_without_approval() {
        let (dir, store, config) = setup();
        let outcome = execute(
            &call(
                "write_file",
                serde_json::json!({"path": "new.txt", "content": "hi"}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysDeny,
        )
        .unwrap();
        assert!(!outcome.success);
        assert!(!dir.path().join("new.txt").exists());
    }

    #[test]
    fn write_file_succeeds_when_approved_and_is_journalled_and_undoable() {
        let (dir, store, config) = setup();
        let outcome = execute(
            &call(
                "write_file",
                serde_json::json!({"path": "new.txt", "content": "hi"}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "hi"
        );

        journal::undo_last(&store, dir.path()).unwrap();
        assert!(!dir.path().join("new.txt").exists());
    }

    #[test]
    fn write_file_auto_policy_needs_no_approval() {
        let (dir, store, _) = setup();
        // AlwaysDeny would fail this if the policy gate were consulted —
        // proving `auto` really does skip the approver entirely.
        let toml = "[tools.write_file]\npolicy = \"auto\"\n";
        std::fs::write(dir.path().join(".delta/config.toml"), toml).unwrap();
        let config = Config::load(dir.path()).unwrap();

        let outcome = execute(
            &call(
                "write_file",
                serde_json::json!({"path": "auto.txt", "content": "hi"}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysDeny,
        )
        .unwrap();
        assert!(outcome.success);
    }

    #[test]
    fn apply_patch_writes_and_journals_previous_content() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("f.txt"), "one\ntwo\n").unwrap();
        let diff = "@@ -2,1 +2,1 @@\n-two\n+TWO\n";
        let outcome = execute(
            &call(
                "apply_patch",
                serde_json::json!({"path": "f.txt", "diff": diff}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "one\nTWO\n"
        );

        journal::undo_last(&store, dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "one\ntwo\n"
        );
    }

    #[test]
    fn apply_patch_hunk_not_found_fails_without_touching_the_file() {
        let (dir, store, config) = setup();
        std::fs::write(dir.path().join("f.txt"), "one\ntwo\n").unwrap();
        let diff = "@@ -1,1 +1,1 @@\n-nonexistent\n+X\n";
        let outcome = execute(
            &call(
                "apply_patch",
                serde_json::json!({"path": "f.txt", "diff": diff}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(!outcome.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "one\ntwo\n"
        );
    }

    #[test]
    fn run_command_rejects_programs_not_on_the_allowlist() {
        let (dir, store, config) = setup();
        let outcome = execute(
            &call(
                "run_command",
                serde_json::json!({"program": "rm", "args": ["-rf", "/"]}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(!outcome.success);
        assert!(outcome.output.contains("allowlist"));
    }

    #[test]
    fn run_command_runs_an_allowlisted_program() {
        let (dir, store, _) = setup();
        let toml = "[tools.run_command]\nallowlist = [\"echo\"]\n";
        std::fs::write(dir.path().join(".delta/config.toml"), toml).unwrap();
        let config = Config::load(dir.path()).unwrap();

        let outcome = execute(
            &call(
                "run_command",
                serde_json::json!({"program": "echo", "args": ["hi"]}),
            ),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(outcome.success);
        assert!(outcome.output.contains("hi"));
    }

    #[test]
    fn run_command_times_out_and_kills_the_process_group() {
        let result = spawn_with_timeout(
            "sleep",
            &["5".to_string()],
            Path::new("."),
            Duration::from_millis(100),
        );
        let err = result.unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_tool_name_is_a_soft_failure() {
        let (dir, store, config) = setup();
        let outcome = execute(
            &call("teleport", serde_json::json!({})),
            dir.path(),
            &store,
            &config,
            &AlwaysApprove,
        )
        .unwrap();
        assert!(!outcome.success);
        assert!(outcome.output.contains("unknown tool"));
    }
}
