mod common;

use common::{dag, edge, finding, findings, http, validate};
use gate::codes;

#[test]
fn diamond_is_acyclic() {
    let report = validate(&dag(
        vec![http("a"), http("b"), http("c"), http("d")],
        vec![
            edge("a", "b"),
            edge("a", "c"),
            edge("b", "d"),
            edge("c", "d"),
        ],
    ));
    assert!(!report.has_code(codes::CYCLE), "{:#?}", report.issues);
}

#[test]
fn three_node_cycle_reports_full_path() {
    let report = validate(&dag(
        vec![http("a"), http("b"), http("c")],
        vec![edge("b", "c"), edge("a", "b"), edge("c", "a")],
    ));
    assert_eq!(
        findings(&report),
        vec![finding(codes::CYCLE, Some("a"), Some("/edges/1"))]
    );
    assert_eq!(report.issues[0].message, "cycle detected: a -> b -> c -> a");
}

#[test]
fn two_node_cycle_behind_acyclic_prefix() {
    let report = validate(&dag(
        vec![http("start"), http("x"), http("y")],
        vec![edge("start", "x"), edge("x", "y"), edge("y", "x")],
    ));
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].message, "cycle detected: x -> y -> x");
}

#[test]
fn self_loop_is_left_to_the_structure_rule() {
    let report = validate(&dag(vec![http("a")], vec![edge("a", "a")]));
    assert!(report.has_code(codes::SELF_LOOP));
    assert!(!report.has_code(codes::CYCLE));
}
