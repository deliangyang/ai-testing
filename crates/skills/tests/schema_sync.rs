#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use model::nodes::HttpMethod;
use serde_json::{json, Value};
use skills::schema::{skill_schema, skill_schema_json};
use skills::SkillPackage;

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/skill.schema.json")
}

#[test]
fn schema_file_is_up_to_date() {
    let generated = skill_schema_json();
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::write(schema_path(), &generated).unwrap();
    }
    let on_disk = std::fs::read_to_string(schema_path())
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        on_disk == generated,
        "schemas/skill.schema.json is stale; regenerate with: \
         UPDATE_SCHEMAS=1 cargo test -p skills --test schema_sync"
    );
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::to_value(skill_schema()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn sample() -> Value {
    json!({
        "manifest": {
            "id": "auth-login",
            "name": "登录",
            "version": "1.0.0",
            "dependencies": []
        },
        "inputs": {
            "fields": [
                {"name": "username", "ty": "string", "required": true}
            ]
        },
        "outputs": {
            "fields": [
                {"name": "token", "ty": "string", "required": true}
            ]
        },
        "dag": {
            "id": "auth-login-inner",
            "nodes": [
                {"id": "login", "type": "http", "method": "POST", "path": "/auth/login"},
                {"id": "check", "type": "assert", "target": "login",
                 "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]}
            ],
            "edges": [{"from": "login", "to": "check"}]
        },
        "assertions": [
            {"id": "http_ok", "target": "login",
             "assertion": {"path": "$.status", "op": "eq", "expected": 200}}
        ]
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
    SkillPackage::from_json_str(&doc.to_string()).unwrap();
}

#[test]
fn schema_rejects_unknown_top_level_keys() {
    let mut doc = sample();
    doc["plugin"] = json!("evil.tsx");
    assert!(!validator().is_valid(&doc));
    assert!(SkillPackage::from_json_str(&doc.to_string()).is_err());
}

#[test]
fn schema_rejects_bad_manifest_version_type() {
    let mut doc = sample();
    doc["manifest"]["version"] = json!(1);
    assert!(!validator().is_valid(&doc));
}

#[test]
fn schema_rejects_invalid_http_method_in_dag() {
    let mut doc = sample();
    doc["dag"]["nodes"][0]["method"] = json!("FETCH");
    assert!(!validator().is_valid(&doc));
}

#[test]
fn parser_accepts_lowercase_http_method_even_when_schema_prefers_uppercase() {
    let mut doc = sample();
    doc["dag"]["nodes"][0]["method"] = json!("post");
    let pkg = SkillPackage::from_json_str(&doc.to_string()).unwrap();
    assert!(matches!(
        pkg.dag.nodes[0].kind,
        model::NodeKind::Http(ref c) if c.method == HttpMethod::Post
    ));
}

#[test]
fn bundled_assertion_rejects_bad_operator() {
    let mut doc = sample();
    doc["assertions"][0]["assertion"]["op"] = json!("approx");
    assert!(!validator().is_valid(&doc));
    assert!(SkillPackage::from_json_str(&doc.to_string()).is_err());
}

#[test]
fn contract_field_rejects_unknown_keys() {
    let mut doc = sample();
    doc["inputs"]["fields"][0]["typo"] = json!(true);
    assert!(!validator().is_valid(&doc));
    assert!(SkillPackage::from_json_str(&doc.to_string()).is_err());
}
