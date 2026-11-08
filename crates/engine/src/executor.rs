//! The contract between the scheduler and node-type implementations.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use model::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ExecFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, ExecError>> + Send + 'a>>;

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ExecError {
    pub kind: ExecErrorKind,
    pub message: String,
    /// Structured evidence for the classifier and the report (e.g. HTTP status, diff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

impl ExecError {
    pub fn new(kind: ExecErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// Everything a node may read while executing.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecContext {
    pub run_id: String,
    /// Outputs of the node's completed ancestors, and only those, keyed by node id.
    pub outputs: BTreeMap<String, Value>,
}

pub trait NodeExecutor: Send + Sync {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a>;
}
