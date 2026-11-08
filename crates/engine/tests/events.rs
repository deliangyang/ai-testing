#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{dag, dag_with_cleanup, MockExecutor};
use engine::NodeStatus::{self, Completed, Failed, Polling, Ready, Running, Skipped};
use engine::{
    event_channel, CancelToken, DagSource, EngineEvent, Environment, ExecContext, ExecFuture,
    HttpExecutor, LogLevel, NodeExecutor, PollExecutor, Scheduler, SubworkflowExecutor,
};
use model::nodes::DagRef;
use model::{Dag, Node, NodeKind};
use serde_json::json;
use store::RunStatus;
use tokio::sync::mpsc::UnboundedReceiver;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn drain(rx: &mut UnboundedReceiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

fn state(node: &str, status: NodeStatus) -> EngineEvent {
    EngineEvent::NodeStateChanged {
        node: node.to_owned(),
        status,
    }
}

fn finished(status: RunStatus) -> EngineEvent {
    EngineEvent::RunFinished { status }
}

async fn run_with_events(
    executor: Arc<dyn NodeExecutor>,
    dag: &Dag,
    parallel: usize,
) -> Vec<EngineEvent> {
    let (sink, mut rx) = event_channel();
    Scheduler::new(executor)
        .with_max_parallel(parallel)
        .with_events(Some(sink))
        .run("run-1", dag)
        .await
        .unwrap();
    drain(&mut rx)
}

#[tokio::test]
async fn serial_run_emits_every_transition_in_order() {
    let dag = dag_with_cleanup(&["a", "b"], &["c"], &[("a", "b"), ("b", "c")]);
    let events = run_with_events(Arc::new(MockExecutor::default()), &dag, 1).await;
    assert_eq!(
        events,
        [
            state("a", Ready),
            state("a", Running),
            state("a", Completed),
            state("b", Ready),
            state("b", Running),
            state("b", Completed),
            state("c", Ready),
            state("c", Running),
            state("c", Completed),
            finished(RunStatus::Completed),
        ]
    );
}

#[tokio::test]
async fn a_failure_emits_the_skipped_descendants_before_cleanup() {
    let dag = dag_with_cleanup(&["a", "b"], &["c"], &[("a", "b"), ("b", "c")]);
    let events = run_with_events(Arc::new(MockExecutor::failing(&["a"])), &dag, 1).await;
    assert_eq!(
        events,
        [
            state("a", Ready),
            state("a", Running),
            state("a", Failed),
            state("b", Skipped),
            state("c", Ready),
            state("c", Running),
            state("c", Completed),
            finished(RunStatus::Failed),
        ]
    );
}

#[tokio::test]
async fn parallel_runs_keep_per_node_order_and_finish_last() {
    let dag = dag(
        &["a", "b", "c", "d", "e"],
        &[
            ("a", "b"),
            ("a", "c"),
            ("a", "d"),
            ("b", "e"),
            ("c", "e"),
            ("d", "e"),
        ],
    );
    let events = run_with_events(Arc::new(MockExecutor::default()), &dag, 4).await;

    assert_eq!(events.last(), Some(&finished(RunStatus::Completed)));
    let mut per_node: BTreeMap<String, Vec<NodeStatus>> = BTreeMap::new();
    for event in &events[..events.len() - 1] {
        let EngineEvent::NodeStateChanged { node, status } = event else {
            panic!("unexpected {event:?}");
        };
        per_node.entry(node.clone()).or_default().push(*status);
    }
    for (node, statuses) in &per_node {
        assert_eq!(statuses, &[Ready, Running, Completed], "{node}");
    }
    assert_eq!(per_node.len(), 5);
    let position = |node: &str, status| {
        events
            .iter()
            .position(|e| *e == state(node, status))
            .unwrap()
    };
    for mid in ["b", "c", "d"] {
        assert!(position("a", Completed) < position(mid, Ready));
        assert!(position(mid, Completed) < position("e", Ready));
    }
}

#[tokio::test]
async fn poll_nodes_report_polling_and_every_iteration() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orders/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "PENDING"})))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/orders/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "PAID"})))
        .with_priority(2)
        .mount(&server)
        .await;
    let clock = common_clock();
    let sleeper = ::common::ManualSleeper::new(clock.clone());
    let clock = Arc::new(clock);
    let http = Arc::new(HttpExecutor::new(clock.clone()).unwrap());
    let poll = Arc::new(PollExecutor::new(http, clock, Arc::new(sleeper)));
    let dag = Dag::from_json_str(
        &json!({"id": "p", "nodes": [{
            "id": "wait", "type": "poll",
            "request": {"method": "GET", "path": "/orders/1"},
            "condition": {"path": "$.body.status", "op": "eq", "expected": "PAID"},
            "backoff_ms": [100], "timeout_ms": 10_000
        }]})
        .to_string(),
    )
    .unwrap();
    let env = Environment::new("local", &server.uri(), BTreeMap::new()).unwrap();
    let (sink, mut rx) = event_channel();

    Scheduler::new(poll)
        .with_environment(env)
        .with_events(Some(sink))
        .run("run-1", &dag)
        .await
        .unwrap();

    let events = drain(&mut rx);
    let summary: Vec<String> = events
        .iter()
        .map(|e| match e {
            EngineEvent::NodeStateChanged { node, status } => format!("{node}:{status}"),
            EngineEvent::PollIteration {
                node,
                attempt,
                entry,
            } => format!("{node}:#{attempt}:{}", entry["actual"]),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        summary,
        [
            "wait:READY",
            "wait:RUNNING",
            "wait:POLLING",
            "wait:#1:\"PENDING\"",
            "wait:#2:\"PENDING\"",
            "wait:#3:\"PAID\"",
            "wait:COMPLETED",
            "RunFinished { status: Completed }",
        ]
    );
}

