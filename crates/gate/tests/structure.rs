mod common;

use common::{dag, edge, finding, findings, http, validate};
use gate::codes;

#[test]
fn well_formed_dag_passes() {
    let report = validate(&dag(
        vec![http("login"), http("create_order"), http("pay-1")],
        vec![edge("login", "create_order"), edge("create_order", "pay-1")],
    ));
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}

#[test]
fn duplicate_node_id_points_at_later_occurrence() {
    let report = validate(&dag(vec![http("a"), http("b"), http("a")], vec![]));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::DUPLICATE_NODE_ID,
            Some("a"),
            Some("/nodes/2/id")
        )]
    );
    assert!(report.issues[0].message.contains("/nodes/0"));
}

#[test]
fn invalid_node_ids_are_rejected() {
    for bad in ["1st", "has space", "a.b", "订单", ""] {
        let report = validate(&dag(vec![http(bad)], vec![]));
        assert!(report.has_code(codes::INVALID_NODE_ID), "accepted '{bad}'");
    }
}

#[test]
fn reserved_node_ids_are_rejected() {
    for reserved in ["input", "env", "auth"] {
        let report = validate(&dag(vec![http(reserved)], vec![]));
        assert_eq!(
            findings(&report),
            vec![finding(
                codes::RESERVED_NODE_ID,
                Some(reserved),
                Some("/nodes/0/id")
            )]
        );
    }
}

#[test]
fn dangling_edge_endpoints_are_located() {
    let report = validate(&dag(
        vec![http("a")],
        vec![edge("a", "ghost"), edge("x", "y")],
    ));
    assert_eq!(
        findings(&report),
        vec![
            finding(codes::DANGLING_EDGE, None, Some("/edges/1/from")),
            finding(codes::DANGLING_EDGE, None, Some("/edges/1/to")),
            finding(codes::DANGLING_EDGE, Some("a"), Some("/edges/0/to")),
        ]
    );
}

#[test]
fn self_loop_is_rejected() {
    let report = validate(&dag(vec![http("a")], vec![edge("a", "a")]));
    assert_eq!(
        findings(&report),
        vec![finding(codes::SELF_LOOP, Some("a"), Some("/edges/0"))]
    );
}

#[test]
fn duplicate_edge_is_rejected_once() {
    let report = validate(&dag(
        vec![http("a"), http("b")],
        vec![edge("a", "b"), edge("a", "b"), edge("a", "b")],
    ));
    assert_eq!(
        findings(&report),
        vec![
            finding(codes::DUPLICATE_EDGE, Some("a"), Some("/edges/1")),
            finding(codes::DUPLICATE_EDGE, Some("a"), Some("/edges/2")),
        ]
    );
}

#[test]
fn reverse_edges_are_not_duplicates() {
    let report = validate(&dag(
        vec![http("a"), http("b")],
        vec![edge("a", "b"), edge("b", "a")],
    ));
    assert!(!report.has_code(codes::DUPLICATE_EDGE));
}
