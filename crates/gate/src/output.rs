//! Declared output shape of each node kind, as a JSON Schema.
//!
//! http/poll output is `{status, headers, body, timing}` with `body` described by
//! `response_schema`; db_query output is `{rows}`, `{row}` or `{value}` with `response_schema`
//! describing one row; subworkflow output keys are its `outputs`.

use model::nodes::{HttpConfig, QueryResultMode};
use model::{Node, NodeKind, PathSeg};
use serde_json::{json, Value};

use crate::rules::ApiSpecLookup;

pub(crate) enum OutputShape {
    Schema(Value),
    Unverified { reason: String },
}

fn http_body_schema(cfg: &HttpConfig, api_specs: Option<&dyn ApiSpecLookup>) -> Option<Value> {
    if let Some(id) = cfg.api_spec_id.as_deref() {
        if let Some(lookup) = api_specs {
            if let Some(schema) = lookup.response_schema(id) {
                return Some(schema);
            }
        }
    }
    cfg.response_schema.clone()
}

pub(crate) fn output_shape(node: &Node, api_specs: Option<&dyn ApiSpecLookup>) -> OutputShape {
    let http_like = |schema: Option<Value>, node_id: &str| match schema {
        Some(body) => OutputShape::Schema(http_output(body)),
        None => OutputShape::Unverified {
            reason: format!("'{node_id}' declares no response_schema"),
        },
    };
    match &node.kind {
        NodeKind::Http(cfg) => http_like(http_body_schema(cfg, api_specs), &node.id),
        NodeKind::Poll(cfg) => http_like(cfg.response_schema.clone(), &node.id),
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
    }
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
pub(crate) fn path_reaches_body(path: &[PathSeg]) -> bool {
    match path.first() {
        Some(PathSeg::Key(k)) if k == "body" => path.len() > 1,
        Some(PathSeg::Key(k)) => !matches!(k.as_str(), "status" | "headers" | "timing"),
        _ => true,
    }
}

/// `output.a[0].b`-style rendering of a path prefix, for messages.
pub(crate) fn render_path(root: &str, path: &[PathSeg]) -> String {
    let mut out = String::from(root);
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
