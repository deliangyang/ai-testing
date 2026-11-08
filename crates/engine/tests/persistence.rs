#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use common::{FixedClock, SeqIdGen, Timestamp};
use engine::NodeStatus::{Completed, Failed, Skipped};
use engine::{
    load_results, CancelToken, DagSource, Environment, ExecContext, ExecError, ExecErrorKind,
    ExecFuture, ExecOutput, NodeExecutor, NodeRecord, RecordError, RunOutcome, RunRecorder,
    Scheduler, SchedulerError, StoreRecorder, SubworkflowExecutor, Unclassified,
};
use model::nodes::DagRef;
use model::{Dag, Node, NodeKind};
use serde_json::{json, Value};
use store::{
    DagRepository, Db, ExecutionKey, Failure, NewRun, NodeExecutionRepository, RunMode,
    RunRepository, RunStatus,
};

/// Outputs `{"from": id, "input": <resolved node>}`. Nodes in `fail` fail with a detail,
/// nodes in `traced` attach a poll trace, nodes in `cancel_on` cancel the run and hang.
#[derive(Default)]
struct Script {
    fail: Vec<&'static str>,
    traced: Vec<&'static str>,
    cancel_on: Vec<&'static str>,
    sub: Option<SubworkflowExecutor>,
    calls: Mutex<Vec<String>>,
}

impl NodeExecutor for Script {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        if let (NodeKind::Subworkflow(_), Some(sub)) = (&node.kind, &self.sub) {
            return sub.execute(node, ctx);
        }
        self.calls.lock().unwrap().push(node.id.clone());
        let id = node.id.as_str();
        let (fails, traced, cancels) = (
            self.fail.contains(&id),
            self.traced.contains(&id),
            self.cancel_on.contains(&id),
        );
        Box::pin(async move {
            if cancels {
                ctx.cancel.cancel();
                std::future::pending::<()>().await;
            }
            let input = ctx.vars().resolve_node(node)?;
            let trace = traced.then(|| json!([{"attempt": 1, "passed": !fails}]));
            if fails {
                let mut err = ExecError::new(ExecErrorKind::Assertion, format!("{id} failed"))
                    .with_detail(json!({"expected": 200, "actual": 500, "input": input}));
                err.trace = trace;
                return Err(err);
            }
            Ok(ExecOutput {
                output: json!({"from": node.id, "input": input}),
                trace,
                ..ExecOutput::default()
            })
        })
    }
}

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(Timestamp::from_unix_millis(
        1_794_117_600_000,
    )))
}

fn dag(value: Value) -> Dag {
    Dag::from_json_str(&value.to_string()).unwrap()
}

