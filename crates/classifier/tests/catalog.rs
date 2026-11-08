#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use classifier::{catalog_markdown, ErrorCode, FailureCategory};

const BEGIN: &str = "<!-- BEGIN GENERATED: cargo test -p classifier --test catalog -->\n";
const END: &str = "<!-- END GENERATED -->";

fn doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/error-codes.md")
}

/// Regenerate with `UPDATE_DOCS=1 cargo test -p classifier --test catalog`.
#[test]
fn error_codes_doc_matches_the_catalog() {
    let path = doc_path();
    let doc = std::fs::read_to_string(&path).expect("docs/error-codes.md exists");
    let doc = doc.replace("\r\n", "\n");
    let start = doc.find(BEGIN).expect("begin marker") + BEGIN.len();
    let end = doc.find(END).expect("end marker");
    let generated = catalog_markdown();
    if std::env::var_os("UPDATE_DOCS").is_some() {
        let updated = format!("{}{generated}{}", &doc[..start], &doc[end..]);
        std::fs::write(&path, updated).unwrap();
        return;
    }
    assert_eq!(
        &doc[start..end],
        generated,
        "docs/error-codes.md is stale; rerun with UPDATE_DOCS=1"
    );
}

#[test]
fn categories_are_the_seven_of_the_design() {
    let names: Vec<&str> = FailureCategory::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        names,
        [
            "connection_error",
            "timeout",
            "assertion_failed_status",
            "assertion_failed_body_shape",
            "capture_error",
            "business_rule_violation",
            "unknown",
        ]
    );
}

#[test]
fn names_round_trip_through_text_and_json() {
    for category in FailureCategory::ALL {
        assert_eq!(
            category.as_str().parse::<FailureCategory>().unwrap(),
            category
        );
        assert_eq!(
            serde_json::to_value(category).unwrap(),
            serde_json::json!(category.as_str())
        );
    }
    for code in ErrorCode::ALL {
        assert_eq!(code.as_str().parse::<ErrorCode>().unwrap(), code);
        assert_eq!(
            serde_json::to_value(code).unwrap(),
            serde_json::json!(code.as_str())
        );
    }
    assert!("Timeout".parse::<FailureCategory>().is_err());
    assert_eq!(
        "conn_refused".parse::<ErrorCode>().unwrap_err().to_string(),
        "unknown error code 'conn_refused'"
    );
}

#[test]
fn codes_are_unique_and_every_category_has_one() {
    let names: BTreeSet<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(names.len(), ErrorCode::ALL.len());
    for category in FailureCategory::ALL {
        assert!(category.codes().next().is_some(), "{category} has no code");
    }
    assert_eq!(ErrorCode::Unknown.category(), FailureCategory::Unknown);
}

#[test]
fn codes_are_grouped_by_category_in_catalog_order() {
    let mut seen = Vec::new();
    for code in ErrorCode::ALL {
        if seen.last() != Some(&code.category()) {
            assert!(
                !seen.contains(&code.category()),
                "{code} is separated from its category"
            );
            seen.push(code.category());
        }
    }
    assert_eq!(seen, FailureCategory::ALL);
}
