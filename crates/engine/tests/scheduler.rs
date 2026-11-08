#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::{dag, keys, MockExecutor};
use engine::NodeStatus::{Completed, Failed, Pending};
use engine::{Scheduler, SchedulerError};
use serde_json::json;

async fn run(
    executor: &Arc<MockExecutor>,
    dag: &model::Dag,
) -> Result<engine::RunOutcome, SchedulerError> {
    Scheduler::new(executor.clone()).run("run-1", dag).await
}

#[tokio::test]
async fn runs_level_by_level_with_ids_ordered_within_a_level() {
    // login -> {create, profile}; create -> pay; audit is independent.
    let d = dag(
        &["pay", "profile", "audit", "create", "login"],
        &[("login", "create"), ("login", "profile"), ("create", "pay")],
    );
    let exec = Arc::new(MockExecutor::default());
    let outcome = run(&exec, &d).await.unwrap();

    let expected = ["audit", "login", "create", "profile", "pay"];
    assert_eq!(exec.order(), expected);
    assert_eq!(outcome.started, expected);
    assert!(outcome.succeeded());
    assert!(outcome.nodes.values().all(|n| n.status == Completed));
    assert_eq!(outcome.nodes["pay"].output, Some(json!({"from": "pay"})));
}

#[tokio::test]
async fn order_does_not_depend_on_declaration_order() {
    let edges = [("a", "c"), ("b", "c"), ("c", "d")];
    let first = Arc::new(MockExecutor::default());
    let second = Arc::new(MockExecutor::default());
    run(&first, &dag(&["a", "b", "c", "d"], &edges))
        .await
        .unwrap();
    run(&second, &dag(&["d", "c", "b", "a"], &edges))
        .await
        .unwrap();
    assert_eq!(first.order(), ["a", "b", "c", "d"]);
    assert_eq!(first.order(), second.order());
}

#[tokio::test]
async fn executors_see_exactly_their_ancestors_outputs() {
    let d = dag(
        &["a", "b", "c", "d", "x"],
        &[("a", "b"), ("b", "c"), ("x", "d")],
    );
    let exec = Arc::new(MockExecutor::default());
    run(&exec, &d).await.unwrap();

    let ctx = exec.context_of("c");
    assert_eq!(ctx.run_id, "run-1");
    assert_eq!(keys(&ctx.outputs), ["a", "b"]);
    assert_eq!(ctx.outputs["a"], json!({"from": "a"}));
    assert_eq!(keys(&exec.context_of("d").outputs), ["x"]);
    assert!(exec.context_of("a").outputs.is_empty());
}

#[tokio::test]
async fn nodes_downstream_of_a_failure_are_not_started() {
    let d = dag(&["a", "b", "c", "side"], &[("a", "b"), ("b", "c")]);
    let exec = Arc::new(MockExecutor::failing(&["b"]));
    let outcome = run(&exec, &d).await.unwrap();

    assert_eq!(exec.order(), ["a", "side", "b"]);
    assert!(!outcome.succeeded());
    assert_eq!(outcome.nodes["b"].status, Failed);
    assert_eq!(outcome.nodes["b"].output, None);
    assert_eq!(
        outcome.nodes["b"].error.as_ref().unwrap().message,
        "b failed"
    );
    assert_eq!(outcome.nodes["c"].status, Pending);
    assert_eq!(outcome.nodes["side"].status, Completed);
}

#[tokio::test]
async fn cyclic_dag_is_refused_before_running_anything() {
    let d = dag(&["a", "b"], &[("a", "b"), ("b", "a")]);
    let exec = Arc::new(MockExecutor::default());
    let err = run(&exec, &d).await.unwrap_err();
    assert_eq!(err, SchedulerError::Cycle(vec!["a".into(), "b".into()]));
    assert!(exec.order().is_empty());
}

#[tokio::test]
async fn empty_dag_succeeds_trivially() {
    let exec = Arc::new(MockExecutor::default());
    let outcome = run(&exec, &dag(&[], &[])).await.unwrap();
    assert!(outcome.started.is_empty());
    assert!(outcome.succeeded());
}
