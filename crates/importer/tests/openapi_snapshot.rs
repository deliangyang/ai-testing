#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use importer::{normalize_api_spec, parse_openapi, parse_openapi_json};

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/openapi")
        .join(name);
    fs::read(path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

fn specs_canonical_lines(specs: &[importer::ApiSpec]) -> String {
    let mut normalized: Vec<_> = specs.iter().cloned().map(normalize_api_spec).collect();
    normalized.sort_by(|a, b| a.id.cmp(&b.id));
    normalized
        .into_iter()
        .map(|s| s.to_canonical_json().unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn mini_openapi3_snapshot() {
    let raw = String::from_utf8(fixture_bytes("mini_oas3.json")).unwrap();
    let specs = parse_openapi_json(&raw).unwrap();
    insta::assert_snapshot!(specs_canonical_lines(&specs));
}

#[test]
fn mini_swagger2_snapshot() {
    let raw = String::from_utf8(fixture_bytes("mini_swagger2.json")).unwrap();
    let specs = parse_openapi(&raw).unwrap();
    insta::assert_snapshot!(specs_canonical_lines(&specs));
}

#[test]
fn petstore_minimal_snapshot() {
    let raw = String::from_utf8(fixture_bytes("petstore_minimal.json")).unwrap();
    let specs = parse_openapi_json(&raw).unwrap();
    insta::assert_snapshot!(specs_canonical_lines(&specs));
    assert_eq!(specs.len(), 2);
    assert!(specs
        .iter()
        .any(|s| s.auth_scheme_ref.as_deref() == Some("petstore_auth")));
}

#[test]
fn parse_is_deterministic_across_runs() {
    let raw = String::from_utf8(fixture_bytes("mini_oas3.json")).unwrap();
    let a = specs_canonical_lines(&parse_openapi_json(&raw).unwrap());
    let b = specs_canonical_lines(&parse_openapi_json(&raw).unwrap());
    assert_eq!(a, b);
}
