#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use gate::rules::{MapCatalog, SubworkflowCycleRule};
use gate::{codes, Gate};
use model::Dag;
use serde_json::{json, Value};

fn sub(id: &str, dag: &str, version: Option<u32>) -> Value {
    let mut r = json!({"id": dag});
    if let Some(v) = version {
        r["version"] = json!(v);
    }
    json!({"id": id, "type": "subworkflow", "dag": r})
}

fn http(id: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": "/"})
}

fn dag(id: &str, version: Option<u32>, nodes: Vec<Value>) -> Dag {
    let mut doc = json!({"id": id, "nodes": nodes});
    if let Some(v) = version {
        doc["version"] = json!(v);
    }
    Dag::from_json_str(&doc.to_string()).unwrap()
}

/// (code, node, path, message) of every subworkflow finding.
fn findings(catalog: MapCatalog, root: &Dag) -> Vec<(String, String, String, String)> {
    let gate = Gate::new().with_rule(SubworkflowCycleRule::new(Arc::new(catalog)));
    gate.validate(root)
        .issues
        .into_iter()
        .map(|i| {
            (
                i.code,
                i.node_id.unwrap_or_default(),
                i.json_path.unwrap_or_default(),
                i.message,
            )
        })
        .collect()
}

fn cycle(node: &str, path: &str, chain: &str) -> (String, String, String, String) {
    (
        codes::SUBWORKFLOW_CYCLE.into(),
        node.into(),
        path.into(),
        format!("subworkflow reference cycle: {chain}"),
    )
}

#[test]
fn a_to_b_to_a_is_rejected() {
    let b = dag("b", None, vec![sub("call_a", "a", None)]);
    let a = dag("a", None, vec![http("start"), sub("call_b", "b", None)]);
    assert_eq!(
        findings(MapCatalog::new().with_active(b), &a),
        [cycle("call_b", "/nodes/1/dag", "a -> b -> a")]
    );
}

#[test]
fn self_reference_uses_the_document_being_saved() {
    // The stored version of 'a' has no subworkflow; the new one calls itself.
    let stored = dag("a", None, vec![http("x")]);
    let a = dag("a", None, vec![sub("again", "a", None)]);
    assert_eq!(
        findings(MapCatalog::new().with_active(stored), &a),
        [cycle("again", "/nodes/0/dag", "a -> a")]
    );
}

#[test]
fn removing_a_cycle_through_the_saved_dag_clears_it() {
    // Stored 'a' calls b and b calls a; the new 'a' no longer calls b, so saving it is fine.
    let stored_a = dag("a", None, vec![sub("call_b", "b", None)]);
    let b = dag("b", None, vec![sub("call_a", "a", None)]);
    let new_a = dag("a", None, vec![http("only")]);
    assert!(findings(
        MapCatalog::new().with_active(stored_a).with_active(b),
        &new_a
    )
    .is_empty());
}

#[test]
fn cycles_further_down_are_reported_at_the_entry_node() {
    let b = dag("b", None, vec![sub("to_c", "c", None)]);
    let c = dag("c", None, vec![sub("to_b", "b", None)]);
    let a = dag("a", None, vec![sub("enter", "b", None)]);
    assert_eq!(
        findings(MapCatalog::new().with_active(b).with_active(c), &a),
        [cycle("enter", "/nodes/0/dag", "b -> c -> b")]
    );
}

#[test]
fn shared_children_without_cycles_pass() {
    let d = dag("d", None, vec![http("leaf")]);
    let b = dag("b", None, vec![sub("to_d", "d", None)]);
    let c = dag("c", None, vec![sub("to_d", "d", None)]);
    let a = dag(
        "a",
        None,
        vec![sub("left", "b", None), sub("right", "c", None)],
    );
    let catalog = MapCatalog::new()
        .with_active(b)
        .with_active(c)
        .with_active(d);
    assert!(findings(catalog, &a).is_empty());
}

#[test]
fn pinned_versions_are_followed() {
    let b1 = dag("b", Some(1), vec![sub("call_a", "a", None)]);
    let b2 = dag("b", Some(2), vec![http("safe")]);
    let catalog = || {
        MapCatalog::new()
            .with_version(b1.clone())
            .with_active(b2.clone())
    };

    let to_v2 = dag("a", None, vec![sub("call_b", "b", Some(2))]);
    assert!(findings(catalog(), &to_v2).is_empty());
    let to_active = dag("a", None, vec![sub("call_b", "b", None)]);
    assert!(findings(catalog(), &to_active).is_empty());

    let to_v1 = dag("a", None, vec![sub("call_b", "b", Some(1))]);
    assert_eq!(
        findings(catalog(), &to_v1),
        [cycle("call_b", "/nodes/0/dag", "a -> b -> a")]
    );
}

#[test]
fn unknown_dags_and_versions_are_errors() {
    let b = dag("b", Some(1), vec![http("x")]);
    let a = dag(
        "a",
        None,
        vec![sub("missing", "ghost", None), sub("old", "b", Some(7))],
    );
    let got = findings(MapCatalog::new().with_active(b), &a);
    assert_eq!(
        got,
        [
            (
                codes::SUBWORKFLOW_UNKNOWN_DAG.into(),
                "missing".into(),
                "/nodes/0/dag".into(),
                "subworkflow DAG 'ghost' (active version) does not exist".into()
            ),
            (
                codes::SUBWORKFLOW_UNKNOWN_DAG.into(),
                "old".into(),
                "/nodes/1/dag".into(),
                "subworkflow DAG 'b' (version 7) does not exist".into()
            ),
        ]
    );
}

#[test]
fn save_gate_adds_the_rule_to_the_standard_set() {
    assert!(!Gate::standard().rule_names().contains(&"subworkflow_cycle"));
    let names = Gate::for_save(Arc::new(MapCatalog::new())).rule_names();
    assert_eq!(names.last(), Some(&"subworkflow_cycle"));
    assert_eq!(names.len(), Gate::standard().rule_names().len() + 1);
}
