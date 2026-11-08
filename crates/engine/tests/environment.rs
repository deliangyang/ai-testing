#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use engine::environment::{MissingProcessVar, UndefinedVar};
use engine::{EnvError, Environment};
use model::Dag;
use serde_json::json;

fn process(vars: &[(&str, &str)]) -> BTreeMap<String, String> {
    vars.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn parse(yaml: &str, vars: &[(&str, &str)]) -> Result<Environment, EnvError> {
    Environment::parse("local", yaml, &process(vars))
}

#[test]
fn literal_and_process_values_are_resolved() {
    let env = parse(
        "base_url: http://${env:HOST}:8080\nvariables:\n  TENANT: acme\n  API_KEY: ${env:AITEST_KEY}\n  RAW: \"$${not.a.ref}\"\n",
        &[("HOST", "localhost"), ("AITEST_KEY", "s3cret")],
    )
    .unwrap();
    assert_eq!(env.name(), "local");
    assert_eq!(env.base_url(), "http://localhost:8080");
    assert_eq!(env.var("BASE_URL"), Some("http://localhost:8080"));
    assert_eq!(env.var("TENANT"), Some("acme"));
    assert_eq!(env.var("API_KEY"), Some("s3cret"));
    assert_eq!(env.var("RAW"), Some("${not.a.ref}"));
    assert_eq!(env.var("NOPE"), None);
    assert_eq!(
        env.variables().keys().collect::<Vec<_>>(),
        ["API_KEY", "BASE_URL", "RAW", "TENANT"]
    );
    assert!(env.is_from_process("API_KEY") && env.is_from_process("BASE_URL"));
    assert!(!env.is_from_process("TENANT"));
}

#[test]
fn debug_masks_values_from_the_process_environment() {
    let env = parse(
        "base_url: http://h\nvariables:\n  TOKEN: ${env:T}\n  TENANT: acme\n",
        &[("T", "s3cret")],
    )
    .unwrap();
    let shown = format!("{env:?}");
    assert!(!shown.contains("s3cret"), "{shown}");
    assert!(
        shown.contains("\"TOKEN\": \"****\"") && shown.contains("acme"),
        "{shown}"
    );
}

#[test]
fn every_missing_process_variable_is_reported() {
    let err = parse(
        "base_url: ${env:URL}\nvariables:\n  A: ${env:X}-${env:Y}\n  B: ok\n",
        &[("Y", "1")],
    )
    .unwrap_err();
    assert_eq!(
        err,
        EnvError::MissingProcessVars {
            name: "local".into(),
            missing: vec![
                MissingProcessVar {
                    field: "base_url".into(),
                    var: "URL".into()
                },
                MissingProcessVar {
                    field: "variables.A".into(),
                    var: "X".into()
                },
            ],
        }
    );
    assert_eq!(
        err.to_string(),
        "environment 'local': process environment variables not set: URL (used by base_url), X (used by variables.A)"
    );
}

#[test]
fn malformed_files_are_rejected_with_a_location() {
    let cases: &[(&str, &str)] = &[
        ("variables: {}\n", "missing field `base_url`"),
        ("base_url: h\nextra: 1\n", "unknown field `extra`"),
        ("base_url: h\nvariables:\n  A: [1]\n", "variables.A"),
        (
            "base_url: h\nvariables:\n  BASE_URL: x\n",
            "variables.BASE_URL conflicts with base_url",
        ),
        (
            "base_url: h\nvariables:\n  A: ${env:}\n",
            "variables.A: invalid template",
        ),
        (
            "base_url: h\nvariables:\n  A: ${login.output.token}\n",
            "only ${env:...} references are allowed, found ${login.output.token}",
        ),
        (
            "base_url: ${auth.token}\n",
            "base_url: only ${env:...} references are allowed",
        ),
    ];
    for (yaml, expected) in cases {
        let err = parse(yaml, &[]).unwrap_err().to_string();
        assert!(err.contains(expected), "{yaml:?}: {err}");
    }
}

#[test]
fn load_reads_environments_directory() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("environments");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("local.yaml"), "base_url: http://127.0.0.1:1\n").unwrap();
    std::fs::write(dir.join("stg.yaml"), "base_url: https://stg\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

    let env = Environment::load(root.path(), "stg", &process(&[])).unwrap();
    assert_eq!(env.name(), "stg");
    assert_eq!(env.base_url(), "https://stg");

    let err = Environment::load(root.path(), "prod", &process(&[])).unwrap_err();
    assert!(
        matches!(&err, EnvError::NotFound { available, .. } if available == &["local", "stg"]),
        "{err:?}"
    );
    assert!(err.to_string().contains("available: local, stg"));
}

#[test]
fn load_refuses_names_that_escape_the_directory() {
    let root = tempfile::tempdir().unwrap();
    for name in ["", "../secrets", "a/b", "a.b"] {
        let err = Environment::load(root.path(), name, &process(&[])).unwrap_err();
        assert_eq!(err, EnvError::InvalidName(name.into()));
    }
}

#[test]
fn check_dag_lists_undefined_variables() {
    let env = parse("base_url: http://h\nvariables:\n  TENANT: acme\n", &[]).unwrap();
    let dag = Dag::from_json_str(
        &json!({
            "id": "d",
            "nodes": [
                {"id": "a", "type": "http", "method": "GET",
                 "path": "/t/${env:TENANT}", "headers": {"X-Key": "${env:API_KEY}"}},
                {"id": "b", "type": "http", "method": "GET", "path": "${env:BASE_URL}/x?r=${env:REGION}"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let err = env.check_dag(&dag).unwrap_err();
    assert_eq!(
        err,
        EnvError::UndefinedVars {
            name: "local".into(),
            undefined: vec![
                UndefinedVar {
                    node: "a".into(),
                    pointer: "/headers/X-Key".into(),
                    var: "API_KEY".into()
                },
                UndefinedVar {
                    node: "b".into(),
                    pointer: "/path".into(),
                    var: "REGION".into()
                },
            ],
        }
    );
    assert!(err
        .to_string()
        .contains("API_KEY (node 'a' at /headers/X-Key)"));

    let ok = parse(
        "base_url: http://h\nvariables:\n  TENANT: a\n  API_KEY: k\n  REGION: r\n",
        &[],
    )
    .unwrap();
    assert_eq!(ok.check_dag(&dag), Ok(()));
}