fn common_clock() -> ::common::FixedClock {
    ::common::FixedClock::new(::common::Timestamp::from_unix_millis(1_794_117_600_000))
}

struct OneDag(Dag);

impl DagSource for OneDag {
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String> {
        (dag.id == self.0.id)
            .then(|| (self.0.clone(), 1))
            .ok_or_else(|| format!("unknown DAG {}", dag.id))
    }
}

struct Router {
    sub: SubworkflowExecutor,
    leaf: MockExecutor,
}

impl NodeExecutor for Router {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        match node.kind {
            NodeKind::Subworkflow(_) => self.sub.execute(node, ctx),
            _ => self.leaf.execute(node, ctx),
        }
    }
}

#[tokio::test]
async fn subworkflow_events_use_frame_paths_and_finish_once() {
    let child = dag(&["x"], &[]);
    let child = Dag {
        id: "child".into(),
        ..child
    };
    let parent = Dag::from_json_str(
        &json!({"id": "parent", "nodes": [
            {"id": "s", "type": "subworkflow", "dag": {"id": "child"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let router = Router {
        sub: SubworkflowExecutor::new(Arc::new(OneDag(child))),
        leaf: MockExecutor::default(),
    };

    let events = run_with_events(Arc::new(router), &parent, 1).await;

    assert_eq!(
        events,
        [
            state("s", Ready),
            state("s", Running),
            state("s/x", Ready),
            state("s/x", Running),
            state("s/x", Completed),
            state("s", Completed),
            finished(RunStatus::Completed),
        ]
    );
}

struct CancelOn(&'static str);

impl NodeExecutor for CancelOn {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        let cancels = node.id == self.0;
        Box::pin(async move {
            if cancels {
                ctx.cancel.cancel();
                std::future::pending::<()>().await;
            }
            Ok(json!({}).into())
        })
    }
}

#[tokio::test]
async fn cancellation_is_logged_and_the_run_finishes_cancelled() {
    let dag = dag(&["a", "b"], &[("a", "b")]);
    let (sink, mut rx) = event_channel();
    Scheduler::new(Arc::new(CancelOn("a")))
        .with_cancel(CancelToken::new())
        .with_events(Some(sink))
        .run("run-1", &dag)
        .await
        .unwrap();

    let events = drain(&mut rx);
    assert_eq!(
        events,
        [
            state("a", Ready),
            state("a", Running),
            EngineEvent::Log {
                node: None,
                level: LogLevel::Warn,
                message: "run cancelled: interrupting running nodes, skipping the rest".into(),
            },
            state("b", Skipped),
            state("a", Skipped),
            finished(RunStatus::Cancelled),
        ]
    );
}

#[tokio::test]
async fn a_dropped_receiver_does_not_disturb_the_run() {
    let dag = dag(&["a", "b"], &[("a", "b")]);
    let (sink, rx) = event_channel();
    drop(rx);
    let outcome = Scheduler::new(Arc::new(MockExecutor::default()))
        .with_events(Some(sink))
        .run("run-1", &dag)
        .await
        .unwrap();
    assert!(outcome.succeeded());
}

#[test]
fn events_have_a_stable_json_shape() {
    let cases = [
        (
            state("s/x", Polling),
            json!({"type": "node_state_changed", "node": "s/x", "status": "POLLING"}),
        ),
        (
            EngineEvent::PollIteration {
                node: "wait".into(),
                attempt: 2,
                entry: json!({"passed": false}),
            },
            json!({"type": "poll_iteration", "node": "wait", "attempt": 2,
                   "entry": {"passed": false}}),
        ),
        (
            EngineEvent::Log {
                node: None,
                level: LogLevel::Error,
                message: "m".into(),
            },
            json!({"type": "log", "level": "error", "message": "m"}),
        ),
        (
            finished(RunStatus::Cancelled),
            json!({"type": "run_finished", "status": "CANCELLED"}),
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(serde_json::to_value(&event).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<EngineEvent>(expected).unwrap(),
            event
        );
    }
}
