use model::nodes::{Condition, Operator, PollConfig};
use model::{parse_json_path, DagGraph, NodeKind, Template};
use serde_json::Value;

use crate::codes;
use crate::output::{output_shape, render_path, OutputShape};
use crate::schema_path::{lookup, Lookup};
use crate::{Context, Issue, Rule};

/// JSON types accepted by `type_is`.
pub const TYPE_NAMES: [&str; 7] = Operator::TYPE_NAMES;

/// Poll nodes and assertions must be mechanically decidable: every condition path is a
/// singular JSONPath that exists on the checked output, `expected` fits the operator, and a
/// poll has a non-empty, non-decreasing backoff that fits inside its timeout.
pub struct DecidabilityRule;

impl Rule for DecidabilityRule {
    fn name(&self) -> &'static str {
        "decidability"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let graph = DagGraph::new(ctx.dag);
        for (i, node) in ctx.dag.nodes.iter().enumerate() {
            let base = format!("/nodes/{i}");
            let mut found = Vec::new();
            match &node.kind {
                NodeKind::Poll(cfg) => {
                    check_backoff(cfg, &base, &mut found);
                    let shape = output_shape(node);
                    check_condition(
                        &cfg.condition,
                        Some(&shape),
                        &format!("{base}/condition"),
                        &mut found,
                    );
                }
                NodeKind::Assert(cfg) => {
                    // Unknown or non-ancestor targets are reported by ReferenceRule.
                    let shape = graph
                        .ancestors(&node.id)
                        .contains(cfg.target.as_str())
                        .then(|| ctx.dag.node(&cfg.target))
                        .flatten()
                        .map(output_shape);
                    for (j, cond) in cfg.assertions.iter().enumerate() {
                        let pointer = format!("{base}/assertions/{j}");
                        check_condition(cond, shape.as_ref(), &pointer, &mut found);
                    }
                }
                _ => {}
            }
            out.extend(found.into_iter().map(|issue| issue.at_node(&node.id)));
        }
    }
}

fn check_backoff(cfg: &PollConfig, base: &str, out: &mut Vec<Issue>) {
    let Some(&first) = cfg.backoff_ms.first() else {
        out.push(
            Issue::error(
                codes::POLL_EMPTY_BACKOFF,
                "backoff_ms must contain at least one interval",
            )
            .at_path(format!("{base}/backoff_ms")),
        );
        return;
    };
    if let Some(j) = (1..cfg.backoff_ms.len()).find(|&j| cfg.backoff_ms[j] < cfg.backoff_ms[j - 1])
    {
        out.push(
            Issue::error(
                codes::POLL_BACKOFF_DECREASING,
                format!(
                    "backoff_ms must be non-decreasing, but {} follows {}",
                    cfg.backoff_ms[j],
                    cfg.backoff_ms[j - 1]
                ),
            )
            .at_path(format!("{base}/backoff_ms/{j}")),
        );
    }
    if cfg.timeout_ms < first {
        out.push(
            Issue::error(
                codes::POLL_TIMEOUT_TOO_SHORT,
                format!(
                    "timeout_ms {} is shorter than the first backoff interval {first}",
                    cfg.timeout_ms
                ),
            )
            .at_path(format!("{base}/timeout_ms")),
        );
    }
    if cfg.max_attempts == Some(0) {
        out.push(
            Issue::error(
                codes::POLL_ZERO_MAX_ATTEMPTS,
                "max_attempts must be at least 1",
            )
            .at_path(format!("{base}/max_attempts")),
        );
    }
}

/// `shape` is the output the path is evaluated against, when known.
fn check_condition(
    cond: &Condition,
    shape: Option<&OutputShape>,
    base: &str,
    out: &mut Vec<Issue>,
) {
    match parse_json_path(&cond.path) {
        Err(err) => out.push(
            Issue::error(codes::CONDITION_INVALID_PATH, err.to_string())
                .at_path(format!("{base}/path")),
        ),
        Ok(path) => {
            if let Some(OutputShape::Schema(schema)) = shape {
                if let Lookup::Missing { segment, reason } = lookup(schema, &path) {
                    let end = segment.min(path.len().saturating_sub(1));
                    out.push(
                        Issue::error(
                            codes::CONDITION_UNKNOWN_FIELD,
                            format!(
                                "'{}' does not exist on the checked output ({reason}, at '{}')",
                                cond.path,
                                render_path("$", &path[..=end])
                            ),
                        )
                        .at_path(format!("{base}/path")),
                    );
                }
            }
        }
    }

    match (&cond.expected, cond.op.needs_expected()) {
        (None, true) => out.push(
            Issue::error(
                codes::CONDITION_MISSING_EXPECTED,
                format!("operator '{}' requires 'expected'", op_name(cond.op)),
            )
            .at_path(base.to_owned()),
        ),
        (Some(_), false) => out.push(
            Issue::error(
                codes::CONDITION_UNEXPECTED_EXPECTED,
                format!("operator '{}' takes no 'expected'", op_name(cond.op)),
            )
            .at_path(format!("{base}/expected")),
        ),
        (Some(expected), true) => {
            if let Err(message) = check_expected(cond.op, expected) {
                out.push(
                    Issue::error(codes::CONDITION_EXPECTED_TYPE, message)
                        .at_path(format!("{base}/expected")),
                );
            }
        }
        (None, false) => {}
    }
}

fn check_expected(op: Operator, expected: &Value) -> Result<(), String> {
    // A template is resolved at run time; its type cannot be checked here.
    if let Value::String(s) = expected {
        if Template::parse(s).is_ok_and(|t| t.refs().next().is_some()) {
            return Ok(());
        }
    }
    let name = op_name(op);
    match op {
        Operator::Gt | Operator::Gte | Operator::Lt | Operator::Lte => match expected {
            Value::Number(_) => Ok(()),
            other => Err(format!(
                "operator '{name}' needs a number, got {}",
                type_of(other)
            )),
        },
        Operator::LenEq => match expected.as_u64() {
            Some(_) => Ok(()),
            None => Err(format!(
                "operator '{name}' needs a non-negative integer, got {expected}"
            )),
        },
        Operator::Regex => match expected {
            Value::String(pattern) => regex::Regex::new(pattern)
                .map(|_| ())
                .map_err(|e| format!("invalid regex: {e}")),
            other => Err(format!(
                "operator '{name}' needs a string pattern, got {}",
                type_of(other)
            )),
        },
        Operator::TypeIs => match expected.as_str() {
            Some(t) if TYPE_NAMES.contains(&t) => Ok(()),
            _ => Err(format!(
                "operator '{name}' needs one of {}, got {expected}",
                TYPE_NAMES.join("/")
            )),
        },
        Operator::Eq
        | Operator::Ne
        | Operator::Contains
        | Operator::Exists
        | Operator::NotExists => Ok(()),
    }
}

fn op_name(op: Operator) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
