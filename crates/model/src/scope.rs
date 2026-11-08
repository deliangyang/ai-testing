//! Which parts of a node are templates evaluated in the DAG's own scope.

use serde_json::{json, Map, Value};

use crate::nodes::HttpRequest;
use crate::template::{refs_in_value, LocatedRef, LocatedTemplateError};
use crate::{Node, NodeKind};

/// References in the node's own-scope templates, with JSON Pointers relative to the node
/// object (prefix with `/nodes/<index>` for a document pointer).
///
/// Subworkflow `outputs` are excluded: they are evaluated inside the child DAG.
pub fn node_refs(node: &Node) -> Result<Vec<LocatedRef>, LocatedTemplateError> {
    refs_in_value(&template_view(node))
}

/// A sparse copy of the node holding only template-bearing values at their original pointers.
fn template_view(node: &Node) -> Value {
    match &node.kind {
        NodeKind::Http(cfg) => request_view(&cfg.request()),
        NodeKind::Poll(cfg) => json!({
            "request": request_view(&cfg.request),
            "condition": { "expected": cfg.condition.expected },
        }),
        NodeKind::Assert(cfg) => json!({
            "assertions": cfg
                .assertions
                .iter()
                .map(|a| json!({ "expected": a.expected }))
                .collect::<Vec<_>>(),
        }),
        NodeKind::DbQuery(cfg) => json!({ "params": cfg.params }),
        NodeKind::Subworkflow(cfg) => json!({ "inputs": cfg.inputs }),
        NodeKind::Cleanup(cfg) => json!({
            "requests": cfg.requests.iter().map(request_view).collect::<Vec<_>>(),
        }),
    }
}

fn request_view(req: &HttpRequest) -> Value {
    let mut view = Map::new();
    view.insert("path".into(), Value::String(req.path.clone()));
    view.insert("query".into(), json!(req.query));
    view.insert("headers".into(), json!(req.headers));
    if let Some(body) = &req.body {
        view.insert("body".into(), body.clone());
    }
    Value::Object(view)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Dag;

    fn first_node(node: Value) -> Node {
        let doc = json!({"id": "d", "nodes": [node]});
        Dag::from_json_str(&doc.to_string())
            .unwrap()
            .nodes
            .remove(0)
    }

    fn summary(node: Value) -> Vec<(String, String)> {
        node_refs(&first_node(node))
            .unwrap()
            .into_iter()
            .map(|r| (r.pointer, r.var.to_string()))
            .collect()
    }

    #[test]
    fn http_templates_in_all_request_parts() {
        let refs = summary(json!({
            "id": "n", "type": "http", "method": "POST",
            "path": "/u/${a.output.body.id}",
            "query": {"q": "${env:Q}"},
            "headers": {"Authorization": "Bearer ${auth.token}"},
            "body": {"items": [{"sku": "${b.output.body.sku}"}]},
            "response_schema": {"description": "${not.output.scanned}"}
        }));
        assert_eq!(
            refs,
            vec![
                ("/body/items/0/sku".into(), "${b.output.body.sku}".into()),
                ("/headers/Authorization".into(), "${auth.token}".into()),
                ("/path".into(), "${a.output.body.id}".into()),
                ("/query/q".into(), "${env:Q}".into()),
            ]
        );
    }

    #[test]
    fn poll_request_and_condition_expected() {
        let refs = summary(json!({
            "id": "w", "type": "poll",
            "request": {"method": "GET", "path": "/o/${c.output.body.id}"},
            "condition": {"path": "$.body.id", "op": "eq", "expected": "${c.output.body.id}"},
            "backoff_ms": [1], "timeout_ms": 1
        }));
        assert_eq!(
            refs,
            vec![
                ("/condition/expected".into(), "${c.output.body.id}".into()),
                ("/request/path".into(), "${c.output.body.id}".into()),
            ]
        );
    }

    #[test]
    fn assert_db_query_subworkflow_and_cleanup() {
        assert_eq!(
            summary(
                json!({"id": "x", "type": "assert", "target": "c", "assertions": [
                    {"path": "$.status", "op": "eq", "expected": 200},
                    {"path": "$.body.id", "op": "eq", "expected": "${c.output.body.id}"}
                ]})
            ),
            vec![(
                "/assertions/1/expected".into(),
                "${c.output.body.id}".into()
            )]
        );
        assert_eq!(
            summary(
                json!({"id": "q", "type": "db_query", "connection": "c", "sql": "s",
                "params": ["${u.output.body.id}"]})
            ),
            vec![("/params/0".into(), "${u.output.body.id}".into())]
        );
        assert_eq!(
            summary(
                json!({"id": "s", "type": "subworkflow", "dag": {"id": "child"},
                "inputs": {"user": "${u.output.body.id}"},
                "outputs": {"token": "${login.output.body.token}"}})
            ),
            vec![("/inputs/user".into(), "${u.output.body.id}".into())]
        );
        assert_eq!(
            summary(json!({"id": "z", "type": "cleanup", "requests": [
                {"method": "DELETE", "path": "/o/${c.output.body.id}"}
            ]})),
            vec![("/requests/0/path".into(), "${c.output.body.id}".into())]
        );
    }

    #[test]
    fn invalid_template_is_located() {
        let err = node_refs(&first_node(json!({
            "id": "n", "type": "http", "method": "GET", "path": "/", "headers": {"X": "${oops"}
        })))
        .unwrap_err();
        assert_eq!(err.pointer, "/headers/X");
    }
}
