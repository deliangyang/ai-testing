use model::nodes::QueryResultMode;
use model::scope::node_refs;
use model::{Dag, DagGraph, NodeKind, PathSeg, VarRef};
use serde_json::{json, Value};

use crate::codes;
use crate::schema_path::{lookup, Lookup};
use crate::{Context, Issue, Rule};

/// Referenced output fields must exist in the upstream node's declared output shape.
///
/// http/poll output is `{status, headers, body, timing}` with `body` described by
/// `response_schema`; db_query output is `{rows}`, `{row}` or `{value}` with `response_schema`
/// describing one row; subworkflow output keys are its `outputs`.
pub struct FieldRule;

impl Rule for FieldRule {
    fn name(&self) -> &'static str {
        "fields"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let graph = DagGraph::new(ctx.dag);
        for (i, node) in ctx.dag.nodes.iter().enumerate() {
            let Ok(refs) = node_refs(node) else { continue };
            let ancestors = graph.ancestors(&node.id);
            for located in refs {
                let VarRef::Node { node: target, path } = &located.var else {
                    continue;
                };
                // Bad targets are already reported by ReferenceRule.
                if !ancestors.contains(target.as_str()) {
                    continue;
                }
                let Some(shape) = output_shape(ctx.dag, target) else {
                    continue;
                };
                let pointer = format!("/nodes/{i}{}", located.pointer);
                let issue = match shape {
                    OutputShape::Schema(schema) => match lookup(&schema, path) {
                        Lookup::Found => None,
                        Lookup::Missing { segment, reason } => Some(Issue::error(
                            codes::UNKNOWN_FIELD,
                            format!(
                                "{} does not exist on '{target}' output ({reason}, at '{}')",
                                located.var,
                                render_path(&path[..=segment.min(path.len().saturating_sub(1))])
                            ),
                        )),
                    },
                    OutputShape::Unverified { reason } if path_reaches_body(path) => {
                        Some(Issue::warn(
                            codes::UNVERIFIED_FIELD,
                            format!("{} cannot be checked: {reason}", located.var),
                        ))
                    }
                    OutputShape::Unverified { .. } => None,
                };
                if let Some(issue) = issue {
                    out.push(issue.at_node(&node.id).at_path(pointer));
                }
            }
        }
    }
}

enum OutputShape {
    Schema(Value),
    Unverified { reason: String },
}

fn output_shape(dag: &Dag, target: &str) -> Option<OutputShape> {
    let node = dag.node(target)?;
    let http_like = |schema: &Option<Value>| match schema {
        Some(body) => OutputShape::Schema(http_output(body.clone())),
        None => OutputShape::Unverified {
            reason: format!("'{target}' declares no response_schema"),
        },
    };
    Some(match &node.kind {
        NodeKind::Http(cfg) => http_like(&cfg.response_schema),
        NodeKind::Poll(cfg) => http_like(&cfg.response_schema),
        NodeKind::DbQuery(cfg) => {
            let row = cfg.response_schema.clone().unwrap_or(json!({}));
            let (key, value) = match cfg.result {
                QueryResultMode::Rows => ("rows", json!({"type": "array", "items": row})),
                QueryResultMode::FirstRow => ("row", row),
                QueryResultMode::Scalar => ("value", json!({})),
            };
            OutputShape::Schema(closed_object(vec![(key, value)]))
        }
        NodeKind::Subworkflow(cfg) => OutputShape::Schema(closed_object(
            cfg.outputs
                .keys()
                .map(|k| (k.as_str(), json!({})))
                .collect(),
        )),
        NodeKind::Assert(_) => OutputShape::Schema(closed_object(vec![("results", json!({}))])),
        NodeKind::Cleanup(_) => OutputShape::Schema(closed_object(vec![])),
    })
}

fn http_output(body: Value) -> Value {
    closed_object(vec![
        ("status", json!({"type": "integer"})),
        (
            "headers",
            json!({"type": "object", "additionalProperties": {"type": "string"}}),
        ),
        ("body", body),
        ("timing", json!({})),
    ])
}

fn closed_object(props: Vec<(&str, Value)>) -> Value {
    let properties: serde_json::Map<String, Value> =
        props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    json!({"type": "object", "properties": properties, "additionalProperties": false})
}

/// Without a schema only `status`/`headers`/`timing` are known; anything under `body` is not.
fn path_reaches_body(path: &[PathSeg]) -> bool {
    match path.first() {
        Some(PathSeg::Key(k)) if k == "body" => path.len() > 1,
        Some(PathSeg::Key(k)) => !matches!(k.as_str(), "status" | "headers" | "timing"),
        _ => true,
    }
}

fn render_path(path: &[PathSeg]) -> String {
    let mut out = String::from("output");
    for seg in path {
        match seg {
            PathSeg::Key(k) => {
                out.push('.');
                out.push_str(k);
            }
            PathSeg::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}
