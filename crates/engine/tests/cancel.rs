#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use engine::NodeStatus::{Completed, Failed, Skipped};
use engine::{
    CancelToken, DagSource, ExecContext, ExecErrorKind, ExecFuture, NodeExecutor, NodeStatus,
    RunOutcome, Scheduler, SubworkflowExecutor,
};
use model::nodes::DagRef;
use model::{Dag, Node, NodeKind};
use serde_json::{json, Value};
use store::RunStatus;

use common::dag_with_cleanup;

/// Nodes in `cancel_on` cancel the run and then never finish; nodes in `hang` never finish;
/// nodes in `fail` fail; everything else completes. Subworkflow nodes go to `sub`.
#[derive(Default)]
struct Script {
    cancel_on: Vec<&'static str>,
    hang: Vec<&'static str>,
    fail: Vec<&'static str>,
    sub: Option<SubworkflowExecutor>,
    calls: Mutex<Vec<String>>,
}

impl Script {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl NodeExecutor for Script {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        if let (NodeKind::Subworkflow(_), Some(sub)) = (&node.kind, &self.sub) {
            return sub.execute(node, ctx);
        }
        self.calls.lock().unwrap().push(node.id.clone());
        let id = node.id.as_str();
        let cancels = self.cancel_on.contains(&id);
        let hangs = cancels || self.hang.contains(&id);
        let fails = self.fail.contains(&id);
        Box::pin(async move {
            if cancels {
                ctx.cancel.cancel();
            }
            if hangs {
                std::future::pending::<()>().await;
            }
            if fails {
                return Err(engine::ExecError::new(ExecErrorKind::Assertion, "boom"));
            }
            Ok(json!({"from": node.id}).into())
        })
    }
}

fn statuses(outcome: &RunOutcome) -> BTreeMap<&str, NodeStatus> {
    outcome
        .nodes
        .iter()
        .map(|(id, n)| (id.as_str(), n.status))
        .collect()
}

async fn run(script: Arc<Script>, dag: &Dag, cancel: CancelToken, parallel: usize) -> RunOutcome {
    Scheduler::new(script)
        .with_max_parallel(parallel)
        .with_cancel(cancel)
        .run("run-1", dag)
        .await
        .unwrap()
}

#[test]
fn token_clones_share_one_permanent_flag() {
    let token = CancelToken::new();
    let clone = token.clone();
    assert!(!clone.is_cancelled());
    token.cancel();
    assert!(clone.is_cancelled());
    token.cancel();
    assert!(clone.is_cancelled());
    assert!(format!("{clone:?}").contains("cancelled: true"));
}

#[tokio::test]
async fn cancelled_resolves_for_an_already_cancelled_token() {
    let token = CancelToken::new();
    token.cancel();
    token.cancelled().await;
}

#[tokio::test]
async fn cancelled_resolves_when_another_task_cancels() {
    let token = CancelToken::new();
    let waiter = tokio::spawn({
        let token = token.clone();
        async move { token.cancelled().await }
    });
    tokio::task::yield_now().await;
    token.cancel();
    waiter.await.unwrap();
}

#[tokio::test]
async fn cancelling_before_the_run_skips_everything_but_cleanup() {
    let dag = dag_with_cleanup(&["a", "b"], &["c"], &[("a", "b"), ("b", "c")]);
    let script = Arc::new(Script::default());
    let token = CancelToken::new();
    token.cancel();

    let outcome = run(Arc::clone(&script), &dag, token, 1).await;

    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([("a", Skipped), ("b", Skipped), ("c", Completed)])
    );
    assert_eq!(script.calls(), ["c"]);
    assert!(outcome.cancelled);
    assert_eq!(outcome.status(), RunStatus::Cancelled);
}

#[tokio::test]
async fn a_running_node_is_interrupted_and_cleanup_still_runs() {
    let dag = dag_with_cleanup(
        &["a", "b", "c"],
        &["z"],
        &[("a", "b"), ("b", "c"), ("c", "z")],
    );
    let script = Arc::new(Script {
        cancel_on: vec!["b"],
        ..Script::default()
    });

    let outcome = run(Arc::clone(&script), &dag, CancelToken::new(), 1).await;

    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([
            ("a", Completed),
            ("b", Skipped),
            ("c", Skipped),
            ("z", Completed)
        ])
    );
    let b = &outcome.nodes["b"];
    assert_eq!(b.error.as_ref().unwrap().kind, ExecErrorKind::Cancelled);
    assert!(outcome.nodes["c"].error.is_none());
    assert_eq!(outcome.started, ["a", "b", "z"]);
    assert_eq!(script.calls(), ["a", "b", "z"]);
    assert_eq!(outcome.status(), RunStatus::Cancelled);
    assert!(!outcome.succeeded());
}

