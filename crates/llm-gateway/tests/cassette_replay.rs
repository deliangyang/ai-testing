#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use llm_gateway::{
    CassetteProvider, ChatMessage, ChatRequest, ChatResponse, LlmProvider, MessageRole, TokenUsage,
};
use model::to_canonical_json;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

fn sample_request() -> ChatRequest {
    ChatRequest {
        model: "gpt-4o-mini".into(),
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are terse.".into(),
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Say pong.".into(),
            },
        ],
    }
}

#[tokio::test]
async fn cassette_replay_is_deterministic() {
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let request = sample_request();
    let first = provider.chat(request.clone()).await.unwrap();
    let second = provider.chat(request).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.content, "pong");
    assert_eq!(first.usage.total_tokens, 12);
    let json = to_canonical_json(&first).unwrap();
    assert!(json.contains("\"content\":\"pong\""));
}

#[tokio::test]
async fn cassette_miss_returns_error() {
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let err = provider
        .chat(ChatRequest {
            model: "missing".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "nope".into(),
            }],
        })
        .await
        .unwrap_err();
    assert!(matches!(err, llm_gateway::LlmError::CassetteMiss { .. }));
}

#[tokio::test]
async fn fixture_matches_request_hash_filename() {
    let request = sample_request();
    let key = llm_gateway::chat_request_key(&request).unwrap();
    let path = fixtures_dir().join(format!("{key}.json"));
    assert!(path.is_file(), "expected cassette at {}", path.display());
    let expected = ChatResponse {
        content: "pong".into(),
        model: "gpt-4o-mini".into(),
        usage: TokenUsage {
            prompt_tokens: 8,
            completion_tokens: 4,
            total_tokens: 12,
        },
        finish_reason: Some("stop".into()),
    };
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let got = provider.chat(request).await.unwrap();
    assert_eq!(got, expected);
}
