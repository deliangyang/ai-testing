#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

fn app(tmp: &tempfile::TempDir) -> axum::Router {
    let clock = std::sync::Arc::new(common::FixedClock::new(
        common::Timestamp::from_unix_millis(1_000),
    ));
    let state = server::AppState::open(
        tmp.path().join("t.db"),
        tmp.path().to_path_buf(),
        clock,
        std::sync::Arc::new(common::UuidV7Gen),
        std::sync::Arc::new(server::RunEventHub::new()),
    )
    .unwrap();
    server::router(state)
}

#[tokio::test]
async fn copilot_fixture_returns_actions() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(&tmp);
    let body = json!({
        "context": { "dag_id": "serial", "selected_node_id": "pay" },
        "message": "补健康检查"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/chat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["message"]["role"], "assistant");
    let actions = value["message"]["actions"].as_array().unwrap();
    assert!(actions.iter().any(|a| a["type"] == "insert_canvas"));
}
