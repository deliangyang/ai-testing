//! Dispatch from node type to executor.

use std::sync::Arc;

use common::clock::{Clock, Sleeper};
use model::{Node, NodeKind};

use crate::assert::AssertExecutor;
use crate::cleanup::CleanupExecutor;
use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use crate::http::HttpExecutor;
use crate::poll::PollExecutor;
use crate::subworkflow::SubworkflowExecutor;

/// The executor a run hands to the scheduler: every node type goes to its own executor.
/// Node types without one (`db_query` until it has an executor, `subworkflow` unless enabled)
/// fail with [`ExecErrorKind::Unsupported`].
pub struct NodeRouter {
    http: Arc<HttpExecutor>,
    poll: PollExecutor,
    cleanup: CleanupExecutor,
    subworkflow: Option<SubworkflowExecutor>,
}

impl NodeRouter {
    pub fn new(http: Arc<HttpExecutor>, clock: Arc<dyn Clock>, sleeper: Arc<dyn Sleeper>) -> Self {
        Self {
            poll: PollExecutor::new(Arc::clone(&http), clock, sleeper),
            cleanup: CleanupExecutor::new(Arc::clone(&http)),
            http,
            subworkflow: None,
        }
    }

    #[must_use]
    pub fn with_subworkflows(mut self, executor: SubworkflowExecutor) -> Self {
        self.subworkflow = Some(executor);
        self
    }
}

impl NodeExecutor for NodeRouter {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        match &node.kind {
            NodeKind::Http(_) => self.http.execute(node, ctx),
            NodeKind::Poll(_) => self.poll.execute(node, ctx),
            NodeKind::Assert(_) => AssertExecutor.execute(node, ctx),
            NodeKind::Cleanup(_) => self.cleanup.execute(node, ctx),
            NodeKind::Subworkflow(_) => match &self.subworkflow {
                Some(sub) => sub.execute(node, ctx),
                None => unsupported(node),
            },
            NodeKind::DbQuery(_) => unsupported(node),
        }
    }
}

fn unsupported(node: &Node) -> ExecFuture<'_> {
    let message = format!(
        "no executor for node type '{}' (node '{}')",
        node.kind.type_name(),
        node.id
    );
    Box::pin(async move { Err(ExecError::new(ExecErrorKind::Unsupported, message)) })
}
