mod common;

use common::{dag, edge, error_findings, finding, http, validate};
use gate::codes;
use serde_json::json;

fn http_with_path(id: &str, path: &str) -> serde_json::Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

#[test]
fn transitive_ancestor_reference_passes() {
    let report = validate(&dag(
        vec![
            http("login"),
            http("create"),
            http_with_path(
                "pay",
                "/pay/${login.output.body.uid}/${create.output.body.id}",
            ),
        ],
        vec![edge("login", "create"), edge("create", "pay")],
    ));
    assert!(report.passed(), "{:#?}", report.issues);
}

#[test]
fn sibling_reference_is_located_to_the_field() {
    let report = validate(&dag(
        vec![
            http("root"),
            http("c"),
            json!({"id": "d", "type": "http", "method": "POST", "path": "/d",
                   "body": {"order": {"id": "${c.output.body.id}"}}}),
        ],
        vec![edge("root", "c"), edge("root", "d")],
    ));
    assert_eq!(
        error_findings(&report),
        vec![finding(
            codes::NOT_ANCESTOR,
            Some("d"),
            Some("/nodes/2/body/order/id")
        )]
    );
    assert!(report.issues[0].message.contains("'c'"));
}

#[test]
fn successor_and_self_references_are_rejected() {
    let report = validate(&dag(
        vec![
            http_with_path("a", "/${b.output.body.id}"),
            http_with_path("b", "/${b.output.body.id}"),
        ],
        vec![edge("a", "b")],
    ));
    assert_eq!(
        error_findings(&report),
        vec![
            finding(codes::NOT_ANCESTOR, Some("a"), Some("/nodes/0/path")),
            finding(codes::NOT_ANCESTOR, Some("b"), Some("/nodes/1/path")),
        ]
    );
    assert!(report.issues[1].message.contains("itself"));
}

#[test]
fn unknown_node_reference_is_rejected() {
    let report = validate(&dag(vec![http_with_path("a", "/${ghost.output}")], vec![]));
    assert_eq!(
        error_findings(&report),
        vec![finding(
            codes::UNKNOWN_NODE,
            Some("a"),
            Some("/nodes/0/path")
        )]
    );
}

#[test]
fn assert_target_must_be_upstream() {
    let assert_node = json!({"id": "check", "type": "assert", "target": "b",
        "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]});
    let report = validate(&dag(
        vec![http("a"), http("b"), assert_node],
        vec![edge("a", "check"), edge("a", "b")],
    ));
    assert_eq!(
        error_findings(&report),
        vec![finding(
            codes::NOT_ANCESTOR,
            Some("check"),
            Some("/nodes/2/target")
        )]
    );
}

#[test]
fn env_auth_and_input_references_are_not_node_references() {
    let report = validate(&dag(
        vec![json!({"id": "a", "type": "http", "method": "GET",
            "path": "/${input.output.user}?k=${env:KEY}",
            "headers": {"Authorization": "Bearer ${auth.token}"}})],
        vec![],
    ));
    assert!(report.passed(), "{:#?}", report.issues);
}

#[test]
fn invalid_template_is_reported_with_pointer() {
    let report = validate(&dag(
        vec![
            json!({"id": "a", "type": "http", "method": "GET", "path": "/",
            "headers": {"X": "${a.input}"}}),
        ],
        vec![],
    ));
    assert_eq!(
        error_findings(&report),
        vec![finding(
            codes::INVALID_TEMPLATE,
            Some("a"),
            Some("/nodes/0/headers/X")
        )]
    );
    assert!(report.issues[0].message.contains("char 3"));
}