fn http(id: &str, path: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

struct Fixture {
    db: Arc<Mutex<Db>>,
    clock: Arc<FixedClock>,
    ids: SeqIdGen,
    recorder: Arc<StoreRecorder>,
}

impl Fixture {
    fn new(dags: &[&Dag]) -> Self {
        let clock = clock();
        let db = Db::open_in_memory(clock.as_ref()).unwrap();
        {
            let repo = DagRepository::new(db.conn(), clock.as_ref());
            for d in dags {
                repo.save(d, "test").unwrap();
            }
        }
        let db = Arc::new(Mutex::new(db));
        let recorder = Arc::new(StoreRecorder::new(
            Arc::clone(&db),
            clock.clone(),
            Arc::new(Unclassified),
        ));
        Self {
            db,
            clock,
            ids: SeqIdGen::new("run"),
            recorder,
        }
    }

    async fn run(&self, scheduler: Scheduler, dag: &Dag) -> (String, RunOutcome) {
        let run = self
            .recorder
            .begin_run(
                &NewRun {
                    dag_id: dag.id.clone(),
                    dag_version: 1,
                    env_name: "test".into(),
                    mode: RunMode::Ci,
                },
                &self.ids,
            )
            .unwrap();
        let recorder: Arc<dyn RunRecorder> = self.recorder.clone();
        let outcome = scheduler
            .with_recorder(Some(recorder))
            .run(&run.id, dag)
            .await
            .unwrap();
        self.recorder
            .finish_run(&run.id, &outcome, &self.ids)
            .unwrap();
        (run.id, outcome)
    }

    fn results(&self, run_id: &str) -> BTreeMap<String, engine::NodeResult> {
        load_results(&self.db.lock().unwrap(), self.clock.as_ref(), run_id).unwrap()
    }

    fn row(&self, run_id: &str, node: &str) -> store::NodeExecution {
        let db = self.db.lock().unwrap();
        NodeExecutionRepository::new(db.conn(), self.clock.as_ref())
            .get(&ExecutionKey::new(run_id, node, 1))
            .unwrap()
            .unwrap()
    }

    fn run_status(&self, run_id: &str) -> RunStatus {
        let db = self.db.lock().unwrap();
        RunRepository::new(db.conn(), self.clock.as_ref(), &self.ids)
            .get(run_id)
            .unwrap()
            .unwrap()
            .status
    }
}

/// a -> b(fails) -> c ; a -> d ; cleanup z after everything.
fn mixed() -> Dag {
    dag(json!({
        "id": "mixed",
        "nodes": [
            http("a", "/a"),
            http("b", "/b/${a.output.from}"),
            http("c", "/c/${b.output.from}"),
            http("d", "/d/${a.output.from}"),
            {"id": "z", "type": "cleanup"}
        ],
        "edges": [
            {"from": "a", "to": "b"}, {"from": "b", "to": "c"}, {"from": "a", "to": "d"},
            {"from": "c", "to": "z"}, {"from": "d", "to": "z"}
        ]
    }))
}

#[tokio::test]
async fn every_node_result_can_be_rebuilt_from_the_store() {
    let dag = mixed();
    let f = Fixture::new(&[&dag]);
    let script = Arc::new(Script {
        fail: vec!["b"],
        traced: vec!["b", "d"],
        ..Script::default()
    });

    let (run_id, outcome) = f.run(Scheduler::new(script), &dag).await;

    let statuses: BTreeMap<&str, _> = outcome
        .nodes
        .iter()
        .map(|(id, n)| (id.as_str(), n.status))
        .collect();
    assert_eq!(
        statuses,
        BTreeMap::from([
            ("a", Completed),
            ("b", Failed),
            ("c", Skipped),
            ("d", Completed),
            ("z", Completed)
        ])
    );
    assert_eq!(f.results(&run_id), outcome.nodes);
    assert_eq!(f.run_status(&run_id), RunStatus::Failed);
}

#[tokio::test]
async fn rows_hold_resolved_input_trace_and_classification() {
    let dag = mixed();
    let f = Fixture::new(&[&dag]);
    let script = Arc::new(Script {
        fail: vec!["b"],
        traced: vec!["b"],
        ..Script::default()
    });

    let (run_id, _) = f.run(Scheduler::new(script), &dag).await;

    let b = f.row(&run_id, "b");
    assert_eq!(b.status, Failed);
    assert_eq!(b.input_resolved.as_ref().unwrap()["path"], "/b/a");
    assert_eq!(
        b.failure,
        Some(Failure {
            category: "unknown".into(),
            error_code: "UNCLASSIFIED".into()
        })
    );
    assert_eq!(b.poll_trace, Some(json!([{"attempt": 1, "passed": false}])));
    assert_eq!(b.output.as_ref().unwrap()["kind"], "assertion");
    assert!(b.started_at.is_some() && b.finished_at.is_some());

    let c = f.row(&run_id, "c");
    assert_eq!(c.status, Skipped);
    assert_eq!(
        (c.input_resolved, c.output, c.started_at),
        (None, None, None)
    );
    assert!(c.finished_at.is_some());

    let a = f.row(&run_id, "a");
    assert_eq!(a.output.unwrap()["from"], "a");
    assert!(a.failure.is_none());
}

#[tokio::test]
async fn evidence_does_not_depend_on_parallelism() {
    let dag = mixed();
    let f = Fixture::new(&[&dag]);
    let mut runs = Vec::new();
    for parallel in [1, 4] {
        let script = Arc::new(Script {
            fail: vec!["b"],
            ..Script::default()
        });
        let scheduler = Scheduler::new(script).with_max_parallel(parallel);
        runs.push(f.run(scheduler, &dag).await.0);
    }
    assert_eq!(f.results(&runs[0]), f.results(&runs[1]));
}

#[tokio::test]
async fn process_environment_values_are_masked() {
    let dag = dag(json!({
        "id": "secret",
        "nodes": [http("login", "/login?key=${env:API_KEY}&tenant=${env:TENANT}")]
    }));
    let f = Fixture::new(&[&dag]);
    let process = BTreeMap::from([("AITEST_KEY".to_owned(), "s3cr3t-value".to_owned())]);
    let env = Environment::parse(
        "test",
        "base_url: http://sut\nvariables:\n  API_KEY: ${env:AITEST_KEY}\n  TENANT: acme\n",
        &process,
    )
    .unwrap();
    let script = Arc::new(Script::default());

    let (run_id, outcome) = f
        .run(Scheduler::new(script).with_environment(env), &dag)
        .await;

    let in_memory = outcome.nodes["login"].output.as_ref().unwrap().to_string();
    assert!(in_memory.contains("s3cr3t-value"));
    let row = f.row(&run_id, "login");
    assert_eq!(
        row.input_resolved.unwrap()["path"],
        "/login?key=****&tenant=acme"
    );
    let stored = row.output.unwrap().to_string();
    assert!(!stored.contains("s3cr3t"), "{stored}");
    assert!(stored.contains("****"));
}

struct OneDag(Dag);

impl DagSource for OneDag {
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String> {
        (dag.id == self.0.id)
            .then(|| (self.0.clone(), 1))
            .ok_or_else(|| format!("unknown DAG {}", dag.id))
    }
}

#[tokio::test]
async fn subworkflow_nodes_are_recorded_under_their_frame_path() {
    let child = dag(json!({
        "id": "child",
        "nodes": [http("x", "/x/${input.output.user}"), http("y", "/y/${x.output.from}")],
        "edges": [{"from": "x", "to": "y"}]
    }));
    let parent = dag(json!({
        "id": "parent",
        "nodes": [
            {"id": "s", "type": "subworkflow", "dag": {"id": "child"},
             "inputs": {"user": "bob"}, "outputs": {"last": "${y.output.from}"}},
            http("after", "/after/${s.output.last}")
        ],
        "edges": [{"from": "s", "to": "after"}]
    }));
    let f = Fixture::new(&[&parent]);
    let script = Arc::new(Script {
        sub: Some(SubworkflowExecutor::new(Arc::new(OneDag(child)))),
        ..Script::default()
    });

    let (run_id, outcome) = f.run(Scheduler::new(script), &parent).await;

    assert!(outcome.succeeded());
    let stored = f.results(&run_id);
    assert_eq!(
        stored.keys().map(String::as_str).collect::<Vec<_>>(),
        ["after", "s", "s/x", "s/y"]
    );
    assert_eq!(stored, outcome.nodes);
    assert_eq!(
        f.row(&run_id, "s/x").input_resolved.unwrap()["path"],
        "/x/bob"
    );
    assert_eq!(
        f.row(&run_id, "s").input_resolved.unwrap()["inputs"],
        json!({"user": "bob"})
    );
    assert_eq!(f.run_status(&run_id), RunStatus::Completed);
}

#[tokio::test]
async fn a_cancelled_run_is_stored_as_cancelled() {
    let dag = mixed();
    let f = Fixture::new(&[&dag]);
    let script = Arc::new(Script {
        cancel_on: vec!["b"],
        ..Script::default()
    });

    let (run_id, outcome) = f
        .run(Scheduler::new(script).with_cancel(CancelToken::new()), &dag)
        .await;

    assert_eq!(f.run_status(&run_id), RunStatus::Cancelled);
    assert_eq!(f.results(&run_id), outcome.nodes);
    let b = f.row(&run_id, "b");
    assert_eq!(b.status, Skipped);
    assert!(b.started_at.is_some());
    assert_eq!(b.output.unwrap()["kind"], "cancelled");
    assert_eq!(f.row(&run_id, "z").status, Completed);
}

struct Broken {
    seen: Mutex<BTreeSet<String>>,
}

impl RunRecorder for Broken {
    fn record(&self, _run_id: &str, record: &NodeRecord) -> Result<(), RecordError> {
        let node = match record {
            NodeRecord::Started { node, .. } | NodeRecord::Finished { node, .. } => node,
        };
        self.seen.lock().unwrap().insert(node.clone());
        Err(RecordError("disk full".into()))
    }
}

#[tokio::test]
async fn a_recording_failure_fails_the_run_after_cleanup() {
    let dag = mixed();
    let script = Arc::new(Script::default());
    let broken = Arc::new(Broken {
        seen: Mutex::new(BTreeSet::new()),
    });
    let recorder: Arc<dyn RunRecorder> = broken.clone();

    let err = Scheduler::new(Arc::clone(&script) as Arc<dyn NodeExecutor>)
        .with_recorder(Some(recorder))
        .run("run-1", &dag)
        .await
        .unwrap_err();

    assert_eq!(err, SchedulerError::Record(RecordError("disk full".into())));
    assert_eq!(broken.seen.lock().unwrap().len(), 1);
    assert_eq!(*script.calls.lock().unwrap(), ["a", "b", "d", "c", "z"]);
}
