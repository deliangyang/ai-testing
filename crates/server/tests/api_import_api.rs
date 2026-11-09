#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::clock::{FixedClock, Timestamp};
use http_body_util::BodyExt;
use importer::ImportPreviewResponse;
use serde_json::json;
use server::{router, AppState, RunEventHub};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn petstore_openapi() -> String {
    fs::read_to_string(repo_root().join("crates/importer/tests/fixtures/openapi/mini_oas3.json"))
        .unwrap()
}

fn test_state() -> AppState {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(
        1_700_000_000_000,
    )));
    AppState::open(
        db,
        dir.path().to_path_buf(),
        clock,
        Arc::new(common::UuidV7Gen),
        Arc::new(RunEventHub::new()),
    )
    .unwrap()
}

#[tokio::test]
async fn preview_openapi_returns_confidence_one() {
    let app = router(test_state());
    let body = json!({
        "format": "openapi",
        "content": petstore_openapi(),
        "strategy": "merge"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/api-specs/import/preview")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let preview: ImportPreviewResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(!preview.items.is_empty());
    assert!(preview.items.iter().all(|row| row.confidence == 1.0));
}
