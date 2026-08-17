//! LLM provider abstraction: async streaming completions, retry with
//! backoff, and mid-flight cancellation, with `OpenAiCompatible`,
//! `Anthropic`, and `Gemini` implementations declared in `config.rs` and
//! constructed here via [`load`]. Driven by `dlt run <stage>` in
//! `cli.rs`.

pub mod anthropic;
pub mod gemini;
pub mod openai_compatible;

use std::collections::BTreeMap;
use std::time::Duration;

use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::ProviderError;

/// A chat request. Deliberately minimal — just enough for the stage
/// machine (prompt 3) to assemble a prompt and stream a completion.
#[derive(Debug, Clone)]
pub struct Request {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    /// Not yet constructed anywhere: `dlt run` only ever sends a single
    /// user turn per stage. Kept so `Message`/`Role` don't need a
    /// breaking shape change once multi-turn conversation history exists.
    #[allow(dead_code)]
    Assistant,
}

/// One incremental chunk of assistant output: either visible `Text`
/// (part of the model's actual response) or `Reasoning` — Anthropic's
/// `thinking_delta`, OpenAI-compatible reasoning-model servers'
/// `reasoning_content` delta field, or Gemini's `thought`-flagged parts.
/// Kept distinct rather than folded into `Text` so callers never let a
/// model's chain-of-thought leak into a saved artifact or a tool-call
/// parse (prompt 5's `tools::agent`), and so prompt 6's TUI can render
/// it as a collapsible, visually distinct block instead of ordinary
/// output. Not every backend streams reasoning; providers that don't
/// simply never emit this variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    Text(String),
    Reasoning(String),
}

/// An LLM provider: streaming completions plus the metadata the stage
/// machine needs to budget context (prompt 3).
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn context_window(&self) -> u32;

    /// Approximate token count. Real tokenization arrives in prompt 3;
    /// until then this is a chars/4 heuristic, not provider-accurate.
    fn count_tokens(&self, text: &str) -> u32;

    fn stream(
        &self,
        request: Request,
        cancel: CancellationToken,
    ) -> impl Future<
        Output = Result<BoxStream<'static, Result<Delta, ProviderError>>, ProviderError>,
    > + Send;
}

/// Runtime-selected provider, dispatched by `kind` from config. A plain
/// enum rather than `Box<dyn Provider>`: `Provider::stream` is a native
/// async fn, which native traits don't make dyn-compatible, and nothing
/// here needs one provider list holding mixed kinds at once.
#[derive(Debug)]
pub enum AnyProvider {
    OpenAiCompatible(openai_compatible::OpenAiCompatible),
    Anthropic(anthropic::AnthropicProvider),
    Gemini(gemini::GeminiProvider),
}

impl Provider for AnyProvider {
    fn name(&self) -> &str {
        match self {
            AnyProvider::OpenAiCompatible(p) => p.name(),
            AnyProvider::Anthropic(p) => p.name(),
            AnyProvider::Gemini(p) => p.name(),
        }
    }

    fn context_window(&self) -> u32 {
        match self {
            AnyProvider::OpenAiCompatible(p) => p.context_window(),
            AnyProvider::Anthropic(p) => p.context_window(),
            AnyProvider::Gemini(p) => p.context_window(),
        }
    }

    fn count_tokens(&self, text: &str) -> u32 {
        match self {
            AnyProvider::OpenAiCompatible(p) => p.count_tokens(text),
            AnyProvider::Anthropic(p) => p.count_tokens(text),
            AnyProvider::Gemini(p) => p.count_tokens(text),
        }
    }

    async fn stream(
        &self,
        request: Request,
        cancel: CancellationToken,
    ) -> Result<BoxStream<'static, Result<Delta, ProviderError>>, ProviderError> {
        match self {
            AnyProvider::OpenAiCompatible(p) => p.stream(request, cancel).await,
            AnyProvider::Anthropic(p) => p.stream(request, cancel).await,
            AnyProvider::Gemini(p) => p.stream(request, cancel).await,
        }
    }
}

/// A provider's config, read out of `[providers.<name>]`. Every field
/// here is drawn straight from PLAN.md's example TOML except
/// `context_window`, which isn't in that example — the Messages/Chat
/// Completions APIs don't report it, so it has to come from config or a
/// fallback; see `DEFAULT_CONTEXT_WINDOW` below.
struct ProviderSpec {
    kind: String,
    base_url: String,
    model: String,
    api_key_env: String,
    headers: BTreeMap<String, String>,
    context_window: u32,
}

const DEFAULT_CONTEXT_WINDOW: u32 = 128_000;

