//! The contract between the scheduler and node-type implementations.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use model::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cancel::CancelToken;
use crate::environment::Environment;
use crate::events::{EngineEvent, EventSink};
use crate::record::RunRecorder;
use crate::render::VarContext;
use crate::scheduler::NodeResult;

pub type ExecFuture<'a> = Pin<Box<dyn Future<Output = Result<ExecOutput, ExecError>> + Send + 'a>>;

/// A successful execution: the node output plus, for poll nodes, the iteration trace that is
/// stored separately from the output.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExecOutput {
    pub output: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
    /// Results of a subworkflow's child nodes keyed by their frame-relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, NodeResult>,
}

impl From<Value> for ExecOutput {
    fn from(output: Value) -> Self {
        Self {
            output,
            ..Self::default()
        }
    }
}

/// One level of subworkflow nesting: the parent's subworkflow node and the child DAG it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub node: String,
    pub dag: String,
    pub version: u32,
}

/// What went wrong, as reported by an executor. Executors only describe the raw failure;
/// mapping it to a failure category and error code is the classifier's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecErrorKind {
    /// A template could not be rendered (missing reference, bad path).
    Template,
    /// Connection refused, DNS failure, TLS error...
    Transport,
    Timeout,
    /// A check on the response did not hold.
    Assertion,
    /// A value could not be extracted from a response.
    Extraction,
    /// The executor does not handle this node type.
    Unsupported,
    Internal,
    /// The run was cancelled while the node executed. The scheduler records the node as
    /// `SKIPPED`, never `FAILED`, so this kind is never classified.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ExecError {
    pub kind: ExecErrorKind,
    pub message: String,
    /// Structured evidence for the classifier and the report (e.g. HTTP status, diff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
    /// Poll iterations up to the failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
    /// Child node results of a failed subworkflow.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, NodeResult>,
}

impl ExecError {
    pub fn new(kind: ExecErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
            trace: None,
            children: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }

    #[must_use]
    pub fn with_trace(mut self, trace: Value) -> Self {
        self.trace = Some(trace);
        self
    }
}

/// Everything a node may read while executing.
#[derive(Clone, Default)]
pub struct ExecContext {
    pub run_id: String,
    /// Outputs of the node's completed ancestors, and only those, keyed by node id. Inside a
    /// subworkflow this also holds the reserved `input` entry.
    pub outputs: BTreeMap<String, Value>,
    pub env: Arc<Environment>,
    /// Subworkflow nesting, outermost first; empty at the top level.
    pub frames: Vec<Frame>,
    /// The executor running this DAG, used by subworkflow nodes to run their child DAG.
    pub runner: Option<Arc<dyn NodeExecutor>>,
    /// The run's cancellation token; subworkflow nodes hand it to their child DAG.
    pub cancel: CancelToken,
    /// Where the run's evidence goes; subworkflow nodes hand it to their child DAG.
    pub recorder: Option<Arc<dyn RunRecorder>>,
    /// Live progress events; subworkflow nodes hand it to their child DAG.
    pub events: Option<EventSink>,
}

impl fmt::Debug for ExecContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecContext")
            .field("run_id", &self.run_id)
            .field("outputs", &self.outputs)
            .field("env", &self.env)
            .field("frames", &self.frames)
            .field("runner", &self.runner.is_some())
            .field("cancel", &self.cancel)
            .field("recorder", &self.recorder.is_some())
            .field("events", &self.events.is_some())
            .finish()
    }
}

impl ExecContext {
    /// `id` prefixed with the enclosing subworkflow nodes, as used in evidence and events.
    pub fn node_path(&self, id: &str) -> String {
        let mut path: String = self.frames.iter().map(|f| format!("{}/", f.node)).collect();
        path.push_str(id);
        path
    }

    /// Callers mask environment values in the event themselves.
    pub fn emit(&self, event: EngineEvent) {
        if let Some(events) = &self.events {
            events.emit(event);
        }
    }

    pub fn vars(&self) -> VarContext<'_> {
        VarContext {
            env: &self.env,
            outputs: &self.outputs,
        }
    }
}

pub trait NodeExecutor: Send + Sync {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a>;
}
