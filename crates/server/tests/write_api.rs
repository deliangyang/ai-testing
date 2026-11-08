#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixedClock, Sleeper, Timestamp, TokioSleeper, UuidV7Gen};
use http_body_util::BodyExt;
use mock_sut::MockSutBuilder;
use model::Dag;
use server::{router, AppState, RunEventHub};
use store::{RunRepository, RunStatus};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn install_rustls() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

struct Layout {
    _sut: mock_sut::MockSutHandle,
    _dir: tempfile::TempDir,
    state: AppState,
}

impl Layout {
    async fn new(sut: mock_sut::MockSutHandle) -> Self {
        install_rustls();
        let base_url = sut.base_url();
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("environments")).unwrap();
        fs::write(
            dir.path().join("environments/local.yaml"),
            format!("base_url: {base_url}\n"),
        )
        .unwrap();
        let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
        let state = AppState::open(
            dir.path().join("aitest.db"),
            dir.path().to_path_buf(),
            clock,
            Arc::new(UuidV7Gen),
            Arc::new(RunEventHub::new()),
        )
        .unwrap();
        Self {
            _sut: sut,
            _dir: dir,
            state,
        }
    }

    fn app(&self) -> axum::Router {
        router(self.state.clone())
    }

    fn serial_dag() -> Dag {
        let text = fs::read_to_string(repo_root().join("examples/serial.yaml")).unwrap();
        Dag::from_yaml_str(&text).unwrap()
    }
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn wait_run_terminal(state: &AppState, run_id: &str) -> RunStatus {
    for _ in 0..400 {
        let status = state
            .with_locked_db(|db| {
                Ok(
                    RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref())
                        .get(run_id)?
                        .map(|r| r.status),
                )
            })
            .unwrap()
            .unwrap_or(RunStatus::Running);
        if status.is_terminal() {
            return status;
        }
        TokioSleeper.sleep(Duration::from_millis(25)).await;
    }
    panic!("run {run_id} did not finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn write_api_validate_save_activate_and_run() {
    let sut = MockSutBuilder::new()
        .pay_delay_ms(0)
        .jitter_ms(0)
        .seed(1)
        .spawn()
        .await
        .unwrap();
    let layout = Layout::new(sut).await;
    let app = layout.app();
    let dag = Layout::serial_dag();
    let dag_json = serde_json::to_value(&dag).unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/dags/validate")
                .header("content-type", "application/json")
                .body(Body::from(dag_json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let gate = body_json(res).await;
    assert!(gate["passed"].as_bool().unwrap());

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/dags")
                .header("content-type", "application/json")
                .body(Body::from(dag_json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let saved = body_json(res).await;
    assert_eq!(saved["status"], "pending");

    let version = saved["version"].as_u64().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/dags/serial/versions/{version}/activate"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"dag_id": "serial", "env": "local"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let started = body_json(res).await;
    let run_id = started["run_id"].as_str().unwrap();
    TokioSleeper.sleep(Duration::from_millis(50)).await;
    let status = wait_run_terminal(&layout.state, run_id).await;
    assert_eq!(status, RunStatus::Completed);

    let res = app
        .oneshot(
            Request::post(format!("/api/runs/{run_id}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn save_rejected_when_gate_fails() {
    let sut = MockSutBuilder::new().spawn().await.unwrap();
    let layout = Layout::new(sut).await;
    let app = layout.app();
    let dag = serde_json::json!({
        "id": "cycle",
        "nodes": [
            {"id": "a", "type": "http", "method": "GET", "path": "/"},
            {"id": "b", "type": "http", "method": "GET", "path": "/"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "a"}
        ]
    });

    let res = app
        .oneshot(
            Request::post("/api/dags")
                .header("content-type", "application/json")
                .body(Body::from(dag.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(res).await;
    assert!(!body["passed"].as_bool().unwrap());
}