/// Load and construct the provider declared as `[providers.<name>]`.
pub fn load(config: &Config, name: &str) -> Result<AnyProvider, ProviderError> {
    let spec = load_spec(config, name)?;
    let api_key = std::env::var(&spec.api_key_env).map_err(|_| ProviderError::MissingApiKey {
        name: name.to_string(),
        env_var: spec.api_key_env.clone(),
    })?;

    match spec.kind.as_str() {
        "openai_compatible" => Ok(AnyProvider::OpenAiCompatible(
            openai_compatible::OpenAiCompatible::new(name, &spec, api_key),
        )),
        "anthropic" => Ok(AnyProvider::Anthropic(anthropic::AnthropicProvider::new(
            name, &spec, api_key,
        ))),
        "gemini" => Ok(AnyProvider::Gemini(gemini::GeminiProvider::new(
            name, &spec, api_key,
        ))),
        other => Err(ProviderError::InvalidConfig {
            name: name.to_string(),
            reason: format!(
                "unknown provider kind {other:?} (expected \"openai_compatible\", \"anthropic\", or \"gemini\")"
            ),
        }),
    }
}

/// A provider stand-in for `dlt run --dry-run`: exposes only
/// `context_window`/`count_tokens` (both derivable from config alone) so
/// prompt assembly can be debugged without live credentials — `--dry-run`
/// must work even when no `api_key_env` is set. `stream` is never called
/// on it; `dlt run` without `--dry-run` uses [`load`] instead.
#[derive(Debug)]
pub struct DryRunProvider {
    name: String,
    context_window: u32,
}

impl Provider for DryRunProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn context_window(&self) -> u32 {
        self.context_window
    }

    fn count_tokens(&self, text: &str) -> u32 {
        count_tokens(text)
    }

    async fn stream(
        &self,
        _request: Request,
        _cancel: CancellationToken,
    ) -> Result<BoxStream<'static, Result<Delta, ProviderError>>, ProviderError> {
        unreachable!("DryRunProvider is only used for --dry-run prompt assembly, never streamed")
    }
}

/// Load just enough of `[providers.<name>]` to budget a prompt: no
/// `api_key_env` lookup, so this succeeds even without credentials.
pub fn load_for_dry_run(config: &Config, name: &str) -> Result<DryRunProvider, ProviderError> {
    let spec = load_spec(config, name)?;
    Ok(DryRunProvider {
        name: name.to_string(),
        context_window: spec.context_window,
    })
}

fn load_spec(config: &Config, name: &str) -> Result<ProviderSpec, ProviderError> {
    let prefix = format!("providers.{name}");
    if config.get(&prefix).is_none() {
        return Err(ProviderError::NotConfigured {
            name: name.to_string(),
        });
    }

    let required = |key: &str| -> Result<String, ProviderError> {
        config
            .get_str(&format!("{prefix}.{key}"))
            .map(str::to_string)
            .ok_or_else(|| ProviderError::MissingConfig {
                name: name.to_string(),
                key: key.to_string(),
            })
    };

    let context_window = config
        .get(&format!("{prefix}.context_window"))
        .and_then(toml::Value::as_integer)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);

    let headers = config
        .get(&format!("{prefix}.headers"))
        .and_then(toml::Value::as_table)
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Ok(ProviderSpec {
        kind: required("kind")?,
        base_url: required("base_url")?,
        model: required("model")?,
        api_key_env: required("api_key_env")?,
        headers,
        context_window,
    })
}

/// Chars/4 token approximation: the commonly cited rule of thumb for
/// English text. Used only as a fallback if the bundled `cl100k_base`
/// vocab somehow fails to load — `count_tokens` is the real API.
fn approximate_token_count(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    chars.div_ceil(4).max(if text.is_empty() { 0 } else { 1 })
}

/// The `cl100k_base` BPE, built once from its bundled (not
/// network-fetched) vocab data. `cl100k_base()` returns a `Result` only
/// because the crate's API is generic over loading failures; with the
/// vocab compiled in via `include_str!` it cannot actually fail here, so
/// this treats it as fallible anyway (`unwrap`/`expect` are denied
/// crate-wide) and falls back to the chars/4 heuristic rather than panic.
fn bpe() -> Option<&'static tiktoken_rs::CoreBPE> {
    static BPE: std::sync::OnceLock<Option<tiktoken_rs::CoreBPE>> = std::sync::OnceLock::new();
    BPE.get_or_init(|| tiktoken_rs::cl100k_base().ok()).as_ref()
}

