#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::{dag, dag_with_cleanup, MockExecutor};
use engine::NodeStatus::{self, Completed, Failed, Pending, Skipped};
use engine::{RunOutcome, Scheduler};
use model::Dag;

/// a -> {b, c} -> d -> e
fn diamond() -> Dag {
    dag(
        &["a", "b", "c", "d", "e"],
        &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d"), ("d", "e")],
    )
}

async fn run(exec: &Arc<MockExecutor>, dag: &Dag, parallel: usize) -> RunOutcome {
    Scheduler::new(exec.clone())
        .with_max_parallel(parallel)
        .run("run-1", dag)
        .await
        .unwrap()
}

fn statuses(outcome: &RunOutcome) -> Vec<(&str, NodeStatus)> {
    outcome
        .nodes
        .iter()
        .map(|(id, n)| (id.as_str(), n.status))
        .collect()
}

#[tokio::test]
async fn failing_branch_skips_the_join_and_everything_after_it() {
    for parallel in [1, 4] {
        let exec = Arc::new(MockExecutor::failing(&["b"]));
        let outcome = run(&exec, &diamond(), parallel).await;
        assert_eq!(
            statuses(&outcome),
            [
                ("a", Completed),
                ("b", Failed),
                ("c", Completed),
                ("d", Skipped),
                ("e", Skipped),
            ],
            "parallel {parallel}"
        );
        let mut called = exec.order();
        called.sort();
        assert_eq!(called, ["a", "b", "c"]);
        assert!(outcome.nodes["d"].output.is_none() && outcome.nodes["d"].error.is_none());
    }
}

#[tokio::test]
async fn failing_root_skips_every_descendant() {
    let exec = Arc::new(MockExecutor::failing(&["a"]));
    let outcome = run(&exec, &diamond(), 4).await;
    assert_eq!(exec.order(), ["a"]);
    assert_eq!(
        statuses(&outcome),
        [
            ("a", Failed),
            ("b", Skipped),
            ("c", Skipped),
            ("d", Skipped),
            ("e", Skipped),
        ]
    );
}

#[tokio::test]
async fn both_branches_failing_gives_the_same_report_at_any_parallelism() {
    let serial = Arc::new(MockExecutor::failing(&["b", "c"]));
    let parallel = Arc::new(MockExecutor::failing(&["b", "c"]));
    let one = run(&serial, &diamond(), 1).await;
    let four = run(&parallel, &diamond(), 4).await;
    assert_eq!(one.nodes["d"].status, Skipped);
    assert_eq!(one.report(), four.report());
}

#[tokio::test]
async fn unrelated_nodes_keep_running_after_a_failure() {
    let d = dag(&["a", "b", "x", "y"], &[("a", "b"), ("x", "y")]);
    let exec = Arc::new(MockExecutor::failing(&["a"]));
    let outcome = run(&exec, &d, 1).await;
    assert_eq!(
        statuses(&outcome),
        [
            ("a", Failed),
            ("b", Skipped),
            ("x", Completed),
            ("y", Completed),
        ]
    );
}

#[tokio::test]
async fn cleanup_downstream_of_a_failure_is_not_skipped() {
    let d = dag_with_cleanup(&["a", "b"], &["teardown"], &[("a", "b"), ("b", "teardown")]);
    let exec = Arc::new(MockExecutor::failing(&["a"]));
    let outcome = run(&exec, &d, 1).await;
    assert_eq!(outcome.nodes["b"].status, Skipped);
    assert_eq!(outcome.nodes["teardown"].status, Pending);
}

#[tokio::test]
async fn cleanup_after_success_runs_normally() {
    let d = dag_with_cleanup(&["a"], &["teardown"], &[("a", "teardown")]);
    let exec = Arc::new(MockExecutor::default());
    let outcome = run(&exec, &d, 1).await;
    assert!(outcome.succeeded());
    assert_eq!(exec.order(), ["a", "teardown"]);
}
