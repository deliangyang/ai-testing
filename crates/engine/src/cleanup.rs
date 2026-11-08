//! `cleanup` nodes: run every request in order; one failing does not stop the rest.
//!
//! A request fails on a template or transport error, or a status of 400 or above other than
//! 404/410 (the resource is already gone). Output `{results: [{index, status?, error?}]}`; if
//! any request failed the node fails with the first failure's kind and the results as detail.

use std::sync::Arc;

use model::{Node, NodeKind};
use serde_json::json;

use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use crate::http::HttpExecutor;

pub struct CleanupExecutor {
    http: Arc<HttpExecutor>,
}

impl CleanupExecutor {
    pub fn new(http: Arc<HttpExecutor>) -> Self {
        Self { http }
    }
}

impl NodeExecutor for CleanupExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            let NodeKind::Cleanup(cfg) = &node.kind else {
                return Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!(
                        "cleanup executor cannot run '{}' nodes",
                        node.kind.type_name()
                    ),
                ));
            };
            let mut results = Vec::with_capacity(cfg.requests.len());
            let mut first_failure: Option<ExecErrorKind> = None;
            let mut failures = 0usize;
            for (index, request) in cfg.requests.iter().enumerate() {
                let pointer = format!("/requests/{index}");
                let outcome = match self.http.send(request, &ctx, &pointer).await {
                    Ok(output) => {
                        let status = output["status"].as_u64().unwrap_or(0);
                        if status >= 400 && status != 404 && status != 410 {
                            Err((
                                ExecErrorKind::Assertion,
                                json!({"index": index, "status": status,
                                       "error": {"kind": ExecErrorKind::Assertion,
                                                 "message": format!("status {status}")}}),
                            ))
                        } else {
                            Ok(json!({"index": index, "status": status}))
                        }
                    }
                    Err(e) => Err((
                        e.kind,
                        json!({"index": index, "error": {"kind": e.kind, "message": e.message}}),
                    )),
                };
                match outcome {
                    Ok(result) => results.push(result),
                    Err((kind, result)) => {
                        failures += 1;
                        first_failure.get_or_insert(kind);
                        results.push(result);
                    }
                }
            }
            let output = json!({ "results": results });
            match first_failure {
                None => Ok(output.into()),
                Some(kind) => Err(ExecError::new(
                    kind,
                    format!(
                        "{failures} of {} cleanup requests failed",
                        cfg.requests.len()
                    ),
                )
                .with_detail(output)),
            }
        })
    }
}