/// Real BPE token count via `cl100k_base`, the vocab OpenAI- and
/// Anthropic-compatible endpoints both roughly track closely enough for
/// context-budget purposes.
fn count_tokens(text: &str) -> u32 {
    match bpe() {
        Some(bpe) => bpe.encode_ordinary(text).len() as u32,
        None => approximate_token_count(text),
    }
}

/// Read `Retry-After` as a whole number of seconds. HTTP also allows an
/// http-date there; providers rate-limiting a request send delta-seconds
/// in practice, so that's the only form handled.
fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?;
    let seconds: u64 = value.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(seconds))
}

/// POST with retry: exponential backoff with jitter on 429 and 5xx,
/// honoring `Retry-After` when present. Every other status is permanent.
/// `build` must return a fresh, unsent `RequestBuilder` each call.
async fn send_with_retry(
    provider_name: &str,
    build: impl Fn() -> reqwest::RequestBuilder,
    cancel: &CancellationToken,
) -> Result<reqwest::Response, ProviderError> {
    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(500))
        .with_max_interval(Duration::from_secs(30))
        .with_max_elapsed_time(Some(Duration::from_secs(120)))
        .build();

    backoff::future::retry(policy, || async {
        if cancel.is_cancelled() {
            return Err(backoff::Error::permanent(ProviderError::Cancelled {
                name: provider_name.to_string(),
            }));
        }

        let response = build().send().await.map_err(|source| {
            backoff::Error::transient(ProviderError::Request {
                name: provider_name.to_string(),
                source,
            })
        })?;

        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let retryable =
            status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        let retry_after = retry_after_from_headers(response.headers());
        let body = response.text().await.unwrap_or_default();
        let err = ProviderError::Http {
            name: provider_name.to_string(),
            status: status.as_u16(),
            body,
        };

        if !retryable {
            return Err(backoff::Error::permanent(err));
        }
        match retry_after {
            Some(duration) => Err(backoff::Error::retry_after(err, duration)),
            None => Err(backoff::Error::transient(err)),
        }
    })
    .await
}

/// Wrap a boxed, pinned stream so it stops yielding once `cancel` fires
/// — the "abortable mid-flight" requirement. Boxing+pinning upfront
/// sidesteps the `Unpin` bounds `StreamExt::next` needs regardless of
/// what adapter chain produced the stream.
fn with_cancellation<S>(
    stream: std::pin::Pin<Box<S>>,
    cancel: CancellationToken,
) -> BoxStream<'static, S::Item>
where
    S: Stream + Send + 'static,
{
    Box::pin(futures::stream::unfold(
        (stream, cancel),
        |(mut stream, cancel)| async move {
            tokio::select! {
                biased;
                () = cancel.cancelled() => None,
                maybe_item = stream.next() => maybe_item.map(|item| (item, (stream, cancel))),
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_approximation_is_roughly_chars_over_four() {
        assert_eq!(approximate_token_count(""), 0);
        assert_eq!(approximate_token_count("abcd"), 1);
        assert_eq!(approximate_token_count("abcde"), 2);
        assert_eq!(approximate_token_count(&"a".repeat(400)), 100);
    }

    #[test]
    fn count_tokens_uses_real_bpe() {
        assert_eq!(count_tokens(""), 0);
        // Real BPE tokens roughly track words, not chars/4 — a repeated
        // single character compresses far below the chars/4 estimate,
        // which is the property that matters for budgeting: real counts
        // and the heuristic must not be interchangeable.
        let text = "a".repeat(400);
        assert!(count_tokens(&text) < approximate_token_count(&text));
    }

    #[test]
    fn load_spec_requires_declared_provider() {
        let config = Config::load(std::path::Path::new("/nonexistent")).unwrap();
        let err = load(&config, "missing").unwrap_err();
        assert!(matches!(err, ProviderError::NotConfigured { .. }));
    }

    /// The literal "abortable mid-flight" requirement, tested at the
    /// combinator itself rather than through a real HTTP stream: cancel
    /// partway through and confirm no further items are yielded.
    #[tokio::test]
    async fn with_cancellation_stops_mid_stream() {
        let cancel = CancellationToken::new();
        let cancel_inside = cancel.clone();
        let inner = futures::stream::iter(vec![1, 2, 3]).then(move |item| {
            let cancel_inside = cancel_inside.clone();
            async move {
                if item == 2 {
                    // Simulate the caller cancelling once item 2 has
                    // already started being produced.
                    cancel_inside.cancel();
                }
                item
            }
        });

        let mut cancellable = with_cancellation(Box::pin(inner), cancel);
        let mut collected = Vec::new();
        while let Some(item) = cancellable.next().await {
            collected.push(item);
        }

        assert_eq!(collected, vec![1, 2]);
    }
}
