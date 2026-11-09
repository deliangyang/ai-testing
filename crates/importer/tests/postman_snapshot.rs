#![allow(clippy::unwrap_used)]

use std::fs;

use importer::{api_specs_canonical_lines, parse_postman_collection_str, PostmanParseError};

#[test]
fn sample_collection_v21_snapshot() {
    let raw = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/postman/sample_v21.json"),
    )
    .unwrap();
    let specs = parse_postman_collection_str(&raw).unwrap();
    assert_eq!(specs.len(), 5);
    let snapshot = api_specs_canonical_lines(&specs).unwrap();
    insta::assert_snapshot!(snapshot);
}

#[test]
fn parse_is_deterministic_twice() {
    let raw = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/postman/sample_v21.json"),
    )
    .unwrap();
    let a = api_specs_canonical_lines(&parse_postman_collection_str(&raw).unwrap()).unwrap();
    let b = api_specs_canonical_lines(&parse_postman_collection_str(&raw).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn rejects_v20_schema() {
    let json = r#"{
      "info": {
        "name": "Old",
        "schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"
      },
      "item": []
    }"#;
    let err = parse_postman_collection_str(json).unwrap_err();
    assert!(matches!(err, PostmanParseError::UnsupportedSchema { .. }));
}
