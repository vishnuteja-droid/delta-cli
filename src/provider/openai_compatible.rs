//! OpenAI-compatible provider: the Chat Completions streaming shape
//! used by OpenAI itself, most local model servers (Ollama, llama.cpp),
//! and most LLM gateways. Reasoning-capable servers (DeepSeek's API,
//! several local reasoning-model backends) stream a `reasoning_content`
//! field on `delta` alongside the standard `content` field — surfaced
//! as `Delta::Reasoning`, kept distinct from `Delta::Text`.

use std::collections::BTreeMap;

use eventsource_stream::{Event, Eventsource};
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;

use super::{
    Delta, Provider, ProviderSpec, Request, Role, count_tokens, send_with_retry, with_cancellation,
};

#[derive(Debug, Clone)]
pub struct OpenAiCompatible {
    name: String,
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    extra_headers: BTreeMap<String, String>,
    context_window: u32,
}

impl OpenAiCompatible {
    pub(super) fn new(name: &str, spec: &ProviderSpec, api_key: String) -> Self {
        Self {
            name: name.to_string(),
            client: reqwest::Client::new(),
            base_url: spec.base_url.trim_end_matches('/').to_string(),
            model: spec.model.clone(),
            api_key,
            extra_headers: spec.headers.clone(),
            context_window: spec.context_window,
        }
    }

    fn request_body(&self, request: &Request) -> Value {
        let mut messages = Vec::new();
        if let Some(system) = &request.system {
            messages.push(json!({"role": "system", "content": system}));
        }
        for message in &request.messages {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            messages.push(json!({"role": role, "content": message.content}));
        }
        json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": request.max_tokens,
            "stream": true,
        })
    }

    fn build_request(&self, request: &Request) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&self.request_body(request));
        for (key, value) in &self.extra_headers {
            builder = builder.header(key, value);
        }
        builder
    }
}

impl Provider for OpenAiCompatible {
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
        request: Request,
        cancel: CancellationToken,
    ) -> Result<BoxStream<'static, Result<Delta, ProviderError>>, ProviderError> {
        let response =
            send_with_retry(&self.name, || self.build_request(&request), &cancel).await?;

        let name = self.name.clone();
        let events = Box::pin(response.bytes_stream().eventsource());
        let events = with_cancellation(events, cancel);

        let name_for_events = name.clone();
        let deltas = events.filter_map(move |event| {
            let name = name_for_events.clone();
            std::future::ready(match event {
                Ok(event) => parse_event(&event, &name),
                Err(source) => Some(Err(ProviderError::MalformedStream {
                    name,
                    reason: source.to_string(),
                })),
            })
        });

        Ok(Box::pin(deltas))
    }
}

