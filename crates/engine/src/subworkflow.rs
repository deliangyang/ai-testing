//! `subworkflow` nodes: run a stored child DAG in its own frame.
//!
//! The child sees only the rendered `inputs`, as the reserved node `input`, plus the
//! environment; parent outputs are invisible. The parent sees only `outputs`, rendered in the
//! child scope once the child succeeded. Child results are returned under their node ids and
//! the scheduler stores them as `<subworkflow node>/<child node>`, so nested frames give paths
//! like `checkout/login/token`.

use std::sync::{Arc, Mutex};

use common::Clock;
use model::nodes::DagRef;
use model::{Dag, Node, NodeKind};
use serde_json::{json, Map, Value};
use store::{DagRepository, DagStatus, Db};

use crate::executor::{
    ExecContext, ExecError, ExecErrorKind, ExecFuture, ExecOutput, Frame, NodeExecutor,
};
use crate::render::{RenderError, VarContext};
use crate::scheduler::{Scheduler, INPUT_NODE_ID};
use crate::state::NodeStatus;

/// Where child DAGs come from.
pub trait DagSource: Send + Sync {
    /// The DAG and its resolved version. `version: None` means the active version.
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String>;
}

/// Loads child DAGs from the store. A pinned version must have been activated at some point
/// (`active` or `archived`): pending versions never run.
pub struct StoreDagSource {
    db: Arc<Mutex<Db>>,
    clock: Arc<dyn Clock>,
}

impl StoreDagSource {
    pub fn new(db: Arc<Mutex<Db>>, clock: Arc<dyn Clock>) -> Self {
        Self { db, clock }
    }
}

impl DagSource for StoreDagSource {
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String> {
        let db = self
            .db
            .lock()
            .map_err(|_| "store lock poisoned".to_owned())?;
        let repo = DagRepository::new(db.conn(), self.clock.as_ref());
        let record = match dag.version {
            None => repo
                .active(&dag.id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("DAG '{}' has no active version", dag.id))?,
            Some(version) => {
                let record = repo
                    .get(&dag.id, version)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("DAG '{}' has no version {version}", dag.id))?;
                if record.info.status == DagStatus::Pending {
                    return Err(format!(
                        "DAG '{}' version {version} is pending and has never been activated",
                        dag.id
                    ));
                }
                record
            }
        };
        Ok((record.dag, record.info.version))
    }
}

pub struct SubworkflowExecutor {
    source: Arc<dyn DagSource>,
    max_parallel: usize,
}

impl SubworkflowExecutor {
    /// Child DAGs run serially unless [`Self::with_max_parallel`] says otherwise.
    pub fn new(source: Arc<dyn DagSource>) -> Self {
        Self {
            source,
            max_parallel: 1,
        }
    }

    #[must_use]
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel;
        self
    }
}

impl NodeExecutor for SubworkflowExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            let NodeKind::Subworkflow(cfg) = &node.kind else {
                return Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!(
                        "subworkflow executor cannot run '{}' nodes",
                        node.kind.type_name()
                    ),
                ));
            };
            let runner = ctx.runner.clone().ok_or_else(|| {
                ExecError::new(ExecErrorKind::Internal, "no runner for the child DAG")
            })?;

            let vars = ctx.vars();
            let mut inputs = Map::new();
            for (name, value) in &cfg.inputs {
                let rendered = vars
                    .render_value(value)
                    .map_err(|e| located(e, &format!("/inputs/{}", escape(name))))?;
                inputs.insert(name.clone(), rendered);
            }

            let source = Arc::clone(&self.source);
            let dag_ref = cfg.dag.clone();
            let (child, version) = tokio::task::spawn_blocking(move || source.load(&dag_ref))
                .await
                .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?
                .map_err(|message| {
                    ExecError::new(
                        ExecErrorKind::Internal,
                        format!("cannot load subworkflow DAG: {message}"),
                    )
                    .with_detail(json!({"dag": cfg.dag.id, "version": cfg.dag.version}))
                })?;

            if ctx.frames.iter().any(|f| f.dag == child.id) {
                let mut chain: Vec<&str> = ctx.frames.iter().map(|f| f.dag.as_str()).collect();
                chain.push(&child.id);
                return Err(ExecError::new(
                    ExecErrorKind::Internal,
                    format!("subworkflow cycle: {}", chain.join(" -> ")),
                ));
            }
            let mut frames = ctx.frames.clone();
            frames.push(Frame {
                node: node.id.clone(),
                dag: child.id.clone(),
                version,
            });

            let inputs = Value::Object(inputs);
            let outcome = Scheduler::new(runner)
                .with_max_parallel(self.max_parallel)
                .with_shared_environment(Arc::clone(&ctx.env))
                .with_frames(frames)
                .with_inputs(inputs.clone())
                .run(&ctx.run_id, &child)
                .await
                .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?;
            let dag_info = json!({"id": child.id, "version": version});

            if !outcome.succeeded() {
                let failed: Vec<(&String, Option<ExecErrorKind>)> = outcome
                    .nodes
                    .iter()
                    .filter(|(id, n)| n.status == NodeStatus::Failed && !id.contains('/'))
                    .map(|(id, n)| (id, n.error.as_ref().map(|e| e.kind)))
                    .collect();
                let kind = failed
                    .iter()
                    .find_map(|(_, kind)| *kind)
                    .unwrap_or(ExecErrorKind::Internal);
                let ids: Vec<&str> = failed.iter().map(|(id, _)| id.as_str()).collect();
                let mut err = ExecError::new(
                    kind,
                    format!(
                        "subworkflow '{}' failed at {}",
                        child.id,
                        if ids.is_empty() {
                            "an unfinished node".to_owned()
                        } else {
                            ids.join(", ")
                        }
                    ),
                )
                .with_detail(json!({"dag": dag_info, "failed": ids}));
                err.children = outcome.nodes;
                return Err(err);
            }

            let mut scope: std::collections::BTreeMap<String, Value> = outcome
                .nodes
                .iter()
                .filter(|(id, _)| !id.contains('/'))
                .filter_map(|(id, n)| Some((id.clone(), n.output.clone()?)))
                .collect();
            scope.insert(INPUT_NODE_ID.to_owned(), inputs);
            let child_vars = VarContext {
                env: &ctx.env,
                outputs: &scope,
            };
            let mut output = Map::new();
            for (name, template) in &cfg.outputs {
                match child_vars.render_str(template) {
                    Ok(value) => {
                        output.insert(name.clone(), value);
                    }
                    Err(e) => {
                        let mut err = located(e, &format!("/outputs/{}", escape(name)));
                        err.children = outcome.nodes;
                        return Err(err);
                    }
                }
            }
            Ok(ExecOutput {
                output: Value::Object(output),
                trace: None,
                children: outcome.nodes,
            })
        })
    }
}

fn located(mut e: RenderError, pointer: &str) -> ExecError {
    e.pointer = format!("{pointer}{}", e.pointer);
    ExecError::from(e)
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
