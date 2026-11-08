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

mod poll {
    use super::*;
    use model::nodes::Operator;

    const VALID: &str = r#"id: wait_paid
type: poll
request: { method: GET, path: "/orders/${create.output.body.id}" }
condition: { path: $.body.status, op: eq, expected: PAID }
backoff_ms: [500, 1000, 2000]
timeout_ms: 30000"#;

    fn without(field: &str) -> String {
        VALID
            .lines()
            .filter(|l| !l.starts_with(&format!("{field}:")))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn valid_config_deserializes() {
        let NodeKind::Poll(cfg) = single_node(VALID).unwrap() else {
            panic!("expected poll node")
        };
        assert_eq!(cfg.request.method, HttpMethod::Get);
        assert_eq!(cfg.request.path, "/orders/${create.output.body.id}");
        assert_eq!(cfg.condition.path, "$.body.status");
        assert_eq!(cfg.condition.op, Operator::Eq);
        assert_eq!(cfg.condition.expected, Some(json!("PAID")));
        assert_eq!(cfg.backoff_ms, vec![500, 1000, 2000]);
        assert_eq!(cfg.timeout_ms, 30000);
        assert_eq!(cfg.max_attempts, None);
    }

    #[test]
    fn each_required_field_is_enforced() {
        for field in ["request", "condition", "backoff_ms", "timeout_ms"] {
            let msg = error_message(single_node(&without(field)));
            assert!(msg.contains(field), "missing {field}: {msg}");
        }
    }

    #[test]
    fn free_form_condition_expression_is_rejected() {
        let input = VALID.replace(
            "condition: { path: $.body.status, op: eq, expected: PAID }",
            "condition: \"body.status == 'PAID'\"",
        );
        let msg = error_message(single_node(&input));
        assert!(msg.contains("condition"), "{msg}");
    }

    #[test]
    fn unknown_operator_is_rejected() {
        let input = VALID.replace("op: eq", "op: approx");
        let msg = error_message(single_node(&input));
        assert!(msg.contains("approx"), "{msg}");
    }

    #[test]
    fn exists_needs_no_expected_value() {
        let input = VALID.replace("op: eq, expected: PAID", "op: exists");
        let NodeKind::Poll(cfg) = single_node(&input).unwrap() else {
            panic!("expected poll node")
        };
        assert_eq!(cfg.condition.expected, None);
        assert!(!cfg.condition.op.needs_expected());
        assert!(Operator::Eq.needs_expected());
    }
}
