#![allow(clippy::unwrap_used)]

mod common;

use common::{dag, edge, finding, findings, http, validate_all};
use gate::codes;
use model::Dag;
use serde_json::{json, Value};

fn check(target: &str) -> Value {
    json!({"id": format!("check_{target}"), "type": "assert", "target": target,
        "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]})
}

fn cleanup(id: &str) -> Value {
    json!({"id": id, "type": "cleanup", "requests": [{"method": "DELETE", "path": "/o"}]})
}

fn run(nodes: Vec<Value>, edges: Vec<Value>) -> Vec<(String, Option<String>, Option<String>)> {
    findings(&validate_all(&dag(nodes, edges)))
}

#[test]
fn complete_dag_passes() {
    assert_eq!(
        run(
            vec![http("a"), check("a"), cleanup("c")],
            vec![edge("a", "check_a"), edge("check_a", "c")],
        ),
        vec![]
    );
}

#[test]
fn single_node_dag_is_not_an_orphan() {
    assert_eq!(
        run(
            vec![json!({"id": "x", "type": "db_query", "connection": "main", "sql": "select 1"})],
            vec![]
        ),
        vec![]
    );
}

#[test]
fn empty_required_fields_are_rejected() {
    let nodes = vec![
        json!({"id": "h", "type": "http", "method": "GET", "path": " "}),
        json!({"id": "q", "type": "db_query", "connection": "", "sql": ""}),
        json!({"id": "s", "type": "subworkflow", "dag": {"id": ""}}),
        json!({"id": "chk", "type": "assert", "target": "h", "assertions": []}),
        json!({"id": "c", "type": "cleanup", "requests": [{"method": "DELETE", "path": ""}]}),
    ];
    let edges = vec![
        edge("h", "q"),
        edge("q", "s"),
        edge("s", "chk"),
        edge("chk", "c"),
    ];
    let mut got = run(nodes, edges);
    got.retain(|f| f.0 == codes::EMPTY_FIELD);
    let at = |node: &str, path: &str| finding(codes::EMPTY_FIELD, Some(node), Some(path));
    assert_eq!(
        got,
        vec![
            at("c", "/nodes/4/requests/0/path"),
            at("chk", "/nodes/3/assertions"),
            at("h", "/nodes/0/path"),
            at("q", "/nodes/1/connection"),
            at("q", "/nodes/1/sql"),
            at("s", "/nodes/2/dag/id"),
        ]
    );
}

#[test]
fn empty_cleanup_and_empty_dag_are_rejected() {
    assert_eq!(
        run(vec![json!({"id": "c", "type": "cleanup"})], vec![]),
        vec![finding(
            codes::EMPTY_FIELD,
            Some("c"),
            Some("/nodes/0/requests")
        )]
    );
    let empty = Dag::from_json_str(r#"{"id": "", "nodes": []}"#).unwrap();
    assert_eq!(
        findings(&validate_all(&empty)),
        vec![
            finding(codes::EMPTY_FIELD, None, Some("/id")),
            finding(codes::EMPTY_FIELD, None, Some("/nodes")),
        ]
    );
}

#[test]
fn orphan_node_is_rejected() {
    let got = run(
        vec![http("a"), check("a"), cleanup("lonely")],
        vec![
            edge("a", "check_a"),
            edge("ghost", "lonely"),
            edge("lonely", "lonely"),
        ],
    );
    assert!(
        got.contains(&finding(
            codes::ORPHAN_NODE,
            Some("lonely"),
            Some("/nodes/2")
        )),
        "{got:#?}"
    );
    assert_eq!(got.iter().filter(|f| f.0 == codes::ORPHAN_NODE).count(), 1);
}

#[test]
fn cleanup_with_successor_is_rejected() {
    let got = run(
        vec![http("a"), cleanup("c"), check("a")],
        vec![edge("a", "c"), edge("c", "check_a")],
    );
    assert_eq!(
        got,
        vec![finding(
            codes::CLEANUP_HAS_SUCCESSOR,
            Some("c"),
            Some("/edges/1")
        )]
    );
}

#[test]
fn unasserted_http_is_a_warning() {
    let report = validate_all(&dag(
        vec![http("a"), http("b"), check("a")],
        vec![edge("a", "b"), edge("a", "check_a")],
    ));
    assert!(report.passed(), "{:#?}", report.issues);
    assert_eq!(
        findings(&report),
        vec![finding(codes::UNASSERTED_HTTP, Some("b"), Some("/nodes/1"))]
    );
}

#[test]
fn http_referenced_from_an_assert_expected_value_counts_as_asserted() {
    let check_b = json!({"id": "check_b", "type": "assert", "target": "a", "assertions": [
        {"path": "$.status", "op": "eq", "expected": "${b.output.status}"}
    ]});
    assert_eq!(
        run(
            vec![http("a"), http("b"), check_b],
            vec![edge("a", "b"), edge("b", "check_b")],
        ),
        vec![]
    );
}
