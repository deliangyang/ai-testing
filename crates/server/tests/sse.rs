#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixedClock, Sleeper, Timestamp, TokioSleeper, UuidV7Gen};
use engine::{EngineEvent, NodeStatus};
use http_body_util::BodyExt;
use model::Dag;
use server::{router, AppState, RunEventHub};
use store::{DagRepository, NewRun, RunMode, RunRepository, RunStatus};
use tower::ServiceExt;

fn seed_run(tmp: &tempfile::TempDir) -> (AppState, String) {
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
    let hub = Arc::new(RunEventHub::new());
    let state = AppState::open(
        tmp.path().join("t.db"),
        tmp.path().to_path_buf(),
        clock.clone(),
        Arc::new(UuidV7Gen),
        hub.clone(),
    )
    .unwrap();
    let dag: Dag = serde_json::from_value(serde_json::json!({
        "id": "sse-fixture",
        "nodes": [{"id": "a", "type": "http", "method": "GET", "path": "/"}],
        "edges": []
    }))
    .unwrap();
    let saved = state
        .with_locked_db(|db| DagRepository::new(db.conn(), clock.as_ref()).save(&dag, "test"))
        .unwrap();
    state
        .with_locked_db(|db| {
            DagRepository::new(db.conn(), clock.as_ref()).activate(
                "sse-fixture",
                saved.info.version,
                None,
            )
        })
        .unwrap();
    let run = state
        .with_locked_db(|db| {
            RunRepository::new(db.conn(), clock.as_ref(), &UuidV7Gen).create(&NewRun {
                dag_id: "sse-fixture".into(),
                dag_version: saved.info.version,
                env_name: "local".into(),
                mode: RunMode::Manual,
            })
        })
        .unwrap();
    state
        .with_locked_db(|db| {
            RunRepository::new(db.conn(), clock.as_ref(), &UuidV7Gen)
                .transition(&run.id, RunStatus::Running)
        })
        .unwrap();
    hub.push(
        &run.id,
        EngineEvent::NodeStateChanged {
            node: "a".into(),
            status: NodeStatus::Running,
        },
    );
    hub.push(
        &run.id,
        EngineEvent::NodeStateChanged {
            node: "a".into(),
            status: NodeStatus::Completed,
        },
    );
    hub.push(
        &run.id,
        EngineEvent::RunFinished {
            status: RunStatus::Completed,
        },
    );
    (state, run.id)
}

async fn read_sse(mut body: Body, until_contains: &str) -> String {
    let mut buf = Vec::new();
    for _ in 0..30 {
        let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .expect("timed out waiting for SSE frame")
            .expect("stream ended")
            .expect("frame error");
        if let Ok(chunk) = frame.into_data() {
            buf.extend_from_slice(&chunk);
            let text = String::from_utf8(buf.clone()).unwrap();
            if text.contains(until_contains) {
                return text;
            }
        }
    }
    String::from_utf8(buf).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn sse_replays_buffered_engine_events() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, run_id) = seed_run(&tmp);
    let app = router(state);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/runs/{run_id}/events"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let text = read_sse(res.into_body(), "run_finished").await;
    assert!(text.contains("node_state_changed"));
    assert!(text.contains("run_finished"));
    assert!(text.contains("id: 1"));
    assert!(text.contains("id: 3"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sse_resumes_after_last_event_id() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, run_id) = seed_run(&tmp);
    let app = router(state);

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/runs/{run_id}/events"))
                .header("Last-Event-ID", "2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = read_sse(res.into_body(), "id: 3").await;
    assert!(text.contains("id: 3"));
    assert!(!text.contains("id: 1\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sse_streams_live_events_after_replay() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, run_id) = seed_run(&tmp);
    let hub = state.hub.clone();
    let app = router(state.clone());

    let rid = run_id.clone();
    tokio::spawn(async move {
        TokioSleeper.sleep(Duration::from_millis(50)).await;
        hub.push(
            &rid,
            EngineEvent::Log {
                node: Some("a".into()),
                level: engine::LogLevel::Info,
                message: "late".into(),
            },
        );
    });

    let res = app
        .oneshot(
            Request::get(format!("/api/runs/{run_id}/events"))
                .header("Last-Event-ID", "3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let text = read_sse(res.into_body(), "late").await;
    assert!(text.contains("late"));
    assert!(text.contains("id: 4"));
}
