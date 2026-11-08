#![allow(clippy::unwrap_used, clippy::expect_used)]

use model::nodes::HttpMethod;
use model::{Dag, ModelError, NodeKind};
use serde_json::json;

fn single_node(yaml_node: &str) -> Result<NodeKind, ModelError> {
    let indented: String = yaml_node
        .lines()
        .map(|l| format!("    {l}\n"))
        .collect::<String>()
        .replacen("    ", "  - ", 1);
    let doc = format!("id: d\nnodes:\n{indented}");
    Dag::from_yaml_str(&doc).map(|mut dag| dag.nodes.remove(0).kind)
}

fn error_message(result: Result<NodeKind, ModelError>) -> String {
    match result.expect_err("expected error") {
        ModelError::Parse { message, .. } => message,
        other => panic!("unexpected error: {other}"),
    }
}

mod http {
    use super::*;

    const FULL: &str = r#"id: create
type: http
method: POST
path: /orders/${login.output.body.shop_id}
query: { dry_run: "false" }
headers: { Authorization: "Bearer ${auth.token}" }
body:
  sku: ${cart.output.body.items[0].sku}
  qty: 2
timeout_ms: 5000
response_schema:
  type: object
  properties: { id: { type: string } }"#;

    #[test]
    fn full_config_deserializes() {
        let NodeKind::Http(cfg) = single_node(FULL).unwrap() else {
            panic!("expected http node")
        };
        assert_eq!(cfg.method, HttpMethod::Post);
        assert_eq!(cfg.path, "/orders/${login.output.body.shop_id}");
        assert_eq!(cfg.query["dry_run"], "false");
        assert_eq!(cfg.headers["Authorization"], "Bearer ${auth.token}");
        assert_eq!(
            cfg.body,
            Some(json!({"sku": "${cart.output.body.items[0].sku}", "qty": 2}))
        );
        assert_eq!(cfg.timeout_ms, Some(5000));
        assert_eq!(
            cfg.response_schema.unwrap()["properties"]["id"]["type"],
            "string"
        );
    }

    #[test]
    fn minimal_config_defaults_optional_fields() {
        let NodeKind::Http(cfg) = single_node("id: a\ntype: http\nmethod: get\npath: /").unwrap()
        else {
            panic!("expected http node")
        };
        assert_eq!(cfg.method, HttpMethod::Get);
        assert!(cfg.query.is_empty() && cfg.headers.is_empty());
        assert_eq!(
            (cfg.body, cfg.timeout_ms, cfg.response_schema),
            (None, None, None)
        );
    }

    #[test]
    fn request_view_copies_request_fields() {
        let NodeKind::Http(cfg) = single_node(FULL).unwrap() else {
            panic!("expected http node")
        };
        let req = cfg.request();
        assert_eq!(req.method, cfg.method);
        assert_eq!(req.body, cfg.body);
        assert_eq!(req.headers, cfg.headers);
    }

    #[test]
    fn rejects_unknown_method() {
        let msg = error_message(single_node("id: a\ntype: http\nmethod: FETCH\npath: /"));
        assert!(msg.contains("FETCH"), "{msg}");
    }

    #[test]
    fn rejects_missing_path() {
        let msg = error_message(single_node("id: a\ntype: http\nmethod: GET"));
        assert!(msg.contains("path"), "{msg}");
    }

    #[test]
    fn rejects_non_string_header_value() {
        let msg = error_message(single_node(
            "id: a\ntype: http\nmethod: GET\npath: /\nheaders: { X: [1] }",
        ));
        assert!(msg.contains("headers"), "{msg}");
    }
}
