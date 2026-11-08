#![allow(dead_code, clippy::unwrap_used)]

use gate::{Gate, Report};
use model::Dag;
use serde_json::{json, Value};

pub fn http(id: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": "/"})
}

pub fn edge(from: &str, to: &str) -> Value {
    json!({"from": from, "to": to})
}

pub fn dag(nodes: Vec<Value>, edges: Vec<Value>) -> Dag {
    let doc = json!({"id": "t", "nodes": nodes, "edges": edges});
    Dag::from_json_str(&doc.to_string()).unwrap()
}

/// Standard gate minus `completeness.*`, so rule-focused tests can use minimal DAGs.
pub fn validate(dag: &Dag) -> Report {
    let report = validate_all(dag);
    Report::new(
        report
            .issues
            .into_iter()
            .filter(|i| !i.code.starts_with("completeness."))
            .collect(),
    )
}

pub fn validate_all(dag: &Dag) -> Report {
    Gate::standard().validate(dag)
}

/// (code, node_id, json_path) triples for compact assertions.
pub fn findings(report: &Report) -> Vec<(String, Option<String>, Option<String>)> {
    report
        .issues
        .iter()
        .map(|i| (i.code.clone(), i.node_id.clone(), i.json_path.clone()))
        .collect()
}

pub fn error_findings(report: &Report) -> Vec<(String, Option<String>, Option<String>)> {
    report
        .errors()
        .map(|i| (i.code.clone(), i.node_id.clone(), i.json_path.clone()))
        .collect()
}

pub fn finding(
    code: &str,
    node: Option<&str>,
    path: Option<&str>,
) -> (String, Option<String>, Option<String>) {
    (
        code.to_owned(),
        node.map(str::to_owned),
        path.map(str::to_owned),
    )
}
