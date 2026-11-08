#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use model::schema::{dag_schema, dag_schema_json};
use serde_json::{json, Value};

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/dag.schema.json")
}

#[test]
fn schema_file_is_up_to_date() {
    let generated = dag_schema_json();
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::write(schema_path(), &generated).unwrap();
    }
    let on_disk = std::fs::read_to_string(schema_path())
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        on_disk == generated,
        "schemas/dag.schema.json is stale; regenerate with: \
         UPDATE_SCHEMAS=1 cargo test -p model --test schema_sync"
    );
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::to_value(dag_schema()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn sample() -> Value {
    json!({
        "id": "order-pay",
        "nodes": [
            {"id": "create", "type": "http", "method": "POST", "path": "/orders",
             "response_schema": {"type": "object"}},
            {"id": "wait", "type": "poll",
             "request": {"method": "GET", "path": "/orders/${create.output.body.id}"},
             "condition": {"path": "$.body.status", "op": "eq", "expected": "PAID"},
             "backoff_ms": [500, 1000], "timeout_ms": 30000},
            {"id": "check", "type": "assert", "target": "create",
             "assertions": [{"path": "$.status", "op": "eq", "expected": 201}]},
            {"id": "q", "type": "db_query", "connection": "ro", "sql": "select 1", "result": "scalar"},
            {"id": "sub", "type": "subworkflow", "dag": {"id": "login", "version": 2},
             "inputs": {"u": "x"}, "outputs": {"token": "${a.output.body.t}"}},
            {"id": "cleanup", "type": "cleanup",
             "requests": [{"method": "DELETE", "path": "/orders/1"}]}
        ],
        "edges": [{"from": "create", "to": "wait"}]
    })
}

#[test]
fn sample_document_satisfies_schema_and_parses() {
    let doc = sample();
    let errors: Vec<String> = validator()
        .iter_errors(&doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model::Dag::from_json_str(&doc.to_string()).unwrap();
}

type Mutation = Box<dyn Fn(&mut Value)>;

#[test]
fn schema_rejects_what_the_parser_rejects() {
    let validator = validator();
    let mutations: Vec<(&str, Mutation)> = vec![
        (
            "unknown node type",
            Box::new(|d| d["nodes"][0]["type"] = json!("grpc")),
        ),
        (
            "unknown config key",
            Box::new(|d| d["nodes"][0]["retries"] = json!(3)),
        ),
        (
            "key from another node type",
            Box::new(|d| d["nodes"][0]["timeout_ms"] = json!("x")),
        ),
        (
            "poll without backoff",
            Box::new(|d| {
                d["nodes"][1].as_object_mut().unwrap().remove("backoff_ms");
            }),
        ),
        (
            "bad operator",
            Box::new(|d| d["nodes"][2]["assertions"][0]["op"] = json!("approx")),
        ),
        (
            "missing node id",
            Box::new(|d| {
                d["nodes"][5].as_object_mut().unwrap().remove("id");
            }),
        ),
        (
            "unknown top-level key",
            Box::new(|d| d["owner"] = json!("qa")),
        ),
    ];
    for (name, mutate) in mutations {
        let mut doc = sample();
        mutate(&mut doc);
        assert!(!validator.is_valid(&doc), "schema accepted: {name}");
        assert!(
            model::Dag::from_json_str(&doc.to_string()).is_err(),
            "parser accepted: {name}"
        );
    }
}
