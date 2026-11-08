#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use engine::jsonpath::select;
use engine::{evaluate, AssertExecutor, Environment, ExecContext, ExecErrorKind, NodeExecutor};
use model::nodes::{Condition, Operator};
use model::{Dag, Node};
use serde_json::{json, Value};

fn output() -> Value {
    json!({
        "status": 201,
        "headers": {"content-type": "application/json"},
        "body": {
            "id": "ord-42",
            "total": 99.5,
            "count": 3,
            "paid": false,
            "note": null,
            "tags": ["new", "vip"],
            "items": [{"sku": "A", "qty": 2}],
            "meta": {"a.b": 1, "region": "eu"},
            "emoji": "héllo"
        }
    })
}

#[test]
fn select_follows_singular_paths() {
    let out = output();
    let cases: &[(&str, Option<Value>)] = &[
        ("$", Some(out.clone())),
        ("$.status", Some(json!(201))),
        ("$.body.items[0].sku", Some(json!("A"))),
        ("$.body.meta['a.b']", Some(json!(1))),
        ("$.body.note", Some(Value::Null)),
        ("$.body.items[3]", None),
        ("$.body.id.x", None),
        ("$.body.tags.x", None),
        ("$.body.meta[0]", None),
        ("$.missing", None),
    ];
    for (path, expected) in cases {
        assert_eq!(select(&out, path).unwrap().cloned(), *expected, "{path}");
    }
    assert!(select(&out, "$.body[*]").is_err());
    assert!(select(&out, "body").is_err());
}

