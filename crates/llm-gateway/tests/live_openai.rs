#![allow(clippy::unwrap_used)]

//! Optional live call; skipped unless `AITEST_LIVE_LLM=1` and credentials are configured.

use llm_gateway::{ChatMessage, ChatRequest, LlmProvider, MessageRole, OpenAiClient, OpenAiConfig};

#[tokio::test]
#[ignore = "live LLM; run with AITEST_LIVE_LLM=1"]
async fn live_openai_smoke() {
    if std::env::var("AITEST_LIVE_LLM").ok().as_deref() != Some("1") {
        eprintln!("skipping live OpenAI test (set AITEST_LIVE_LLM=1)");
        return;
    }
    let api_key_ref =
        std::env::var("AITEST_LIVE_LLM_KEY_REF").unwrap_or_else(|_| "${env:OPENAI_API_KEY}".into());
    let base_url = std::env::var("AITEST_LIVE_LLM_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com".into());
    let model = std::env::var("AITEST_LIVE_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());

    let client =
        OpenAiClient::new(OpenAiConfig::new(&model, &api_key_ref).with_base_url(&base_url))
            .unwrap();

    let response = client
        .chat(ChatRequest {
            model: model.clone(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Reply with exactly: ok".into(),
            }],
        })
        .await
        .unwrap();

    assert!(!response.content.is_empty());
    assert!(response.usage.total_tokens > 0);
}
