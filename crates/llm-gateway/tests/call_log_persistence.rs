#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use common::clock::{FixedClock, Timestamp};
use common::id::SeqIdGen;
use llm_gateway::{
    ChatMessage, ChatRequest, DbLlmCallLogger, LlmCallContext, LoggingLlmProvider, MessageRole,
    OpenAiClient, OpenAiConfig,
};
use serde_json::json;
use store::{Db, LlmCallLogRepository, LlmCallOutcome};
use tempfile::NamedTempFile;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn logging_provider_persists_to_sqlite() {
    let clock: Arc<dyn common::Clock> = Arc::new(FixedClock::new(Timestamp::from_unix_millis(0)));
    let ids: Arc<dyn common::IdGen> = Arc::new(SeqIdGen::new("llm"));
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_path_buf();
    Db::open(&db_path, clock.as_ref()).unwrap();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "gpt-4o-mini",
            "choices": [{
                "message": {"role": "assistant", "content": "pong"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 4, "completion_tokens": 1, "total_tokens": 5}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let inner = OpenAiClient::new(OpenAiConfig::with_resolved_api_key_for_test(
        server.uri(),
        "gpt-4o-mini",
        "sk-test",
    ))
    .unwrap();
    let logger = Arc::new(DbLlmCallLogger::new(
        db_path.clone(),
        Arc::clone(&clock),
        Arc::clone(&ids),
    ));
    let client = LoggingLlmProvider::new(inner, logger);

    client
        .chat_with_context(
            LlmCallContext::new("health_check")
                .with_run_id("run-1")
                .with_node_id("n0"),
            ChatRequest {
                model: "gpt-4o-mini".into(),
                messages: vec![ChatMessage {
                    role: MessageRole::User,
                    content: "ping".into(),
                }],
            },
        )
        .await
        .unwrap();

    let db = Db::open(&db_path, clock.as_ref()).unwrap();
    let rows = LlmCallLogRepository::new(db.conn(), ids.as_ref())
        .list_for_run("run-1")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].purpose, "health_check");
    assert_eq!(rows[0].node_id.as_deref(), Some("n0"));
    assert_eq!(rows[0].tokens_in, 4);
    assert_eq!(rows[0].tokens_out, 1);
    assert_eq!(rows[0].outcome, LlmCallOutcome::Success);
    assert!(rows[0].cost.is_none());
    assert!(rows[0].latency_ms < 60_000);
}
