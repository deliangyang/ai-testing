mod common;

use common::{dag, edge, http, validate};
use gate::calibrate;
use gate::codes;
use model::NodeKind;
use serde_json::json;

fn http_with_path(id: &str, path: &str) -> serde_json::Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

fn poll(patch: serde_json::Value) -> serde_json::Value {
    let mut node = json!({
        "id": "p", "type": "poll",
        "request": {"method": "GET", "path": "/jobs/1"},
        "condition": {"path": "$.body.state", "op": "eq", "expected": "done"},
        "backoff_ms": [100, 200],
        "timeout_ms": 5000
    });
    if let (Some(node), serde_json::Value::Object(patch)) = (node.as_object_mut(), patch) {
        node.extend(patch);
    }
    node
}

#[test]
fn adds_edge_when_sibling_references_upstream_output() {
    let input = dag(
        vec![
            http("root"),
            http("c"),
            json!({"id": "d", "type": "http", "method": "POST", "path": "/d",
                   "body": {"order": {"id": "${c.output.body.id}"}}}),
        ],
        vec![edge("root", "c"), edge("root", "d")],
    );
    let out = calibrate(input);
    assert_eq!(out.repairs.len(), 1);
    assert_eq!(out.repairs[0].code, codes::REPAIR_ADD_EDGE);
    assert_eq!(out.repairs[0].json_path, "/edges/2");
    assert!(out.repairs[0].message.contains("c -> d"));
    assert!(validate(&out.dag).passed());
}

#[test]
fn does_not_add_edge_when_reference_is_unreachable_without_cycle() {
    let input = dag(
        vec![
            http_with_path("a", "/${b.output.body.id}"),
            http_with_path("b", "/${b.output.body.id}"),
        ],
        vec![edge("a", "b")],
    );
    let out = calibrate(input);
    assert!(out.repairs.is_empty());
    assert!(!validate(&out.dag).passed());
}

#[test]
fn does_not_repair_unknown_node_reference() {
    let input = dag(vec![http_with_path("a", "/${ghost.output}")], vec![]);
    let out = calibrate(input);
    assert!(out.repairs.is_empty());
}

#[test]
fn adds_edge_for_assert_target_not_ancestor() {
    let assert_node = json!({"id": "check", "type": "assert", "target": "b",
        "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]});
    let input = dag(
        vec![http("a"), http("b"), assert_node],
        vec![edge("a", "check"), edge("a", "b")],
    );
    let out = calibrate(input);
    assert_eq!(out.repairs.len(), 1);
    assert_eq!(out.repairs[0].code, codes::REPAIR_ADD_EDGE);
    assert!(validate(&out.dag).passed());
}

#[test]
fn fills_empty_poll_backoff() {
    let input = dag(vec![poll(json!({"backoff_ms": []}))], vec![]);
    let out = calibrate(input);
    assert_eq!(out.repairs.len(), 1);
    assert_eq!(out.repairs[0].code, codes::REPAIR_POLL_BACKOFF);
    assert_eq!(out.repairs[0].json_path, "/nodes/0/backoff_ms");
    let NodeKind::Poll(cfg) = &out.dag.nodes[0].kind else {
        panic!("expected poll");
    };
    assert_eq!(cfg.backoff_ms, gate::calibrate::DEFAULT_POLL_BACKOFF_MS);
    assert!(validate(&out.dag).passed());
}

#[test]
fn fills_empty_backoff_and_zero_timeout_together() {
    let input = dag(
        vec![poll(json!({"backoff_ms": [], "timeout_ms": 0}))],
        vec![],
    );
    let out = calibrate(input);
    assert_eq!(out.repairs.len(), 2);
    assert_eq!(out.repairs[0].code, codes::REPAIR_POLL_BACKOFF);
    assert_eq!(out.repairs[1].code, codes::REPAIR_POLL_TIMEOUT);
    assert!(validate(&out.dag).passed());
}

#[test]
fn does_not_change_valid_poll_timing() {
    let input = dag(vec![poll(json!({}))], vec![]);
    let out = calibrate(input.clone());
    assert!(out.repairs.is_empty());
    assert_eq!(out.dag, input);
}

#[test]
fn fixes_poll_timeout_shorter_than_first_backoff() {
    let input = dag(
        vec![poll(json!({"backoff_ms": [1000], "timeout_ms": 999}))],
        vec![],
    );
    let out = calibrate(input);
    assert_eq!(out.repairs.len(), 1);
    assert_eq!(out.repairs[0].code, codes::REPAIR_POLL_TIMEOUT);
    let model::NodeKind::Poll(cfg) = &out.dag.nodes[0].kind else {
        panic!("expected poll");
    };
    assert!(cfg.timeout_ms >= 1000);
    assert!(validate(&out.dag).passed());
}

#[test]
fn fixes_poll_zero_timeout_with_existing_backoff() {
    let input = dag(
        vec![poll(json!({"backoff_ms": [500, 1000], "timeout_ms": 0}))],
        vec![],
    );
    let out = calibrate(input);
    assert!(out
        .repairs
        .iter()
        .any(|r| r.code == codes::REPAIR_POLL_TIMEOUT));
    assert!(validate(&out.dag).passed());
}