#[tokio::test]
async fn every_parallel_node_in_flight_is_interrupted() {
    let dag = dag_with_cleanup(
        &["a", "b", "c", "d"],
        &["z"],
        &[("a", "d"), ("b", "d"), ("c", "d"), ("d", "z")],
    );
    let script = Arc::new(Script {
        cancel_on: vec!["c"],
        hang: vec!["a", "b"],
        ..Script::default()
    });

    let outcome = run(Arc::clone(&script), &dag, CancelToken::new(), 3).await;

    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([
            ("a", Skipped),
            ("b", Skipped),
            ("c", Skipped),
            ("d", Skipped),
            ("z", Completed)
        ])
    );
    for id in ["a", "b", "c"] {
        let kind = outcome.nodes[id].error.as_ref().unwrap().kind;
        assert_eq!(kind, ExecErrorKind::Cancelled, "{id}");
    }
    assert_eq!(outcome.status(), RunStatus::Cancelled);
}

#[tokio::test]
async fn a_failure_before_cancellation_stays_failed() {
    let dag = dag_with_cleanup(&["a", "b", "c"], &[], &[("a", "c"), ("b", "c")]);
    let script = Arc::new(Script {
        fail: vec!["a"],
        cancel_on: vec!["b"],
        ..Script::default()
    });

    let outcome = run(Arc::clone(&script), &dag, CancelToken::new(), 1).await;

    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([("a", Failed), ("b", Skipped), ("c", Skipped)])
    );
    assert_eq!(outcome.status(), RunStatus::Cancelled);
}

#[tokio::test]
async fn cancelling_during_cleanup_does_not_interrupt_it() {
    let dag = dag_with_cleanup(&["a"], &["y", "z"], &[("a", "y"), ("a", "z")]);
    let script = Arc::new(Script::default());
    let token = CancelToken::new();
    struct CancelInCleanup {
        inner: Arc<Script>,
    }
    impl NodeExecutor for CancelInCleanup {
        fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
            if node.id == "y" {
                ctx.cancel.cancel();
            }
            self.inner.execute(node, ctx)
        }
    }

    let outcome = Scheduler::new(Arc::new(CancelInCleanup {
        inner: Arc::clone(&script),
    }))
    .with_cancel(token.clone())
    .run("run-1", &dag)
    .await
    .unwrap();

    assert!(token.is_cancelled());
    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([("a", Completed), ("y", Completed), ("z", Completed)])
    );
    assert!(!outcome.cancelled);
    assert_eq!(outcome.status(), RunStatus::Completed);
}

#[tokio::test]
async fn run_status_without_cancellation() {
    let dag = dag_with_cleanup(&["a", "b"], &[], &[("a", "b")]);
    let ok = run(Arc::new(Script::default()), &dag, CancelToken::new(), 1).await;
    assert_eq!(ok.status(), RunStatus::Completed);
    assert!(!ok.cancelled);

    let failing = Arc::new(Script {
        fail: vec!["a"],
        ..Script::default()
    });
    let failed = run(failing, &dag, CancelToken::new(), 1).await;
    assert_eq!(failed.status(), RunStatus::Failed);
}

struct OneDag(Dag);

impl DagSource for OneDag {
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String> {
        if dag.id == self.0.id {
            Ok((self.0.clone(), 1))
        } else {
            Err(format!("unknown DAG {}", dag.id))
        }
    }
}

fn dag(value: Value) -> Dag {
    Dag::from_json_str(&value.to_string()).unwrap()
}

#[tokio::test]
async fn a_subworkflow_winds_down_its_child_and_runs_the_child_cleanup() {
    let child = dag(json!({
        "id": "child",
        "nodes": [
            {"id": "x", "type": "http", "method": "GET", "path": "/"},
            {"id": "after", "type": "http", "method": "GET", "path": "/"},
            {"id": "tidy", "type": "cleanup"}
        ],
        "edges": [{"from": "x", "to": "after"}, {"from": "after", "to": "tidy"}]
    }));
    let parent = dag(json!({
        "id": "parent",
        "nodes": [
            {"id": "s", "type": "subworkflow", "dag": {"id": "child"}},
            {"id": "next", "type": "http", "method": "GET", "path": "/"},
            {"id": "done", "type": "cleanup"}
        ],
        "edges": [{"from": "s", "to": "next"}, {"from": "next", "to": "done"}]
    }));
    let script = Arc::new(Script {
        cancel_on: vec!["x"],
        sub: Some(SubworkflowExecutor::new(Arc::new(OneDag(child)))),
        ..Script::default()
    });

    let outcome = run(Arc::clone(&script), &parent, CancelToken::new(), 1).await;

    assert_eq!(
        statuses(&outcome),
        BTreeMap::from([
            ("done", Completed),
            ("next", Skipped),
            ("s", Skipped),
            ("s/after", Skipped),
            ("s/tidy", Completed),
            ("s/x", Skipped),
        ])
    );
    let s = outcome.nodes["s"].error.as_ref().unwrap();
    assert_eq!(s.kind, ExecErrorKind::Cancelled);
    assert_eq!(script.calls(), ["x", "tidy", "done"]);
    assert_eq!(outcome.status(), RunStatus::Cancelled);
}
