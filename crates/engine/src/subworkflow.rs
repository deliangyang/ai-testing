//! `subworkflow` nodes: run a stored child DAG in its own frame.
//!
//! The child sees only the rendered `inputs`, as the reserved node `input`, plus the
//! environment; parent outputs are invisible. The parent sees only `outputs`, rendered in the
//! child scope once the child succeeded. Child results are returned under their node ids and
//! the scheduler stores them as `<subworkflow node>/<child node>`, so nested frames give paths
//! like `checkout/login/token`.

use std::sync::{Arc, Mutex};

use common::Clock;
use model::nodes::{DagRef, SkillRef};
use model::{Dag, Node, NodeKind};
use serde_json::{json, Map, Value};
use store::{DagRepository, DagStatus, Db, SkillRepository};

use crate::executor::{
    ExecContext, ExecError, ExecErrorKind, ExecFuture, ExecOutput, Frame, NodeExecutor,
};
use crate::render::{RenderError, VarContext};
use crate::scheduler::{Scheduler, INPUT_NODE_ID};
use crate::state::NodeStatus;

/// What a subworkflow node loads: either a stored DAG or a Skill package's inner DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubworkflowTarget {
    Dag(DagRef),
    Skill(SkillRef),
}

impl SubworkflowTarget {
    pub fn from_config(cfg: &model::nodes::SubworkflowConfig) -> Result<Self, String> {
        cfg.validate_target()?;
        match (&cfg.dag, &cfg.skill) {
            (Some(d), None) => Ok(Self::Dag(d.clone())),
            (None, Some(s)) => Ok(Self::Skill(s.clone())),
            _ => unreachable!("validate_target"),
        }
    }
}

/// Loaded child: the DAG to run plus the store revision used for frame metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct SubworkflowLoad {
    pub dag: Dag,
    pub revision: u32,
    pub skill: Option<ResolvedSkill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSkill {
    pub id: String,
    pub revision: u32,
    pub manifest_version: String,
}

/// Where child DAGs come from.
pub trait DagSource: Send + Sync {
    /// The DAG and its resolved version. `version: None` means the active version.
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String>;

    /// Resolves a subworkflow target (default: DAG-only sources reject Skill refs).
    fn load_target(&self, target: &SubworkflowTarget) -> Result<SubworkflowLoad, String> {
        match target {
            SubworkflowTarget::Dag(dag) => {
                let (dag, revision) = self.load(dag)?;
                Ok(SubworkflowLoad {
                    dag,
                    revision,
                    skill: None,
                })
            }
            SubworkflowTarget::Skill(skill) => Err(format!(
                "skill '{}' is not supported by this subworkflow source",
                skill.id
            )),
        }
    }
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

    fn with_conn<T>(&self, f: impl FnOnce(&mut Db) -> Result<T, String>) -> Result<T, String> {
        let mut db = self
            .db
            .lock()
            .map_err(|_| "store lock poisoned".to_owned())?;
        f(&mut db)
    }
}

impl DagSource for StoreDagSource {
    fn load(&self, dag: &DagRef) -> Result<(Dag, u32), String> {
        self.with_conn(|db| {
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
        })
    }

    fn load_target(&self, target: &SubworkflowTarget) -> Result<SubworkflowLoad, String> {
        match target {
            SubworkflowTarget::Dag(dag) => {
                let (dag, revision) = self.load(dag)?;
                Ok(SubworkflowLoad {
                    dag,
                    revision,
                    skill: None,
                })
            }
            SubworkflowTarget::Skill(skill) => self.with_conn(|db| {
                let repo = SkillRepository::new(db.conn());
                let record = repo
                    .resolve(skill)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        if skill.version_req.is_none() {
                            format!("Skill '{}' has no active version", skill.id)
                        } else {
                            format!(
                                "no activated Skill '{}' matches version_req '{}'",
                                skill.id,
                                skill.version_req.as_deref().unwrap_or_default()
                            )
                        }
                    })?;
                Ok(SubworkflowLoad {
                    dag: record.package.dag.clone(),
                    revision: record.info.version,
                    skill: Some(ResolvedSkill {
                        id: record.info.id,
                        revision: record.info.version,
                        manifest_version: record.info.manifest_version,
                    }),
                })
            }),
        }
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
            let target = SubworkflowTarget::from_config(cfg)
                .map_err(|message| ExecError::new(ExecErrorKind::Internal, message))?;

            let vars = ctx.vars();
            let mut inputs = Map::new();
            for (name, value) in &cfg.inputs {
                let rendered = vars
                    .render_value(value)
                    .map_err(|e| located(e, &format!("/inputs/{}", escape(name))))?;
                inputs.insert(name.clone(), rendered);
            }

            let source = Arc::clone(&self.source);
            let target_for_load = target.clone();
            let loaded = tokio::task::spawn_blocking(move || source.load_target(&target_for_load))
                .await
                .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?
                .map_err(|message| {
                    ExecError::new(
                        ExecErrorKind::Internal,
                        format!("cannot load subworkflow target: {message}"),
                    )
                    .with_detail(load_error_detail(&target, &message))
                })?;

            let target_info = target_detail(&target, &loaded);
            let child = loaded.dag;
            let version = loaded.revision;

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
                .with_cancel(ctx.cancel.clone())
                .with_recorder(ctx.recorder.clone())
                .with_events(ctx.events.clone())
                .run(&ctx.run_id, &child)
                .await
                .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?;
            let dag_info = json!({
                "id": child.id,
                "version": version,
                "target": target_info,
            });

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
                if outcome.cancelled && ids.is_empty() {
                    let mut err = ExecError::new(
                        ExecErrorKind::Cancelled,
                        format!("subworkflow '{}' was cancelled", child.id),
                    )
                    .with_detail(json!({"dag": dag_info}));
                    err.children = outcome.nodes;
                    return Err(err);
                }
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

fn target_detail(target: &SubworkflowTarget, loaded: &SubworkflowLoad) -> Value {
    match target {
        SubworkflowTarget::Dag(dag) => json!({
            "kind": "dag",
            "id": dag.id,
            "pin": dag.version,
            "store_revision": loaded.revision,
        }),
        SubworkflowTarget::Skill(skill) => {
            let resolved = loaded.skill.as_ref();
            json!({
                "kind": "skill",
                "id": skill.id,
                "version_req": skill.version_req,
                "store_revision": loaded.revision,
                "manifest_version": resolved.map(|s| s.manifest_version.as_str()),
            })
        }
    }
}

fn load_error_detail(target: &SubworkflowTarget, _message: &str) -> Value {
    match target {
        SubworkflowTarget::Dag(dag) => json!({"dag": {"id": dag.id, "version": dag.version}}),
        SubworkflowTarget::Skill(skill) => json!({
            "skill": {
                "id": skill.id,
                "version_req": skill.version_req,
            }
        }),
    }
}

fn located(mut e: RenderError, pointer: &str) -> ExecError {
    e.pointer = format!("{pointer}{}", e.pointer);
    ExecError::from(e)
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
