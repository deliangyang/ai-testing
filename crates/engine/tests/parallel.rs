#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::dag;
use engine::NodeStatus::{Completed, Failed, Skipped};
use engine::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor, Scheduler};
use model::{Dag, Node};
use serde_json::json;

/// Yields `delays[id]` times before finishing, so completion order can be steered without
/// real time; tracks how many nodes run at once.
#[derive(Default)]
struct Steered {
    delays: BTreeMap<String, usize>,
    fail: Vec<String>,
    running: AtomicUsize,
    max_running: AtomicUsize,
    finished: Mutex<Vec<String>>,
}

impl Steered {
    fn new(delays: &[(&str, usize)], fail: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            delays: delays.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
            fail: fail.iter().map(|s| (*s).to_owned()).collect(),
            ..Self::default()
        })
    }

    fn finished(&self) -> Vec<String> {
        self.finished.lock().unwrap().clone()
    }
}

impl NodeExecutor for Steered {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_running.fetch_max(now, Ordering::SeqCst);
            for _ in 0..self.delays.get(&node.id).copied().unwrap_or(0) {
                tokio::task::yield_now().await;
            }
            self.running.fetch_sub(1, Ordering::SeqCst);
            self.finished.lock().unwrap().push(node.id.clone());
            if self.fail.contains(&node.id) {
                return Err(ExecError::new(ExecErrorKind::Assertion, "boom"));
            }
            let seen: Vec<&String> = ctx.outputs.keys().collect();
            Ok(json!({"node": node.id, "seen": seen}))
        })
    }
}

async fn run(exec: &Arc<Steered>, dag: &Dag, parallel: usize) -> engine::RunOutcome {
    Scheduler::new(exec.clone())
        .with_max_parallel(parallel)
        .run("run-1", dag)
        .await
        .unwrap()
}

fn diamond() -> Dag {
    dag(
        &["a", "c", "d", "e"],
        &[("a", "c"), ("a", "d"), ("c", "e"), ("d", "e")],
    )
}

#[tokio::test]
async fn report_is_identical_for_parallelism_1_and_4() {
    let serial = Steered::new(&[("c", 50)], &[]);
    let parallel = Steered::new(&[("c", 50)], &[]);
    let one = run(&serial, &diamond(), 1).await;
    let four = run(&parallel, &diamond(), 4).await;

    // The runs really differed: c and d overlapped and finished in the other order.
    assert_eq!(serial.max_running.load(Ordering::SeqCst), 1);
    assert_eq!(parallel.max_running.load(Ordering::SeqCst), 2);
    assert_eq!(serial.finished(), ["a", "c", "d", "e"]);
    assert_eq!(parallel.finished(), ["a", "d", "c", "e"]);

    assert!(one.succeeded() && four.succeeded());
    assert_eq!(one.report(), four.report());
    assert_eq!(one.nodes, four.nodes);
    assert_eq!(
        one.nodes["e"].output,
        Some(json!({"node": "e", "seen": ["a", "c", "d"]}))
    );
}

#[tokio::test]
async fn parallelism_is_bounded() {
    let children: Vec<String> = (0..10).map(|i| format!("n{i}")).collect();
    let mut ids: Vec<&str> = vec!["root"];
    ids.extend(children.iter().map(String::as_str));
    let edges: Vec<(&str, &str)> = children.iter().map(|c| ("root", c.as_str())).collect();
    let delays: Vec<(&str, usize)> = children.iter().map(|c| (c.as_str(), 5)).collect();

    for (limit, expected_peak) in [(1, 1), (3, 3), (32, 10)] {
        let exec = Steered::new(&delays, &[]);
        let outcome = run(&exec, &dag(&ids, &edges), limit).await;
        assert!(outcome.succeeded());
        assert_eq!(
            exec.max_running.load(Ordering::SeqCst),
            expected_peak,
            "limit {limit}"
        );
    }
}

#[tokio::test]
async fn serial_start_order_matches_levels() {
    let exec = Steered::new(&[], &[]);
    let outcome = run(&exec, &dag(&["b", "a", "c"], &[("a", "b")]), 1).await;
    assert_eq!(outcome.started, ["a", "c", "b"]);
}

#[tokio::test]
async fn failure_in_one_branch_lets_the_running_sibling_finish() {
    for parallel in [1, 4] {
        let exec = Steered::new(&[("d", 20)], &["c"]);
        let outcome = run(&exec, &diamond(), parallel).await;
        assert_eq!(outcome.nodes["c"].status, Failed, "parallel {parallel}");
        assert_eq!(outcome.nodes["d"].status, Completed, "parallel {parallel}");
        assert_eq!(outcome.nodes["e"].status, Skipped, "parallel {parallel}");
        assert!(!exec.finished().contains(&"e".to_owned()));
    }
}

#[tokio::test]
async fn zero_parallelism_means_serial() {
    let exec = Steered::new(&[("c", 10)], &[]);
    run(&exec, &diamond(), 0).await;
    assert_eq!(exec.max_running.load(Ordering::SeqCst), 1);
}
