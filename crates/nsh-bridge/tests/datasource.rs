#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use common::{FixedClock, Timestamp, UuidV7Gen};
use model::Dag;
use nsh_bridge::datasource::{
    ensure_api, get_active_dag, list_dags, list_executions, list_runs, HostConfig,
};
use nsh_bridge::reset_host_for_tests;
use store::{
    DagRepository, ExecutionKey, NewRun, NodeExecutionRepository, NodeUpdate, RunMode,
    RunRepository, RunStatus,
};

fn sample_dag() -> Dag {
    serde_json::from_value(serde_json::json!({
        "id": "serial",
        "nodes": [
            {"id": "create", "type": "http", "method": "POST", "path": "/orders"},
            {"id": "pay", "type": "http", "method": "POST", "path": "/pay"},
            {"id": "check", "type": "http", "method": "GET", "path": "/status"}
        ],
        "edges": [
            {"from": "create", "to": "pay"},
            {"from": "pay", "to": "check"}
        ]
    }))
    .unwrap()
}

fn seed_db(path: &std::path::Path, clock: &FixedClock) {
    let db = store::Db::open(path, clock).unwrap();
    let repo = DagRepository::new(db.conn(), clock);
    let saved = repo.save(&sample_dag(), "test").unwrap();
    repo.activate("serial", saved.info.version, None).unwrap();
    let run_repo = RunRepository::new(db.conn(), clock, &UuidV7Gen);
    let run = run_repo
        .create(&NewRun {
            dag_id: "serial".into(),
            dag_version: saved.info.version,
            env_name: "local".into(),
            mode: RunMode::Manual,
        })
        .unwrap();
    run_repo.transition(&run.id, RunStatus::Running).unwrap();
    run_repo.transition(&run.id, RunStatus::Completed).unwrap();
    for node in ["create", "pay", "check"] {
        let key = ExecutionKey::new(&run.id, node, 1);
        let nodes = NodeExecutionRepository::new(db.conn(), clock);
        nodes
            .insert(&key, model::NodeStatus::Running, None)
            .unwrap();
        nodes
            .update(
                &key,
                &NodeUpdate {
                    status: model::NodeStatus::Completed,
                    output: Some(serde_json::json!({"ok": true})),
                    poll_trace: None,
                    failure: None,
                },
            )
            .unwrap();
    }
}

#[test]
fn native_datasource_reads_match_fixture_shape() {
    reset_host_for_tests();
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(1)));
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("t.db");
    seed_db(&db, clock.as_ref());

    let base = ensure_api(HostConfig {
        db_path: db.clone(),
        env_root: tmp.path().to_path_buf(),
    })
    .unwrap();
    assert!(base.starts_with("http://127.0.0.1:"));

    let dags = list_dags().unwrap();
    assert!(dags.iter().any(|d| d.id == "serial"));

    let active = get_active_dag("serial".into()).unwrap();
    assert_eq!(
        active.dag.get("id").and_then(|v| v.as_str()),
        Some("serial")
    );

    let runs = list_runs(Some("serial".into())).unwrap();
    assert!(!runs.is_empty());

    let run_id = runs[0].id.clone();
    let executions = list_executions(run_id).unwrap();
    assert_eq!(executions.len(), 3);
}
