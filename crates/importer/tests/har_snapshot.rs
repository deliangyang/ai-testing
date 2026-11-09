#![allow(clippy::unwrap_used)]

use std::fs;

use importer::{api_specs_canonical_lines, parse_har_str, HarParseError};

#[test]
fn sample_har_snapshot() {
    let raw = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/har/sample.har.json"),
    )
    .unwrap();
    let specs = parse_har_str(&raw).unwrap();
    assert_eq!(specs.len(), 3);
    let snapshot = api_specs_canonical_lines(&specs).unwrap();
    insta::assert_snapshot!(snapshot);
}

#[test]
fn parse_is_deterministic_twice() {
    let raw = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/har/sample.har.json"),
    )
    .unwrap();
    let a = api_specs_canonical_lines(&parse_har_str(&raw).unwrap()).unwrap();
    let b = api_specs_canonical_lines(&parse_har_str(&raw).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn rejects_missing_log() {
    let har = r#"{"not_log": {}}"#;
    let err = parse_har_str(har).unwrap_err();
    assert!(matches!(err, HarParseError::MissingLog));
}
