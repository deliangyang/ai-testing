#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixedClock, Timestamp, UuidV7Gen};
use http_body_util::BodyExt;
use model::Dag;
use server::{router, AppState, RunEventHub};
use store::{
    DagRepository, ExecutionKey, NodeExecutionRepository, NodeUpdate, RunRepository, RunStatus,
};
use store::{Db, NewRun, RunMode};
use tower::ServiceExt;

fn sample_dag() -> Dag {
    serde_json::from_value(serde_json::json!({
        "id": "api-fixture",
        "nodes": [
            {"id": "ping", "type": "http", "method": "GET", "path": "/health"}
        ],
        "edges": []
    }))
    .unwrap()
}

fn app(tmp: &tempfile::TempDir) -> axum::Router {
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
    let state = AppState::new(
        tmp.path().join("t.db"),
        clock.clone(),
        Arc::new(UuidV7Gen),
        Arc::new(RunEventHub::new()),
    );
    let db = Db::open(state.db_path.clone(), clock.as_ref()).unwrap();
    let saved = DagRepository::new(db.conn(), clock.as_ref())
        .save(&sample_dag(), "test")
        .unwrap();
    DagRepository::new(db.conn(), clock.as_ref())
        .activate("api-fixture", saved.info.version, None)
        .unwrap();
    let run_repo = RunRepository::new(db.conn(), clock.as_ref(), &UuidV7Gen);
    let run = run_repo
        .create(&NewRun {
            dag_id: "api-fixture".into(),
            dag_version: saved.info.version,
            env_name: "local".into(),
            mode: RunMode::Manual,
        })
        .unwrap();
    run_repo.transition(&run.id, RunStatus::Running).unwrap();
    run_repo.transition(&run.id, RunStatus::Completed).unwrap();
    let nodes = NodeExecutionRepository::new(db.conn(), clock.as_ref());
    let key = ExecutionKey::new(&run.id, "ping", 1);
    nodes
        .insert(&key, model::NodeStatus::Running, None)
        .unwrap();
    nodes
        .update(
            &key,
            &NodeUpdate {
                status: model::NodeStatus::Completed,
                output: Some(serde_json::json!({"status": 200})),
                poll_trace: None,
                failure: None,
            },
        )
        .unwrap();
    router(state)
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn read_only_api_lists_dags_runs_and_executions() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(&tmp);

    let res = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(Request::get("/api/dags").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dags = body_json(res).await;
    assert!(dags
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == "api-fixture"));

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/dags/api-fixture/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(Request::get("/api/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let runs = body_json(res).await;
    let run_id = runs[0]["id"].as_str().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/runs/{run_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::get(format!("/api/runs/{run_id}/executions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let execs = body_json(res).await;
    assert_eq!(execs[0]["node_id"], "ping");
}
