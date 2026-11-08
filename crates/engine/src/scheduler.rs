//! Topological scheduler with bounded parallelism.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use model::{to_canonical_json, Dag, DagGraph, Node, NodeKind};
use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinSet;

use crate::environment::Environment;
use crate::executor::{ExecContext, ExecError, ExecOutput, NodeExecutor};
use crate::state::{InvalidTransition, NodeStatus};

#[derive(Debug, Clone, PartialEq, Serialize)]
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
}

impl RunOutcome {
    pub fn succeeded(&self) -> bool {
        self.nodes
            .values()
            .all(|n| n.status == NodeStatus::Completed)
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
}

/// Runs a DAG in topological order with at most `max_parallel` nodes in flight.
///
/// A node becomes `READY` once all its predecessors completed and is started in
/// (level, node id) order, levels as in [`DagGraph::topo_levels`]. With `max_parallel == 1`
/// this is a strict serial order. When a node fails, every descendant that has not run is
/// marked `SKIPPED`, except cleanup nodes, which stay `PENDING` here.
pub struct Scheduler {
    executor: Arc<dyn NodeExecutor>,
    max_parallel: usize,
    env: Arc<Environment>,
}

impl Scheduler {
    /// Serial by default, with an empty environment.
    pub fn new(executor: Arc<dyn NodeExecutor>) -> Self {
        Self {
            executor,
            max_parallel: 1,
            env: Arc::new(Environment::default()),
        }
    }

    #[must_use]
    pub fn with_environment(mut self, env: Environment) -> Self {
        self.env = Arc::new(env);
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
        let mut ready: BTreeSet<(usize, &str)> = BTreeSet::new();
        for (id, _) in waiting_on.iter().filter(|(_, n)| **n == 0) {
            ready.insert((0, id));
        }
        for &(_, id) in &ready {
            self.result(&mut nodes, id)?
                .advance(id, NodeStatus::Ready)?;
        }

        let mut started = Vec::new();
        let mut in_flight: JoinSet<(String, Result<ExecOutput, ExecError>)> = JoinSet::new();
        loop {
            while in_flight.len() < self.max_parallel {
                let Some((_, id)) = ready.pop_first() else {
                    break;
                };
                let Some(node) = dag.node(id) else { continue };
                let ctx = ExecContext {
                    run_id: run_id.to_owned(),
                    outputs: graph
                        .ancestors(id)
                        .into_iter()
                        .filter_map(|a| Some((a.to_owned(), nodes.get(a)?.output.clone()?)))
                        .collect(),
                    env: Arc::clone(&self.env),
                };
                self.result(&mut nodes, id)?
                    .advance(id, NodeStatus::Running)?;
                started.push(id.to_owned());

                let executor = Arc::clone(&self.executor);
                let node = node.clone();
                in_flight.spawn(async move {
                    let result = executor.execute(&node, ctx).await;
                    (node.id, result)
                });
            }

            let Some(joined) = in_flight.join_next().await else {
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
                    for succ in graph.successors(&id) {
                        let Some(count) = waiting_on.get_mut(succ) else {
                            continue;
                        };
                        *count -= 1;
                        let pending = nodes
                            .get(succ)
                            .is_some_and(|n| n.status == NodeStatus::Pending);
                        if *count == 0 && pending {
                            ready.insert((level_of.get(succ).copied().unwrap_or(0), succ));
                            self.result(&mut nodes, succ)?
                                .advance(succ, NodeStatus::Ready)?;
                        }
                    }
                }
                Err(mut error) => {
                    let entry = self.result(&mut nodes, &id)?;
                    entry.advance(&id, NodeStatus::Failed)?;
                    entry.trace = error.trace.take();
                    entry.error = Some(error);
                    for desc in graph.descendants(&id) {
                        if dag.node(desc).is_some_and(is_cleanup) {
                            continue;
                        }
                        let entry = self.result(&mut nodes, desc)?;
                        if entry.status == NodeStatus::Pending {
                            entry.advance(desc, NodeStatus::Skipped)?;
                        }
                    }
                }
            }
        }
        Ok(RunOutcome { started, nodes })
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

fn is_cleanup(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Cleanup(_))
}
