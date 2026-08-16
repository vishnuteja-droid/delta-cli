//! Anthropic Messages API provider: `x-api-key` auth, the
//! `anthropic-version` header, and `event:`-discriminated SSE frames
//! (`content_block_delta` carrying `text_delta` chunks).

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

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    name: String,
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    extra_headers: BTreeMap<String, String>,
    context_window: u32,
}

impl AnthropicProvider {
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
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                json!({"role": role, "content": message.content})
            })
            .collect();
        let mut body = json!({
            "model": self.model,
            "max_tokens": request.max_tokens,
            "messages": messages,
            "stream": true,
        });
        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }
        body
    }

    fn build_request(&self, request: &Request) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&self.request_body(request));
        for (key, value) in &self.extra_headers {
            builder = builder.header(key, value);
        }
        builder
    }
}

impl Provider for AnthropicProvider {
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

/// Parse one SSE event. Only `content_block_delta` events carrying a
/// `text_delta` produce visible text; `message_start`,
/// `content_block_start/stop`, `message_delta`, `message_stop`, and
/// `ping` are structural and are skipped (`None`), same as `filter_map`
/// skips them without ending the stream. A server-sent `event: error`
/// surfaces as an `Err`.
fn parse_event(event: &Event, name: &str) -> Option<Result<Delta, ProviderError>> {
    match event.event.as_str() {
        "content_block_delta" => {
            let value: Value = match serde_json::from_str(&event.data) {
                Ok(value) => value,
                Err(source) => {
                    return Some(Err(ProviderError::MalformedStream {
                        name: name.to_string(),
                        reason: source.to_string(),
                    }));
                }
            };
            let delta = value.get("delta")?;
            if delta.get("type")?.as_str()? != "text_delta" {
                return None;
            }
            let text = delta.get("text")?.as_str()?;
            if text.is_empty() {
                return None;
            }
            Some(Ok(Delta {
                text: text.to_string(),
            }))
        }
        "error" => Some(Err(ProviderError::MalformedStream {
            name: name.to_string(),
            reason: event.data.clone(),
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Message;
    use tokio_util::bytes::Bytes;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn spec(base_url: String) -> ProviderSpec {
        ProviderSpec {
            kind: "anthropic".to_string(),
            base_url,
            model: "claude-test".to_string(),
            api_key_env: "TEST_API_KEY".to_string(),
            headers: BTreeMap::new(),
            context_window: 200_000,
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
    async fn streams_text_deltas_and_ignores_structural_events() {
        let server = MockServer::start().await;
        let body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\"}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo!\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("x-api-key", "sk-ant-test"))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider =
            AnthropicProvider::new("default", &spec(server.uri()), "sk-ant-test".to_string());
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
                Delta {
                    text: "Hel".to_string()
                },
                Delta {
                    text: "lo!".to_string()
                },
            ]
        );
    }

    /// Frames split mid-field across chunk boundaries must still
    /// reassemble into one `Event` (the eventsource-stream buffering
    /// contract this provider relies on).
    #[tokio::test]
    async fn reassembles_sse_event_split_across_chunk_boundaries() {
        let whole = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"chunked\"}}\n\n";
        let split_at = whole.find("\"text_delta\"").unwrap();
        let (first, second) = whole.split_at(split_at);

        let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from(first.to_string())),
            Ok(Bytes::from(second.to_string())),
        ];
        let byte_stream = futures::stream::iter(chunks);
        let mut events = byte_stream.eventsource();

        let event = events.next().await.unwrap().unwrap();
        assert_eq!(event.event, "content_block_delta");
        assert!(event.data.contains("\"chunked\""));
        assert!(events.next().await.is_none());
    }

    #[tokio::test]
    async fn server_sent_error_event_surfaces_as_err() {
        let server = MockServer::start().await;
        let body = concat!(
            "event: error\n",
            "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider =
            AnthropicProvider::new("default", &spec(server.uri()), "sk-ant-test".to_string());
        let mut stream = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap();
        let first = stream.next().await.unwrap();
        assert!(matches!(first, Err(ProviderError::MalformedStream { .. })));
    }

    #[tokio::test]
    async fn retries_on_503_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(
                        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
                        "text/event-stream",
                    ),
            )
            .mount(&server)
            .await;

        let provider =
            AnthropicProvider::new("default", &spec(server.uri()), "sk-ant-test".to_string());
        let deltas: Vec<Delta> = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap()
            .map(|d| d.unwrap())
            .collect()
            .await;

        assert_eq!(
            deltas,
            vec![Delta {
                text: "ok".to_string()
            }]
        );
    }
}
