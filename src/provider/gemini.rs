//! Gemini (Google Generative Language API) provider: `x-goog-api-key`
//! auth, `:streamGenerateContent?alt=sse` for real SSE framing (without
//! `alt=sse` the API instead returns one large chunked JSON array, not
//! discrete events), and Gemini's own `contents`/`systemInstruction`
//! request shape rather than either other provider's.

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
pub struct GeminiProvider {
    name: String,
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    extra_headers: BTreeMap<String, String>,
    context_window: u32,
}

impl GeminiProvider {
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

    /// Gemini has no "assistant" role — prior model turns are `"model"`.
    fn request_body(&self, request: &Request) -> Value {
        let contents: Vec<Value> = request
            .messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "model",
                };
                json!({"role": role, "parts": [{"text": message.content}]})
            })
            .collect();
        let mut body = json!({
            "contents": contents,
            "generationConfig": {"maxOutputTokens": request.max_tokens},
        });
        if let Some(system) = &request.system {
            body["systemInstruction"] = json!({"parts": [{"text": system}]});
        }
        body
    }

    fn build_request(&self, request: &Request) -> reqwest::RequestBuilder {
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url, self.model
        );
        let mut builder = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .json(&self.request_body(request));
        for (key, value) in &self.extra_headers {
            builder = builder.header(key, value);
        }
        builder
    }
}

impl Provider for GeminiProvider {
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

/// Parse one SSE event. Each frame is a full `GenerateContentResponse`
/// JSON object (unlike Anthropic/OpenAI's small incremental deltas,
/// Gemini's frames are self-contained), whose visible text is the
/// concatenation of `candidates[0].content.parts[].text`. A frame with
/// no candidates or empty text yields no delta; an `{"error": ...}`
/// body (Gemini can send one inline even inside a 200 SSE stream) or a
/// malformed JSON body both surface as `Err`.
fn parse_event(event: &Event, name: &str) -> Option<Result<Delta, ProviderError>> {
    let value: Value = match serde_json::from_str(&event.data) {
        Ok(value) => value,
        Err(source) => {
            return Some(Err(ProviderError::MalformedStream {
                name: name.to_string(),
                reason: source.to_string(),
            }));
        }
    };
    if let Some(error) = value.get("error") {
        return Some(Err(ProviderError::MalformedStream {
            name: name.to_string(),
            reason: error.to_string(),
        }));
    }
    let parts = value
        .get("candidates")?
        .get(0)?
        .get("content")?
        .get("parts")?
        .as_array()?;
    let text: String = parts
        .iter()
        .filter_map(|part| part.get("text")?.as_str())
        .collect();
    if text.is_empty() {
        return None;
    }
    Some(Ok(Delta { text }))
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
            kind: "gemini".to_string(),
            base_url,
            model: "gemini-test".to_string(),
            api_key_env: "TEST_API_KEY".to_string(),
            headers: BTreeMap::new(),
            context_window: 1_000_000,
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
    async fn streams_text_deltas_across_multiple_frames() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hel\"}],\"role\":\"model\"},\"index\":0}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"lo!\"}],\"role\":\"model\"},\"index\":0,\"finishReason\":\"STOP\"}]}\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/models/gemini-test:streamGenerateContent"))
            .and(header("x-goog-api-key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
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

    /// A frame with multiple `parts` concatenates their text into one delta.
    #[tokio::test]
    async fn concatenates_multiple_parts_in_one_frame() {
        let server = MockServer::start().await;
        let body = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"foo \"},{\"text\":\"bar\"}]}}]}\n\n";
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
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
                text: "foo bar".to_string()
            }]
        );
    }

    #[tokio::test]
    async fn reassembles_sse_event_split_across_chunk_boundaries() {
        let whole =
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"chunked\"}]}}]}\n\n";
        let split_at = whole.find("\"chunked\"").unwrap();
        let (first, second) = whole.split_at(split_at);

        let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from(first.to_string())),
            Ok(Bytes::from(second.to_string())),
        ];
        let byte_stream = futures::stream::iter(chunks);
        let mut events = byte_stream.eventsource();

        let event = events.next().await.unwrap().unwrap();
        assert!(event.data.contains("\"chunked\""));
        assert!(events.next().await.is_none());
    }

    #[tokio::test]
    async fn inline_error_body_surfaces_as_err() {
        let server = MockServer::start().await;
        let body = "data: {\"error\":{\"code\":429,\"message\":\"quota exceeded\",\"status\":\"RESOURCE_EXHAUSTED\"}}\n\n";
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
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
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(
                        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n\n",
                        "text/event-stream",
                    ),
            )
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
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

    #[tokio::test]
    async fn non_retryable_status_fails_immediately() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
        let err = provider
            .stream(request(), CancellationToken::new())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, ProviderError::Http { status: 400, .. }));
    }

    #[tokio::test]
    async fn stream_fails_fast_when_token_already_cancelled() {
        let server = MockServer::start().await;
        let provider = GeminiProvider::new("default", &spec(server.uri()), "test-key".to_string());
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = provider.stream(request(), cancel).await.err().unwrap();
        assert!(matches!(err, ProviderError::Cancelled { .. }));
    }
}
