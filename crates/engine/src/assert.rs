//! `assert` nodes: every assertion is evaluated and reported, passing or not.
//!
//! Output `{results: [ConditionResult...]}`. If any assertion fails the node fails with
//! [`ExecErrorKind::Assertion`] and the same `{results}` as error detail, plus
//! `response: {status, body_excerpt}` when the target is an HTTP output.

use model::{to_canonical_json, Node, NodeKind};
use serde_json::{json, Value};

use crate::condition::{evaluate, ConditionResult};
use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use crate::render::RenderError;

#[derive(Debug, Clone, Copy, Default)]
pub struct AssertExecutor;

impl NodeExecutor for AssertExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            let NodeKind::Assert(cfg) = &node.kind else {
                return Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!(
                        "assert executor cannot run '{}' nodes",
                        node.kind.type_name()
                    ),
                ));
            };
            let target = ctx.outputs.get(&cfg.target).ok_or_else(|| {
                ExecError::new(
                    ExecErrorKind::Template,
                    format!("assert target '{}' has no output in scope", cfg.target),
                )
                .with_detail(json!({"pointer": "/target", "node": cfg.target}))
            })?;

            let vars = ctx.vars();
            let mut results: Vec<ConditionResult> = Vec::with_capacity(cfg.assertions.len());
            for (i, assertion) in cfg.assertions.iter().enumerate() {
                let expected = match &assertion.expected {
                    Some(value) => {
                        Some(vars.render_value(value).map_err(|mut e: RenderError| {
                            e.pointer = format!("/assertions/{i}/expected{}", e.pointer);
                            ExecError::from(e)
                        })?)
                    }
                    None => None,
                };
                results.push(evaluate(assertion, expected.as_ref(), target));
            }

            let failed: Vec<&ConditionResult> = results.iter().filter(|r| !r.passed).collect();
            let message = failed.first().map(|first| {
                format!(
                    "{} of {} assertions failed; first: {}",
                    failed.len(),
                    results.len(),
                    describe(first)
                )
            });
            let mut output = json!({ "results": results });
            match message {
                None => Ok(output.into()),
                Some(message) => {
                    if let Some(response) = response_summary(target) {
                        output["response"] = response;
                    }
                    Err(ExecError::new(ExecErrorKind::Assertion, message).with_detail(output))
                }
            }
        })
    }
}

/// Characters of the target body kept in a failure detail.
pub const BODY_EXCERPT_CHARS: usize = 512;

/// `{status, body_excerpt}` of an HTTP-shaped target, for classifying the failure.
fn response_summary(target: &Value) -> Option<Value> {
    let status = target.get("status")?.as_u64()?;
    let body = match target.get("body") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => to_canonical_json(other).unwrap_or_default(),
    };
    let excerpt: String = body.chars().take(BODY_EXCERPT_CHARS).collect();
    Some(json!({"status": status, "body_excerpt": excerpt}))
}

fn describe(result: &ConditionResult) -> String {
    let op = serde_json::to_value(result.operator).unwrap_or(Value::Null);
    let op = op.as_str().unwrap_or_default();
    let mut text = format!("{} {op}", result.path);
    if let Some(expected) = &result.expected {
        text.push_str(&format!(" {expected}"));
    }
    match &result.actual {
        Some(actual) => text.push_str(&format!(" (actual {actual})")),
        None => text.push_str(" (no value at path)"),
    }
    if let Some(message) = &result.message {
        text.push_str(&format!(": {message}"));
    }
    text
}
