//! Serial topological scheduler.

use std::collections::BTreeMap;
use std::sync::Arc;

use model::{Dag, DagGraph};
use serde_json::Value;

use crate::executor::{ExecContext, ExecError, NodeExecutor};
use crate::state::{InvalidTransition, NodeStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct NodeResult {
    pub status: NodeStatus,
    pub output: Option<Value>,
    pub error: Option<ExecError>,
}

impl NodeResult {
    fn pending() -> Self {
        Self {
            status: NodeStatus::Pending,
            output: None,
            error: None,
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
    /// Node ids in the order they were started.
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
}

/// Runs nodes one at a time in topological order: level by level (see
/// [`DagGraph::topo_levels`]), by node id within a level.
///
/// A node starts only when all its predecessors completed; nodes downstream of a failure stay
/// `PENDING`.
pub struct Scheduler {
    executor: Arc<dyn NodeExecutor>,
}

impl Scheduler {
    pub fn new(executor: Arc<dyn NodeExecutor>) -> Self {
        Self { executor }
    }

    pub async fn run(&self, run_id: &str, dag: &Dag) -> Result<RunOutcome, SchedulerError> {
        let graph = DagGraph::new(dag);
        let levels = graph.topo_levels().map_err(|cycle| {
            SchedulerError::Cycle(cycle.into_iter().map(str::to_owned).collect())
        })?;

        let mut nodes: BTreeMap<String, NodeResult> = levels
            .iter()
            .flatten()
            .map(|id| ((*id).to_owned(), NodeResult::pending()))
            .collect();
        let mut started = Vec::new();

        for id in levels.into_iter().flatten() {
            let ready = graph
                .predecessors(id)
                .all(|p| nodes.get(p).map(|n| n.status) == Some(NodeStatus::Completed));
            let Some(node) = dag.node(id) else { continue };
            if !ready {
                continue;
            }
            let ctx = ExecContext {
                run_id: run_id.to_owned(),
                outputs: graph
                    .ancestors(id)
                    .into_iter()
                    .filter_map(|a| {
                        let output = nodes.get(a)?.output.clone()?;
                        Some((a.to_owned(), output))
                    })
                    .collect(),
            };

            let Some(result) = nodes.get_mut(id) else {
                continue;
            };
            result.advance(id, NodeStatus::Ready)?;
            result.advance(id, NodeStatus::Running)?;
            started.push(id.to_owned());

            match self.executor.execute(node, ctx).await {
                Ok(output) => {
                    result.advance(id, NodeStatus::Completed)?;
                    result.output = Some(output);
                }
                Err(error) => {
                    result.advance(id, NodeStatus::Failed)?;
                    result.error = Some(error);
                }
            }
        }
        Ok(RunOutcome { started, nodes })
    }
}
