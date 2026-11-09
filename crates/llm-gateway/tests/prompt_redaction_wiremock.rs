#![allow(clippy::unwrap_used)]

use llm_gateway::{ChatMessage, ChatRequest, LlmProvider, MessageRole, OpenAiClient, OpenAiConfig};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RAW_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
const RAW_SECRET: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";

#[tokio::test]
async fn openai_request_body_never_contains_raw_secrets() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("****"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "gpt-4o-mini",
            "choices": [{
                "message": {"content": "ok"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
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

    let prompt = format!(
        "Config dump:\nOPENAI_API_KEY=sk-in-prompt-not-auth\npassword: hunter2\njwt: {RAW_JWT}\nsecret_access_key: {RAW_SECRET}\n"
    );

    client
        .chat(ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: prompt,
            }],
        })
        .await
        .unwrap();

    let received = &server.received_requests().await.unwrap_or_default();
    assert_eq!(received.len(), 1);
    let body = String::from_utf8_lossy(&received[0].body);
    assert!(!body.contains("sk-in-prompt-not-auth"), "{body}");
    assert!(!body.contains("hunter2"), "{body}");
    assert!(!body.contains(RAW_JWT), "{body}");
    assert!(!body.contains(RAW_SECRET), "{body}");
}
