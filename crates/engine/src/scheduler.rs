//! Topological scheduler with bounded parallelism.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use model::{to_canonical_json, Dag, DagGraph, Node, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use store::RunStatus;
use tokio::task::JoinSet;

use crate::cancel::CancelToken;
use crate::environment::Environment;
use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecOutput, Frame, NodeExecutor};
use crate::record::{NodeRecord, RecordError, RunRecorder};
use crate::state::{InvalidTransition, NodeStatus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeResult {
    pub status: NodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecError>,
    /// Poll iteration trace, whether the node completed or failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
}

impl NodeResult {
    fn pending() -> Self {
        Self {
            status: NodeStatus::Pending,
            output: None,
            error: None,
            trace: None,
        }
    }

    fn advance(&mut self, node: &str, to: NodeStatus) -> Result<(), SchedulerError> {
        self.status =
            self.status
                .transition(to)
                .map_err(|source| SchedulerError::IllegalTransition {
                    node: node.to_owned(),
                    source,
                })?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    /// Node ids in the order they were started. With parallelism above 1 this depends on
    /// completion timing, so it is diagnostic only and excluded from [`Self::report`].
    pub started: Vec<String>,
    /// Final state of every node, keyed by id.
    pub nodes: BTreeMap<String, NodeResult>,
    /// The cancellation token fired before every non-cleanup node had finished.
    pub cancelled: bool,
}

impl RunOutcome {
    pub fn succeeded(&self) -> bool {
        self.nodes
            .values()
            .all(|n| n.status == NodeStatus::Completed)
    }

    pub fn status(&self) -> RunStatus {
        if self.cancelled {
            RunStatus::Cancelled
        } else if self.succeeded() {
            RunStatus::Completed
        } else {
            RunStatus::Failed
        }
    }

    /// Canonical JSON of the per-node results; identical for any parallelism and completion
    /// order as long as executors are deterministic.
    pub fn report(&self) -> String {
        // Serializing plain data into a Value cannot fail.
        to_canonical_json(&self.nodes).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SchedulerError {
    /// Gate-approved DAGs are acyclic; this means an unchecked DAG reached the engine.
    #[error("DAG has a cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    #[error("node '{node}': {source}")]
    IllegalTransition {
        node: String,
        source: InvalidTransition,
    },
    /// An executor panicked or was aborted; that is a bug in the executor.
    #[error("executor task failed: {0}")]
    ExecutorCrashed(String),
    /// The run finished but its evidence is incomplete.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// Runs a DAG in topological order with at most `max_parallel` nodes in flight.
///
/// A node becomes `READY` once all its predecessors completed and is started in
/// (level, node id) order, levels as in [`DagGraph::topo_levels`]. With `max_parallel == 1`
/// this is a strict serial order. When a node fails, every descendant that has not run is
/// marked `SKIPPED`, except cleanup nodes.
///
/// Cleanup nodes are held back until every other node is terminal, then run one at a time in
/// (level, id) order whatever happened upstream. They see the outputs of whichever ancestors
/// completed.
///
/// Cancelling the token stops the main phase: nodes not yet started become `SKIPPED`, running
/// nodes are interrupted and become `SKIPPED` with a `cancelled` error, and the run is
/// `CANCELLED`. Subworkflow nodes are not interrupted but wind their child DAG down through
/// the same token, so the child's cleanup runs. Cleanup nodes run regardless.
pub struct Scheduler {
    executor: Arc<dyn NodeExecutor>,
    max_parallel: usize,
    env: Arc<Environment>,
    frames: Vec<Frame>,
    inputs: Option<Value>,
    cancel: CancelToken,
    recorder: Option<Arc<dyn RunRecorder>>,
}

impl Scheduler {
    /// Serial by default, with an empty environment.
    pub fn new(executor: Arc<dyn NodeExecutor>) -> Self {
        Self {
            executor,
            max_parallel: 1,
            env: Arc::new(Environment::default()),
            frames: Vec::new(),
            inputs: None,
            cancel: CancelToken::new(),
            recorder: None,
        }
    }

    /// Reports node evidence, masked with the environment, to `recorder`. A recording failure
    /// does not stop the run (cleanup must still happen) but makes [`Self::run`] fail.
    #[must_use]
    pub fn with_recorder(mut self, recorder: Option<Arc<dyn RunRecorder>>) -> Self {
        self.recorder = recorder;
        self
    }

    #[must_use]
    pub fn with_cancel(mut self, cancel: CancelToken) -> Self {
        self.cancel = cancel;
        self
    }

    #[must_use]
    pub fn with_environment(mut self, env: Environment) -> Self {
        self.env = Arc::new(env);
        self
    }

    #[must_use]
    pub fn with_shared_environment(mut self, env: Arc<Environment>) -> Self {
        self.env = env;
        self
    }

    /// Runs as a child DAG nested under `frames`.
    #[must_use]
    pub fn with_frames(mut self, frames: Vec<Frame>) -> Self {
        self.frames = frames;
        self
    }

    /// Visible to every node as the reserved `input` entry (`${input.output.<name>}`).
    #[must_use]
    pub fn with_inputs(mut self, inputs: Value) -> Self {
        self.inputs = Some(inputs);
        self
    }

    /// Values below 1 are treated as 1.
    #[must_use]
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel.max(1);
        self
    }

    pub async fn run(&self, run_id: &str, dag: &Dag) -> Result<RunOutcome, SchedulerError> {
        let graph = DagGraph::new(dag);
        let levels = graph.topo_levels().map_err(|cycle| {
            SchedulerError::Cycle(cycle.into_iter().map(str::to_owned).collect())
        })?;
        let level_of: BTreeMap<&str, usize> = levels
            .iter()
            .enumerate()
            .flat_map(|(level, ids)| ids.iter().map(move |id| (*id, level)))
            .collect();

        let mut nodes: BTreeMap<String, NodeResult> = level_of
            .keys()
            .map(|id| ((*id).to_owned(), NodeResult::pending()))
            .collect();
        let mut waiting_on: BTreeMap<&str, usize> = level_of
            .keys()
            .map(|id| (*id, graph.predecessors(id).count()))
            .collect();
        let cleanup: Vec<(usize, &str)> = level_of
            .iter()
            .filter(|(id, _)| dag.node(id).is_some_and(is_cleanup))
            .map(|(id, level)| (*level, *id))
            .collect();
        let mut ready: BTreeSet<(usize, &str)> = BTreeSet::new();
        for (id, _) in waiting_on.iter().filter(|(_, n)| **n == 0) {
            if !cleanup.iter().any(|(_, c)| c == id) {
                ready.insert((0, id));
            }
        }
        for &(_, id) in &ready {
            self.result(&mut nodes, id)?
                .advance(id, NodeStatus::Ready)?;
        }

        let mut journal = Journal {
            recorder: self.recorder.clone(),
            run_id: run_id.to_owned(),
            prefix: self.frames.iter().map(|f| format!("{}/", f.node)).collect(),
            env: Arc::clone(&self.env),
            error: None,
            finished: Vec::new(),
        };
        let mut started = Vec::new();
        let mut cancelled = false;
        let mut in_flight: JoinSet<(String, Result<ExecOutput, ExecError>)> = JoinSet::new();
        for phase in [Phase::Main, Phase::Cleanup] {
            let limit = match phase {
                Phase::Main => self.max_parallel,
                Phase::Cleanup => {
                    let mut queued: Vec<(usize, &str)> = cleanup.clone();
                    queued.sort();
                    for (level, id) in queued {
                        let entry = self.result(&mut nodes, id)?;
                        if entry.status == NodeStatus::Pending {
                            entry.advance(id, NodeStatus::Ready)?;
                            ready.insert((level, id));
                        }
                    }
                    1
                }
            };
            loop {
                if phase == Phase::Main && !cancelled && self.cancel.is_cancelled() {
                    cancelled = true;
                    ready.clear();
                    for (id, entry) in nodes.iter_mut() {
                        let Some(node) = dag.node(id) else { continue };
                        let unstarted =
                            matches!(entry.status, NodeStatus::Pending | NodeStatus::Ready);
                        if unstarted && !is_cleanup(node) {
                            entry.advance(id, NodeStatus::Skipped)?;
                            journal.finished.push(id.clone());
                        }
                    }
                }
                journal.flush(&nodes).await;
                while in_flight.len() < limit {
                    let Some((_, id)) = ready.pop_first() else {
                        break;
                    };
                    let Some(node) = dag.node(id) else { continue };
                    let mut outputs: BTreeMap<String, Value> = graph
                        .ancestors(id)
                        .into_iter()
                        .filter_map(|a| Some((a.to_owned(), nodes.get(a)?.output.clone()?)))
                        .collect();
                    if let Some(inputs) = &self.inputs {
                        outputs.insert(INPUT_NODE_ID.to_owned(), inputs.clone());
                    }
                    let ctx = ExecContext {
                        run_id: run_id.to_owned(),
                        outputs,
                        env: Arc::clone(&self.env),
                        frames: self.frames.clone(),
                        runner: Some(Arc::clone(&self.executor)),
                        cancel: self.cancel.clone(),
                        recorder: self.recorder.clone(),
                    };
                    self.result(&mut nodes, id)?
                        .advance(id, NodeStatus::Running)?;
                    started.push(id.to_owned());
                    if journal.recorder.is_some() {
                        let input = ctx.vars().resolve_node(node).ok();
                        journal.started(id, input).await;
                    }

                    let executor = Arc::clone(&self.executor);
                    let node = node.clone();
                    let interrupt = (phase == Phase::Main
                        && !matches!(node.kind, NodeKind::Subworkflow(_)))
                    .then(|| self.cancel.clone());
                    in_flight.spawn(async move {
                        let exec = executor.execute(&node, ctx);
                        let result = match interrupt {
                            Some(cancel) => tokio::select! {
                                biased;
                                () = cancel.cancelled() => Err(ExecError::new(
                                    ExecErrorKind::Cancelled,
                                    "interrupted by run cancellation",
                                )),
                                result = exec => result,
                            },
                            None => exec.await,
                        };
                        (node.id, result)
                    });
                }

                let joined = if phase == Phase::Main && !cancelled {
                    tokio::select! {
                        biased;
                        () = self.cancel.cancelled() => continue,
                        joined = in_flight.join_next() => joined,
                    }
                } else {
                    in_flight.join_next().await
                };
                let Some(joined) = joined else {
                    break;
                };
                let (id, result) =
                    joined.map_err(|e| SchedulerError::ExecutorCrashed(e.to_string()))?;
                match result {
                    Ok(done) => {
                        let entry = self.result(&mut nodes, &id)?;
                        entry.advance(&id, NodeStatus::Completed)?;
                        entry.output = Some(done.output);
                        entry.trace = done.trace;
                        journal.finished.push(id.clone());
                        for (child, result) in done.children {
                            nodes.insert(format!("{id}/{child}"), result);
                        }
                        if phase == Phase::Cleanup {
                            continue;
                        }
                        for succ in graph.successors(&id) {
                            let Some(count) = waiting_on.get_mut(succ) else {
                                continue;
                            };
                            *count -= 1;
                            let pending = nodes
                                .get(succ)
                                .is_some_and(|n| n.status == NodeStatus::Pending);
                            let deferred = dag.node(succ).is_some_and(is_cleanup);
                            if *count == 0 && pending && !deferred {
                                ready.insert((level_of.get(succ).copied().unwrap_or(0), succ));
                                self.result(&mut nodes, succ)?
                                    .advance(succ, NodeStatus::Ready)?;
                            }
                        }
                    }
                    Err(mut error) if error.kind == ExecErrorKind::Cancelled => {
                        let entry = self.result(&mut nodes, &id)?;
                        entry.advance(&id, NodeStatus::Skipped)?;
                        entry.trace = error.trace.take();
                        let children = std::mem::take(&mut error.children);
                        entry.error = Some(error);
                        journal.finished.push(id.clone());
                        for (child, result) in children {
                            nodes.insert(format!("{id}/{child}"), result);
                        }
                    }
                    Err(mut error) => {
                        let entry = self.result(&mut nodes, &id)?;
                        entry.advance(&id, NodeStatus::Failed)?;
                        entry.trace = error.trace.take();
                        let children = std::mem::take(&mut error.children);
                        entry.error = Some(error);
                        journal.finished.push(id.clone());
                        for (child, result) in children {
                            nodes.insert(format!("{id}/{child}"), result);
                        }
                        for desc in graph.descendants(&id) {
                            if dag.node(desc).is_some_and(is_cleanup) {
                                continue;
                            }
                            let entry = self.result(&mut nodes, desc)?;
                            if entry.status == NodeStatus::Pending {
                                entry.advance(desc, NodeStatus::Skipped)?;
                                journal.finished.push(desc.to_owned());
                            }
                        }
                    }
                }
                journal.flush(&nodes).await;
            }
        }
        journal.flush(&nodes).await;
        if let Some(error) = journal.error {
            return Err(error.into());
        }
        Ok(RunOutcome {
            started,
            nodes,
            cancelled,
        })
    }

    fn result<'n>(
        &self,
        nodes: &'n mut BTreeMap<String, NodeResult>,
        id: &str,
    ) -> Result<&'n mut NodeResult, SchedulerError> {
        nodes
            .get_mut(id)
            .ok_or_else(|| SchedulerError::ExecutorCrashed(format!("unknown node '{id}'")))
    }
}

/// Forwards one scheduler's evidence to the recorder, masked and under the frame path.
struct Journal {
    recorder: Option<Arc<dyn RunRecorder>>,
    run_id: String,
    /// `<subworkflow node>/` per enclosing frame.
    prefix: String,
    env: Arc<Environment>,
    /// The first recording failure; later records are dropped.
    error: Option<RecordError>,
    /// This DAG's nodes that became terminal and are not recorded yet.
    finished: Vec<String>,
}

impl Journal {
    async fn started(&mut self, id: &str, input: Option<Value>) {
        let record = NodeRecord::Started {
            node: format!("{}{id}", self.prefix),
            input: input.map(|v| self.env.mask(&v)),
        };
        self.send(record).await;
    }

    async fn flush(&mut self, nodes: &BTreeMap<String, NodeResult>) {
        for id in std::mem::take(&mut self.finished) {
            let Some(result) = nodes.get(&id) else {
                continue;
            };
            let record = NodeRecord::Finished {
                node: format!("{}{id}", self.prefix),
                result: self.mask(result),
            };
            self.send(record).await;
        }
    }

    fn mask(&self, result: &NodeResult) -> NodeResult {
        let mask = |v: &Value| self.env.mask(v);
        NodeResult {
            status: result.status,
            output: result.output.as_ref().map(mask),
            error: result.error.as_ref().map(|e| ExecError {
                message: match mask(&Value::String(e.message.clone())) {
                    Value::String(s) => s,
                    _ => e.message.clone(),
                },
                detail: e.detail.as_ref().map(mask),
                trace: e.trace.as_ref().map(mask),
                ..ExecError::new(e.kind, String::new())
            }),
            trace: result.trace.as_ref().map(mask),
        }
    }

    async fn send(&mut self, record: NodeRecord) {
        let Some(recorder) = self.recorder.clone() else {
            return;
        };
        if self.error.is_some() {
            return;
        }
        let run_id = self.run_id.clone();
        let outcome = tokio::task::spawn_blocking(move || recorder.record(&run_id, &record)).await;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => self.error = Some(e),
            Err(e) => self.error = Some(RecordError(e.to_string())),
        }
    }
}

/// Reserved id under which a child DAG sees its subworkflow inputs.
pub const INPUT_NODE_ID: &str = "input";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Main,
    /// After every other node is terminal; cleanup nodes run one at a time.
    Cleanup,
}

fn is_cleanup(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Cleanup(_))
}
