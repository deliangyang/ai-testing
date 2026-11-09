#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixedClock, Timestamp, UuidV7Gen};
use http_body_util::BodyExt;
use model::Dag;
use server::{router, AppState, RunEventHub};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const BROKEN_PAY_DAG: &str = r#"{
  "id": "serial",
  "nodes": [
    {"id": "create", "type": "http", "method": "POST", "path": "/orders"},
    {"id": "pay", "type": "http", "method": "POST", "path": "/broken/${missing.output.body}"},
    {"id": "check", "type": "assert", "target": "pay", "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]}
  ],
  "edges": [{"from": "create", "to": "pay"}, {"from": "pay", "to": "check"}]
}"#;

#[tokio::test(flavor = "multi_thread")]
async fn repair_ai_returns_pending_candidate_on_success() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var(
        "AITEST_CASSETTE_DIR",
        repo_root().join("crates/generator/tests/fixtures/cassettes"),
    );
    std::env::set_var("AITEST_LLM_MODEL", "cassette-dag-repair-pay");
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
    let state = AppState::open(
        dir.path().join("aitest.db"),
        dir.path().to_path_buf(),
        clock,
        Arc::new(UuidV7Gen),
        Arc::new(RunEventHub::new()),
    )
    .unwrap();
    let app = router(state);
    let dag: Dag = serde_json::from_str(BROKEN_PAY_DAG).unwrap();
    let res = app
        .oneshot(
            Request::post("/api/dags/repair-ai")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&dag).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "accepted");
    assert_eq!(body["gate"]["passed"], true);
    assert_eq!(body["pending"]["status"], "pending");
    assert_eq!(body["pending"]["created_by"], "ai");
}
