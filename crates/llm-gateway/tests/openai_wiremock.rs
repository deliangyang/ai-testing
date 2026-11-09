#![allow(clippy::unwrap_used)]

use llm_gateway::{ChatMessage, ChatRequest, LlmProvider, MessageRole, OpenAiClient, OpenAiConfig};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn openai_client_posts_chat_completions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test-wiremock-secret"))
        .and(header("content-type", "application/json"))
        .and(body_partial_json(json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "user", "content": "hello"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "gpt-4o-mini",
            "choices": [{
                "message": {"role": "assistant", "content": "world"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 2,
                "total_tokens": 7
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = OpenAiClient::new(OpenAiConfig::with_resolved_api_key_for_test(
        server.uri(),
        "gpt-4o-mini",
        "sk-test-wiremock-secret",
    ))
    .unwrap();

    let response = client
        .chat(ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "hello".into(),
            }],
        })
        .await
        .unwrap();

    assert_eq!(response.content, "world");
    assert_eq!(response.usage.total_tokens, 7);
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn openai_client_surfaces_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "invalid api key", "type": "invalid_request_error"}
        })))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(OpenAiConfig::with_resolved_api_key_for_test(
        server.uri(),
        "gpt-4o-mini",
        "sk-test",
    ))
    .unwrap();

    let err = client
        .chat(ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "x".into(),
            }],
        })
        .await
        .unwrap_err();

    match err {
        llm_gateway::LlmError::Api { status, message } => {
            assert_eq!(status, 401);
            assert_eq!(message, "invalid api key");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
