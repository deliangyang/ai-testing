mod common;

use std::sync::Arc;

use common::{dag, edge, finding, findings, validate, validate_with_api_specs};
use gate::rules::MapApiSpecLookup;
use gate::{codes, Severity};
use serde_json::{json, Value};

fn create_with_schema() -> Value {
    json!({"id": "create", "type": "http", "method": "POST", "path": "/orders",
    "response_schema": {"type": "object", "properties": {
        "id": {"type": "string"},
        "items": {"type": "array", "items": {"type": "object", "properties": {"sku": {"type": "string"}}}}
    }}})
}

fn user(id: &str, path: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

#[test]
fn declared_fields_pass() {
    let report = validate(&dag(
        vec![
            create_with_schema(),
            user("u", "/o/${create.output.body.id}/${create.output.body.items[0].sku}?s=${create.output.status}&h=${create.output.headers.x-trace}"),
        ],
        vec![edge("create", "u")],
    ));
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}

#[test]
fn undeclared_body_field_is_an_error_located_at_the_reference() {
    let report = validate(&dag(
        vec![
            create_with_schema(),
            user("u", "/o/${create.output.body.orderId}"),
        ],
        vec![edge("create", "u")],
    ));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::UNKNOWN_FIELD,
            Some("u"),
            Some("/nodes/1/path")
        )]
    );
    assert!(
        report.issues[0].message.contains("output.body.orderId"),
        "{}",
        report.issues[0].message
    );
}

#[test]
fn unknown_top_level_output_field_is_an_error() {
    let report = validate(&dag(
        vec![create_with_schema(), user("u", "/${create.output.data.id}")],
        vec![edge("create", "u")],
    ));
    assert!(report.has_code(codes::UNKNOWN_FIELD));
}

#[test]
fn nested_array_field_mismatch_is_an_error() {
    let report = validate(&dag(
        vec![
            create_with_schema(),
            user("u", "/${create.output.body.items[0].price}"),
        ],
        vec![edge("create", "u")],
    ));
    assert!(report.has_code(codes::UNKNOWN_FIELD));
}

#[test]
fn missing_schema_downgrades_body_references_to_warnings() {
    let report = validate(&dag(
        vec![
            user("create", "/orders"),
            user("u", "/o/${create.output.body.id}?s=${create.output.status}"),
        ],
        vec![edge("create", "u")],
    ));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::UNVERIFIED_FIELD,
            Some("u"),
            Some("/nodes/1/path")
        )]
    );
    assert_eq!(report.issues[0].severity, Severity::Warn);
    assert!(report.passed());
}

#[test]
fn subworkflow_outputs_and_db_rows_are_checked() {
    let report = validate(&dag(
        vec![
            json!({"id": "login", "type": "subworkflow", "dag": {"id": "auth"},
                "outputs": {"token": "${do.output.body.token}"}}),
            json!({"id": "vip", "type": "db_query", "connection": "ro", "sql": "select id",
                "result": "first_row",
                "response_schema": {"type": "object", "properties": {"id": {"type": "integer"}}}}),
            json!({"id": "use", "type": "http", "method": "GET",
                "path": "/${vip.output.row.id}/${vip.output.row.name}",
                "headers": {"A": "${login.output.token}", "B": "${login.output.session}"}}),
        ],
        vec![edge("login", "use"), edge("vip", "use")],
    ));
    assert_eq!(
        findings(&report),
        vec![
            finding(
                codes::UNKNOWN_FIELD,
                Some("use"),
                Some("/nodes/2/headers/B")
            ),
            finding(codes::UNKNOWN_FIELD, Some("use"), Some("/nodes/2/path")),
        ]
    );
}

#[test]
fn api_spec_response_schema_used_when_inline_schema_missing() {
    let spec_schema = json!({"type": "object", "properties": {"id": {"type": "string"}}});
    let lookup =
        Arc::new(MapApiSpecLookup::new().with_active("orders.create", Some(spec_schema.clone())));
    let report = validate_with_api_specs(
        &dag(
            vec![
                json!({"id": "create", "type": "http", "method": "POST", "path": "/orders",
                    "api_spec_id": "orders.create"}),
                user("u", "/o/${create.output.body.id}"),
            ],
            vec![edge("create", "u")],
        ),
        lookup,
    );
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}

#[test]
fn api_spec_schema_takes_priority_over_inline_response_schema() {
    let lookup = Arc::new(MapApiSpecLookup::new().with_active(
        "orders.create",
        Some(json!({"type": "object", "properties": {
            "id": {"type": "string"},
            "total": {"type": "number"}
        }})),
    ));
    let report = validate_with_api_specs(
        &dag(
            vec![
                json!({"id": "create", "type": "http", "method": "POST", "path": "/orders",
                    "api_spec_id": "orders.create",
                    "response_schema": {"type": "object", "properties": {"id": {"type": "string"}}}}),
                user("u", "/${create.output.body.total}"),
            ],
            vec![edge("create", "u")],
        ),
        lookup,
    );
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}

#[test]
fn api_spec_unknown_field_still_errors() {
    let lookup = Arc::new(MapApiSpecLookup::new().with_active(
        "orders.create",
        Some(json!({"type": "object", "properties": {"id": {"type": "string"}}})),
    ));
    let report = validate_with_api_specs(
        &dag(
            vec![
                json!({"id": "create", "type": "http", "method": "POST", "path": "/orders",
                    "api_spec_id": "orders.create"}),
                user("u", "/${create.output.body.missing}"),
            ],
            vec![edge("create", "u")],
        ),
        lookup,
    );
    assert!(report.has_code(codes::UNKNOWN_FIELD));
}

#[test]
fn api_spec_id_without_lookup_falls_back_to_unverified() {
    let report = validate(&dag(
        vec![
            json!({"id": "create", "type": "http", "method": "GET", "path": "/orders",
                "api_spec_id": "orders.create"}),
            user("u", "/${create.output.body.id}"),
        ],
        vec![edge("create", "u")],
    ));
    assert_eq!(
        findings(&report),
        vec![finding(
            codes::UNVERIFIED_FIELD,
            Some("u"),
            Some("/nodes/1/path")
        )]
    );
}

#[test]
fn whole_output_reference_is_always_valid() {
    let report = validate(&dag(
        vec![create_with_schema(), user("u", "/${create.output}")],
        vec![edge("create", "u")],
    ));
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}
