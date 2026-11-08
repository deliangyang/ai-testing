#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::MockExecutor;
use engine::Scheduler;
use serde_json::json;

#[tokio::test]
async fn upstream_nodes_are_not_executed_again() {
    let d = common::dag(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
    let exec = Arc::new(MockExecutor::default());
    let first = Scheduler::new(exec.clone()).run("run-1", &d).await.unwrap();
    exec.calls.lock().unwrap().clear();

    let outcome = Scheduler::new(exec.clone())
        .with_partial_rerun("b", first.nodes)
        .run("run-2", &d)
        .await
        .unwrap();

    assert_eq!(exec.order(), ["b", "c"]);
    assert!(outcome.succeeded());
    assert_eq!(outcome.nodes["a"].output, Some(json!({"from": "a"})));
    assert_eq!(outcome.nodes["b"].output, Some(json!({"from": "b"})));
}

#[tokio::test]
async fn parallel_branches_outside_the_rerun_subtree_stay_seeded() {
    let d = common::dag(
        &["login", "create", "profile"],
        &[("login", "create"), ("login", "profile")],
    );
    let exec = Arc::new(MockExecutor::default());
    let first = Scheduler::new(exec.clone()).run("run-1", &d).await.unwrap();
    exec.calls.lock().unwrap().clear();

    let outcome = Scheduler::new(exec.clone())
        .with_partial_rerun("create", first.nodes)
        .run("run-2", &d)
        .await
        .unwrap();

    assert_eq!(exec.order(), ["create"]);
    assert_eq!(
        outcome.nodes["profile"].output,
        Some(json!({"from": "profile"}))
    );
    assert!(!exec
        .order()
        .iter()
        .any(|id| id == "login" || id == "profile"));
}