/// Parse one SSE event from the Chat Completions stream. Returns `None`
/// for events with no visible text (role-only chunks, `[DONE]`,
/// finish-reason-only chunks) so the caller's `filter_map` skips them
/// without ending the stream. `content` is checked before
/// `reasoning_content` — reasoning-capable servers send one or the
/// other per chunk, never expected to send both at once, but if a
/// server somehow did, visible text wins.
fn parse_event(event: &Event, name: &str) -> Option<Result<Delta, ProviderError>> {
    if event.data == "[DONE]" {
        return None;
    }
    let value: Value = match serde_json::from_str(&event.data) {
        Ok(value) => value,
        Err(source) => {
            return Some(Err(ProviderError::MalformedStream {
                name: name.to_string(),
                reason: source.to_string(),
            }));
        }
    };
    let delta = value.get("choices")?.get(0)?.get("delta")?;
    if let Some(text) = delta.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        return Some(Ok(Delta::Text(text.to_string())));
    }
    if let Some(text) = delta.get("reasoning_content").and_then(Value::as_str)
        && !text.is_empty()
    {
        return Some(Ok(Delta::Reasoning(text.to_string())));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Message;
    use tokio_util::bytes::Bytes;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn spec(base_url: String) -> ProviderSpec {
        ProviderSpec {
            kind: "openai_compatible".to_string(),
            base_url,
            model: "test-model".to_string(),
            api_key_env: "TEST_API_KEY".to_string(),
            headers: BTreeMap::new(),
            context_window: 8_000,
        }
    }

    fn request() -> Request {
        Request {
            system: Some("be terse".to_string()),
            messages: vec![Message {
                role: Role::User,
                content: "hi".to_string(),
            }],
            max_tokens: 64,
        }
    }

    #[tokio::test]
    async fn streams_text_deltas_and_stops_at_done() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo!\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = OpenAiCompatible::new("default", &spec(server.uri()), "sk-test".to_string());
        let deltas: Vec<Delta> = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap()
            .map(|d| d.unwrap())
            .collect()
            .await;

        assert_eq!(
            deltas,
            vec![
                Delta::Text("Hel".to_string()),
                Delta::Text("lo!".to_string()),
            ]
        );
    }

    /// `reasoning_content` must surface as `Delta::Reasoning`, distinct
    /// from `content`'s `Delta::Text` — the DeepSeek-style reasoning
    /// field some OpenAI-compatible servers stream.
    #[tokio::test]
    async fn reasoning_content_surfaces_as_reasoning_distinct_from_text() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking...\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Answer.\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = OpenAiCompatible::new("default", &spec(server.uri()), "sk-test".to_string());
        let deltas: Vec<Delta> = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap()
            .map(|d| d.unwrap())
            .collect()
            .await;

        assert_eq!(
            deltas,
            vec![
                Delta::Reasoning("Thinking...".to_string()),
                Delta::Text("Answer.".to_string()),
            ]
        );
    }

    /// The literal frame-split-across-chunk-boundaries requirement:
    /// feed the parser a single SSE event whose bytes are split mid
    /// field, across multiple stream items, and confirm it reassembles
    /// into one `Event` rather than two malformed halves.
    #[tokio::test]
    async fn reassembles_sse_event_split_across_chunk_boundaries() {
        let whole = "data: {\"choices\":[{\"delta\":{\"content\":\"chunked\"}}]}\n\n";
        let split_at = whole.find("delta").unwrap();
        let (first, second) = whole.split_at(split_at);

        let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from(first.to_string())),
            Ok(Bytes::from(second.to_string())),
        ];
        let byte_stream = futures::stream::iter(chunks);
        let mut events = byte_stream.eventsource();

        let event = events.next().await.unwrap().unwrap();
        assert_eq!(
            event.data,
            "{\"choices\":[{\"delta\":{\"content\":\"chunked\"}}]}"
        );
        assert!(events.next().await.is_none());
    }

    #[tokio::test]
    async fn retries_on_429_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
                        "text/event-stream",
                    ),
            )
            .mount(&server)
            .await;

        let provider = OpenAiCompatible::new("default", &spec(server.uri()), "sk-test".to_string());
        let deltas: Vec<Delta> = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap()
            .map(|d| d.unwrap())
            .collect()
            .await;

        assert_eq!(deltas, vec![Delta::Text("ok".to_string())]);
    }

    #[tokio::test]
    async fn non_retryable_status_fails_immediately() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;

        let provider = OpenAiCompatible::new("default", &spec(server.uri()), "sk-test".to_string());
        let err = provider
            .stream(request(), CancellationToken::new())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, ProviderError::Http { status: 401, .. }));
    }

    /// Cancelling before the request even goes out must fail fast, not
    /// silently return an empty stream — no network call is made.
    #[tokio::test]
    async fn stream_fails_fast_when_token_already_cancelled() {
        let server = MockServer::start().await;
        // No Mock registered: if the provider ignored cancellation and
        // sent a request anyway, wiremock would panic on an unexpected call.

        let provider = OpenAiCompatible::new("default", &spec(server.uri()), "sk-test".to_string());
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = provider.stream(request(), cancel).await.err().unwrap();
        assert!(matches!(err, ProviderError::Cancelled { .. }));
    }
}