fn cond(path: &str, op: &str, expected: Option<Value>) -> Condition {
    let mut value = json!({"path": path, "op": op});
    if let Some(e) = expected {
        value["expected"] = e;
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn every_operator_has_passing_and_failing_cases() {
    // (path, op, expected, passes)
    let cases: Vec<(&str, &str, Option<Value>, bool)> = vec![
        ("$.status", "eq", Some(json!(201)), true),
        ("$.status", "eq", Some(json!(200)), false),
        ("$.status", "eq", Some(json!(201.0)), true),
        (
            "$.body.items",
            "eq",
            Some(json!([{"qty": 2, "sku": "A"}])),
            true,
        ),
        ("$.body.note", "eq", Some(Value::Null), true),
        ("$.body.paid", "eq", Some(json!("false")), false),
        ("$.status", "ne", Some(json!(500)), true),
        ("$.status", "ne", Some(json!(201)), false),
        ("$.body.total", "gt", Some(json!(99)), true),
        ("$.body.total", "gt", Some(json!(99.5)), false),
        ("$.body.total", "gte", Some(json!(99.5)), true),
        ("$.body.total", "gte", Some(json!(100)), false),
        ("$.body.count", "lt", Some(json!(4)), true),
        ("$.body.count", "lt", Some(json!(3)), false),
        ("$.body.count", "lte", Some(json!(3)), true),
        ("$.body.count", "lte", Some(json!(2.5)), false),
        ("$.body.id", "contains", Some(json!("42")), true),
        ("$.body.id", "contains", Some(json!("43")), false),
        ("$.body.tags", "contains", Some(json!("vip")), true),
        ("$.body.tags", "contains", Some(json!("old")), false),
        ("$.body.meta", "contains", Some(json!("region")), true),
        ("$.body.meta", "contains", Some(json!("zone")), false),
        ("$.body.note", "exists", None, true),
        ("$.body.gone", "exists", None, false),
        ("$.body.gone", "not_exists", None, true),
        ("$.body.id", "not_exists", None, false),
        ("$.body.id", "regex", Some(json!("^ord-\\d+$")), true),
        ("$.body.id", "regex", Some(json!("^inv-")), false),
        ("$.body.tags", "len_eq", Some(json!(2)), true),
        ("$.body.tags", "len_eq", Some(json!(3)), false),
        ("$.body.emoji", "len_eq", Some(json!(5)), true),
        ("$.body.meta", "len_eq", Some(json!(2)), true),
        ("$.body.count", "type_is", Some(json!("integer")), true),
        ("$.body.count", "type_is", Some(json!("number")), true),
        ("$.body.total", "type_is", Some(json!("integer")), false),
        ("$.body.note", "type_is", Some(json!("null")), true),
        ("$.body.tags", "type_is", Some(json!("object")), false),
        ("$.body.gone", "eq", Some(Value::Null), false),
    ];
    let out = output();
    let mut seen = std::collections::BTreeSet::new();
    for (path, op, expected, passes) in cases {
        let c = cond(path, op, expected.clone());
        let result = evaluate(&c, expected.as_ref(), &out);
        assert_eq!(
            result.passed, passes,
            "{path} {op} {expected:?}: {result:?}"
        );
        assert_eq!(result.message, None, "{path} {op}");
        seen.insert((op, passes));
    }
    for op in [
        "eq",
        "ne",
        "gt",
        "gte",
        "lt",
        "lte",
        "contains",
        "exists",
        "not_exists",
        "regex",
        "len_eq",
        "type_is",
    ] {
        assert!(
            seen.contains(&(op, true)) && seen.contains(&(op, false)),
            "{op}"
        );
    }
}

#[test]
fn type_mismatches_fail_with_a_message() {
    let cases: Vec<(&str, &str, Value, &str)> = vec![
        (
            "$.body.id",
            "gt",
            json!(1),
            "compares numbers, got string and number",
        ),
        (
            "$.body.count",
            "contains",
            json!("3"),
            "cannot look for string in number",
        ),
        (
            "$.body.count",
            "regex",
            json!("3"),
            "matches strings, got number",
        ),
        (
            "$.body.paid",
            "len_eq",
            json!(1),
            "needs a string, array or object, got boolean",
        ),
        (
            "$.body.id",
            "type_is",
            json!("date"),
            "needs one of null/boolean",
        ),
        ("$.body.id", "regex", json!("("), "invalid regex"),
    ];
    let out = output();
    for (path, op, expected, message) in cases {
        let result = evaluate(
            &cond(path, op, Some(expected.clone())),
            Some(&expected),
            &out,
        );
        assert!(!result.passed, "{path} {op}");
        let got = result.message.unwrap_or_default();
        assert!(got.contains(message), "{path} {op}: {got}");
    }
}

#[test]
fn results_record_path_operator_expected_and_actual() {
    let result = evaluate(
        &cond("$.status", "eq", Some(json!(200))),
        Some(&json!(200)),
        &output(),
    );
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        json!({"path": "$.status", "operator": "eq", "expected": 200, "actual": 201, "passed": false})
    );
    let missing = evaluate(&cond("$.body.gone", "exists", None), None, &output());
    assert_eq!(
        serde_json::to_value(&missing).unwrap(),
        json!({"path": "$.body.gone", "operator": "exists", "passed": false})
    );
    assert_eq!(result.operator, Operator::Eq);
}

fn ctx() -> ExecContext {
    ExecContext {
        run_id: "run-1".into(),
        outputs: BTreeMap::from([
            ("create".to_owned(), output()),
            ("login".to_owned(), json!({"body": {"order": "ord-42"}})),
        ]),
        env: Arc::new(
            Environment::new(
                "local",
                "http://sut",
                BTreeMap::from([("REGION".to_owned(), "eu".to_owned())]),
            )
            .unwrap(),
        ),
    }
}

fn assert_node(target: &str, assertions: Value) -> Node {
    Dag::from_json_str(
        &json!({"id": "d", "nodes": [
            {"id": "check", "type": "assert", "target": target, "assertions": assertions}
        ]})
        .to_string(),
    )
    .unwrap()
    .nodes
    .remove(0)
}

#[tokio::test]
async fn assert_node_passes_with_rendered_expectations() {
    let node = assert_node(
        "create",
        json!([
            {"path": "$.status", "op": "eq", "expected": 201},
            {"path": "$.body.id", "op": "eq", "expected": "${login.output.body.order}"},
            {"path": "$.body.meta.region", "op": "eq", "expected": "${env:REGION}"},
            {"path": "$.body.id", "op": "exists"}
        ]),
    );
    let output = AssertExecutor.execute(&node, ctx()).await.unwrap().output;
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|r| r["passed"] == json!(true)));
    assert_eq!(results[1]["expected"], json!("ord-42"));
}

#[tokio::test]
async fn assert_node_reports_every_result_when_one_fails() {
    let node = assert_node(
        "create",
        json!([
            {"path": "$.status", "op": "eq", "expected": 200},
            {"path": "$.body.id", "op": "exists"},
            {"path": "$.body.total", "op": "lt", "expected": 10}
        ]),
    );
    let err = AssertExecutor.execute(&node, ctx()).await.unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Assertion);
    assert_eq!(
        err.message,
        "2 of 3 assertions failed; first: $.status eq 200 (actual 201)"
    );
    let detail = err.detail.unwrap();
    let passed: Vec<&Value> = detail["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["passed"])
        .collect();
    assert_eq!(passed, [&json!(false), &json!(true), &json!(false)]);
}

#[tokio::test]
async fn assert_node_scope_and_template_errors() {
    let err = AssertExecutor
        .execute(
            &assert_node("ghost", json!([{"path": "$", "op": "exists"}])),
            ctx(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert!(err.message.contains("'ghost' has no output"));

    let err = AssertExecutor
        .execute(
            &assert_node(
                "create",
                json!([
                    {"path": "$.status", "op": "eq", "expected": 201},
                    {"path": "$.body.id", "op": "eq", "expected": {"x": ["${env:NOPE}"]}}
                ]),
            ),
            ctx(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(
        err.detail.unwrap()["pointer"],
        json!("/assertions/1/expected/x/0")
    );
}
