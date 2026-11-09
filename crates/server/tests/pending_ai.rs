#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixedClock, Timestamp, UuidV7Gen};
use http_body_util::BodyExt;
use model::Dag;
use server::{router, AppState, RunEventHub};
use store::{DagRepository, DagStatus, CREATED_BY_AI};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn sample_dag() -> Dag {
    let text = fs::read_to_string(repo_root().join("examples/serial.yaml")).unwrap();
    Dag::from_yaml_str(&text).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn ai_candidate_save_list_detail_and_requires_gate_to_activate() {
    let dir = tempfile::tempdir().unwrap();
    let clock = std::sync::Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
    let state = AppState::open(
        dir.path().join("aitest.db"),
        dir.path().to_path_buf(),
        clock.clone(),
        std::sync::Arc::new(UuidV7Gen),
        std::sync::Arc::new(RunEventHub::new()),
    )
    .unwrap();
    let app = router(state.clone());
    let dag = sample_dag();
    let dag_json = serde_json::to_value(&dag).unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/dags/candidates")
                .header("content-type", "application/json")
                .body(Body::from(dag_json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let saved = body_json(res).await;
    assert_eq!(saved["status"], "pending");
    assert_eq!(saved["created_by"], CREATED_BY_AI);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/dags/pending")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let pending = body_json(res).await;
    assert_eq!(pending.as_array().unwrap().len(), 1);

    let version = saved["version"].as_u64().unwrap() as u32;
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/dags/serial/versions/{version}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let detail = body_json(res).await;
    assert_eq!(detail["created_by"].as_str().unwrap(), CREATED_BY_AI);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/dags?status=pending&created_by=ai")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let filtered = body_json(res).await;
    assert_eq!(filtered, pending);

    // Corrupt stored content: repository `activate` alone would not re-validate.
    state
        .with_locked_db(|db| {
            db.conn().execute(
                "UPDATE dag_definitions SET content_json = ?1 WHERE id = 'serial' AND version = ?2",
                rusqlite::params![
                    r#"{"id":"serial","nodes":[{"id":"a","type":"http","method":"GET","path":"/"},{"id":"b","type":"http","method":"GET","path":"/"}],"edges":[{"from":"a","to":"b"},{"from":"b","to":"a"}]}"#,
                    version,
                ],
            )?;
            Ok(())
        })
        .unwrap();
    state
        .with_locked_db(|db| {
            let repo = DagRepository::new(db.conn(), state.clock.as_ref());
            let previous = repo.active_version("serial")?;
            repo.activate("serial", version, previous)?;
            Ok(())
        })
        .unwrap();
    state
        .with_locked_db(|db| {
            db.conn().execute(
                "UPDATE dag_definitions SET status = 'pending' WHERE id = 'serial' AND version = ?1",
                [version],
            )?;
            Ok(())
        })
        .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/dags/serial/versions/{version}/activate"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "HTTP activate must re-run the gate and reject bypassing it"
    );
    let gate = body_json(res).await;
    assert!(!gate["passed"].as_bool().unwrap());

    let still_pending = state
        .with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref())
                .get("serial", version)
                .map(|r| r.map(|rec| rec.info.status))
        })
        .unwrap()
        .unwrap();
    assert_eq!(still_pending, DagStatus::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn ai_candidate_save_never_sets_active() {
    let dir = tempfile::tempdir().unwrap();
    let clock = std::sync::Arc::new(FixedClock::new(Timestamp::from_unix_millis(1_000)));
    let state = AppState::open(
        dir.path().join("aitest.db"),
        dir.path().to_path_buf(),
        clock,
        std::sync::Arc::new(UuidV7Gen),
        std::sync::Arc::new(RunEventHub::new()),
    )
    .unwrap();
    let app = router(state.clone());
    let dag_json = serde_json::to_value(sample_dag()).unwrap();

    let res = app
        .oneshot(
            Request::post("/api/dags/candidates")
                .header("content-type", "application/json")
                .body(Body::from(dag_json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let saved = body_json(res).await;
    assert_eq!(saved["status"], "pending");
    assert_eq!(
        state
            .with_locked_db(|db| {
                DagRepository::new(db.conn(), state.clock.as_ref()).active_version("serial")
            })
            .unwrap(),
        None
    );
}
