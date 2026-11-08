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

mod assert {
    use super::*;
    use model::nodes::{AssertConfig, Operator};

    fn parse(assertions_yaml: &str) -> Result<NodeKind, ModelError> {
        single_node(&format!(
            "id: check\ntype: assert\ntarget: create\nassertions:\n{assertions_yaml}"
        ))
    }

    fn config(kind: NodeKind) -> AssertConfig {
        match kind {
            NodeKind::Assert(cfg) => cfg,
            other => panic!("expected assert node, got {}", other.type_name()),
        }
    }

    #[test]
    fn every_operator_deserializes() {
        let cases = [
            ("eq", Operator::Eq, "201"),
            ("ne", Operator::Ne, "500"),
            ("gt", Operator::Gt, "0"),
            ("gte", Operator::Gte, "1"),
            ("lt", Operator::Lt, "100"),
            ("lte", Operator::Lte, "99"),
            ("contains", Operator::Contains, "VIP"),
            ("exists", Operator::Exists, ""),
            ("not_exists", Operator::NotExists, ""),
            ("regex", Operator::Regex, "'^ORD-\\d+$'"),
            ("len_eq", Operator::LenEq, "3"),
            ("type_is", Operator::TypeIs, "string"),
        ];
        for (name, op, expected) in cases {
            let line = if expected.is_empty() {
                format!("  - {{ path: $.body.x, op: {name} }}")
            } else {
                format!("  - {{ path: $.body.x, op: {name}, expected: {expected} }}")
            };
            let cfg = config(parse(&line).unwrap());
            assert_eq!(cfg.target, "create");
            assert_eq!(cfg.assertions.len(), 1);
            assert_eq!(cfg.assertions[0].op, op, "{name}");
            assert_eq!(
                cfg.assertions[0].expected.is_some(),
                op.needs_expected(),
                "{name}"
            );
        }
    }

    #[test]
    fn multiple_assertions_keep_order_and_value_types() {
        let cfg = config(
            parse(
                "  - { path: $.status, op: eq, expected: 201 }\n  - { path: $.body.tags, op: eq, expected: [a, b] }\n  - { path: $.body.vip, op: eq, expected: true }",
            )
            .unwrap(),
        );
        let expected: Vec<_> = cfg.assertions.iter().map(|a| a.expected.clone()).collect();
        assert_eq!(
            expected,
            vec![Some(json!(201)), Some(json!(["a", "b"])), Some(json!(true))]
        );
    }

    #[test]
    fn missing_target_or_assertions_is_rejected() {
        let msg = error_message(single_node("id: c\ntype: assert\nassertions: []"));
        assert!(msg.contains("target"), "{msg}");
        let msg = error_message(single_node("id: c\ntype: assert\ntarget: a"));
        assert!(msg.contains("assertions"), "{msg}");
    }

    #[test]
    fn misspelled_operator_key_is_rejected() {
        let msg = error_message(parse("  - { path: $.status, operator: eq, expected: 1 }"));
        assert!(msg.contains("operator"), "{msg}");
    }
}

mod db_query {
    use super::*;
    use model::nodes::QueryResultMode;

    #[test]
    fn full_config_deserializes() {
        let NodeKind::DbQuery(cfg) = single_node(
            "id: vip\ntype: db_query\nconnection: crm-ro\nsql: select id from users where level = $1 and balance > $2\nparams: [VIP, \"${env:MIN_BALANCE}\"]\nresult: first_row",
        )
        .unwrap() else {
            panic!("expected db_query node")
        };
        assert_eq!(cfg.connection, "crm-ro");
        assert_eq!(cfg.params, vec![json!("VIP"), json!("${env:MIN_BALANCE}")]);
        assert_eq!(cfg.result, QueryResultMode::FirstRow);
    }

    #[test]
    fn result_mode_defaults_to_rows() {
        let NodeKind::DbQuery(cfg) =
            single_node("id: q\ntype: db_query\nconnection: c\nsql: select 1").unwrap()
        else {
            panic!("expected db_query node")
        };
        assert_eq!(cfg.result, QueryResultMode::Rows);
        assert!(cfg.params.is_empty());
    }

    #[test]
    fn missing_sql_is_rejected() {
        let msg = error_message(single_node("id: q\ntype: db_query\nconnection: c"));
        assert!(msg.contains("sql"), "{msg}");
    }
}

mod subworkflow {
    use super::*;

    #[test]
    fn bindings_deserialize() {
        let NodeKind::Subworkflow(cfg) = single_node(
            "id: login\ntype: subworkflow\ndag: { id: auth-login, version: 3 }\ninputs:\n  user: \"${env:USER}\"\n  retries: 2\noutputs:\n  token: \"${do_login.output.body.token}\"",
        )
        .unwrap() else {
            panic!("expected subworkflow node")
        };
        assert_eq!(cfg.dag.id, "auth-login");
        assert_eq!(cfg.dag.version, Some(3));
        assert_eq!(cfg.inputs["user"], json!("${env:USER}"));
        assert_eq!(cfg.inputs["retries"], json!(2));
        assert_eq!(cfg.outputs["token"], "${do_login.output.body.token}");
    }

    #[test]
    fn unpinned_version_is_none() {
        let NodeKind::Subworkflow(cfg) =
            single_node("id: s\ntype: subworkflow\ndag: { id: child }").unwrap()
        else {
            panic!("expected subworkflow node")
        };
        assert_eq!(cfg.dag.version, None);
        assert!(cfg.inputs.is_empty() && cfg.outputs.is_empty());
    }

    #[test]
    fn bare_string_dag_reference_is_rejected() {
        let msg = error_message(single_node("id: s\ntype: subworkflow\ndag: child"));
        assert!(msg.contains("dag"), "{msg}");
    }
}

mod cleanup {
    use super::*;

    #[test]
    fn requests_deserialize_in_order() {
        let NodeKind::Cleanup(cfg) = single_node(
            "id: cleanup\ntype: cleanup\nrequests:\n  - { method: DELETE, path: \"/orders/${create.output.body.id}\" }\n  - { method: POST, path: /reset }",
        )
        .unwrap() else {
            panic!("expected cleanup node")
        };
        let methods: Vec<_> = cfg.requests.iter().map(|r| r.method).collect();
        assert_eq!(methods, vec![HttpMethod::Delete, HttpMethod::Post]);
    }

    #[test]
    fn empty_cleanup_is_allowed() {
        let NodeKind::Cleanup(cfg) = single_node("id: c\ntype: cleanup").unwrap() else {
            panic!("expected cleanup node")
        };
        assert!(cfg.requests.is_empty());
    }
}
