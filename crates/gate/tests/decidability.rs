mod common;

use common::{dag, edge, finding, findings, validate};
use gate::codes;
use serde_json::{json, Value};

fn poll(patch: Value) -> Value {
    let mut node = json!({
        "id": "p", "type": "poll",
        "request": {"method": "GET", "path": "/jobs/1"},
        "condition": {"path": "$.body.state", "op": "eq", "expected": "done"},
        "backoff_ms": [100, 200, 200],
        "timeout_ms": 5000
    });
    if let (Some(node), Value::Object(patch)) = (node.as_object_mut(), patch) {
        node.extend(patch);
    }
    node
}

fn condition(path: &str, op: &str, expected: Option<Value>) -> Value {
    let mut c = json!({"path": path, "op": op});
    if let Some(e) = expected {
        c["expected"] = e;
    }
    c
}

fn poll_findings(patch: Value) -> Vec<(String, Option<String>, Option<String>)> {
    findings(&validate(&dag(vec![poll(patch)], vec![])))
}

#[test]
fn well_formed_poll_passes() {
    assert_eq!(poll_findings(json!({})), vec![]);
    assert_eq!(
        poll_findings(json!({"backoff_ms": [0], "timeout_ms": 0, "max_attempts": 1})),
        vec![]
    );
}

#[test]
fn unparseable_condition_path_is_rejected() {
    for path in ["body.state", "$.body[*]", "$..state", "$.items[-1]"] {
        assert_eq!(
            poll_findings(json!({"condition": condition(path, "exists", None)})),
            vec![finding(
                codes::CONDITION_INVALID_PATH,
                Some("p"),
                Some("/nodes/0/condition/path")
            )],
            "{path}"
        );
    }
}

#[test]
fn condition_path_must_exist_in_response_schema() {
    let schema = json!({"type": "object", "properties": {"state": {"type": "string"}}});
    let ok = json!({"response_schema": schema.clone()});
    assert_eq!(poll_findings(ok), vec![]);

    let report = validate(&dag(
        vec![poll(json!({
            "response_schema": schema,
            "condition": condition("$.body.status", "eq", Some(json!("done")))
        }))],
        vec![],
    ));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::CONDITION_UNKNOWN_FIELD,
            Some("p"),
            Some("/nodes/0/condition/path")
        )]
    );
    assert!(report.issues[0].message.contains("$.body.status"));
}

#[test]
fn operator_and_expected_must_match() {
    let at = |p: &str| Some(format!("/nodes/0/condition{p}"));
    let cases = [
        ("eq", None, codes::CONDITION_MISSING_EXPECTED, ""),
        (
            "exists",
            Some(json!(true)),
            codes::CONDITION_UNEXPECTED_EXPECTED,
            "/expected",
        ),
        (
            "gt",
            Some(json!("10")),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
        (
            "len_eq",
            Some(json!(-1)),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
        (
            "len_eq",
            Some(json!(1.5)),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
        (
            "regex",
            Some(json!("(")),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
        (
            "regex",
            Some(json!(1)),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
        (
            "type_is",
            Some(json!("float")),
            codes::CONDITION_EXPECTED_TYPE,
            "/expected",
        ),
    ];
    for (op, expected, code, pointer) in cases {
        let got = poll_findings(json!({"condition": condition("$.status", op, expected.clone())}));
        assert_eq!(
            got,
            vec![(code.to_owned(), Some("p".to_owned()), at(pointer))],
            "{op} {expected:?}"
        );
    }
}

#[test]
fn compatible_expected_values_pass() {
    let cases = [
        ("gte", json!(200)),
        ("lt", json!(0.5)),
        ("len_eq", json!(0)),
        ("regex", json!("^ok-[0-9]+$")),
        ("type_is", json!("integer")),
        ("contains", json!({"a": 1})),
        ("ne", json!(null)),
        ("gt", json!("${env:LIMIT}")),
    ];
    for (op, expected) in cases {
        assert_eq!(
            poll_findings(json!({"condition": condition("$.status", op, Some(expected.clone()))})),
            vec![],
            "{op} {expected}"
        );
    }
}

#[test]
fn empty_backoff_is_rejected() {
    assert_eq!(
        poll_findings(json!({"backoff_ms": []})),
        vec![finding(
            codes::POLL_EMPTY_BACKOFF,
            Some("p"),
            Some("/nodes/0/backoff_ms")
        )]
    );
}

#[test]
fn decreasing_backoff_is_located_at_the_first_drop() {
    assert_eq!(
        poll_findings(json!({"backoff_ms": [100, 400, 300, 200]})),
        vec![finding(
            codes::POLL_BACKOFF_DECREASING,
            Some("p"),
            Some("/nodes/0/backoff_ms/2")
        )]
    );
}

#[test]
fn timeout_shorter_than_first_backoff_is_rejected() {
    assert_eq!(
        poll_findings(json!({"backoff_ms": [1000], "timeout_ms": 999})),
        vec![finding(
            codes::POLL_TIMEOUT_TOO_SHORT,
            Some("p"),
            Some("/nodes/0/timeout_ms")
        )]
    );
}

#[test]
fn zero_max_attempts_is_rejected() {
    assert_eq!(
        poll_findings(json!({"max_attempts": 0})),
        vec![finding(
            codes::POLL_ZERO_MAX_ATTEMPTS,
            Some("p"),
            Some("/nodes/0/max_attempts")
        )]
    );
}

#[test]
fn assertions_are_checked_against_the_target_output() {
    let create = json!({"id": "create", "type": "http", "method": "POST", "path": "/o",
        "response_schema": {"type": "object", "properties": {"id": {"type": "string"}}}});
    let check = json!({"id": "check", "type": "assert", "target": "create", "assertions": [
        condition("$.status", "eq", Some(json!(201))),
        condition("$.body.id", "exists", None),
        condition("$.body.orderId", "exists", None),
        condition("$.body.id", "gt", Some(json!("x"))),
    ]});
    let report = validate(&dag(vec![create, check], vec![edge("create", "check")]));
    assert_eq!(
        findings(&report),
        vec![
            finding(
                codes::CONDITION_EXPECTED_TYPE,
                Some("check"),
                Some("/nodes/1/assertions/3/expected")
            ),
            finding(
                codes::CONDITION_UNKNOWN_FIELD,
                Some("check"),
                Some("/nodes/1/assertions/2/path")
            ),
        ]
    );
}

#[test]
fn assertions_on_a_non_ancestor_skip_the_field_check() {
    let create = json!({"id": "create", "type": "http", "method": "POST", "path": "/o",
        "response_schema": {"type": "object", "properties": {}}});
    let check = json!({"id": "check", "type": "assert", "target": "create", "assertions": [
        condition("$.body.missing", "exists", None),
    ]});
    let report = validate(&dag(vec![create, check], vec![]));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::NOT_ANCESTOR,
            Some("check"),
            Some("/nodes/1/target")
        )]
    );
}
