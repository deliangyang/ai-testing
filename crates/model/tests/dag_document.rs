#![allow(clippy::unwrap_used, clippy::expect_used)]

use model::{Dag, Format, ModelError, NodeKind};

const SAMPLE_YAML: &str = r#"
id: order-pay
name: 下单并支付
nodes:
  - id: create
    name: 创建订单
    type: http
    method: POST
    path: /orders
  - id: wait_paid
    type: poll
    request: { method: GET, path: /orders/1 }
    condition: { path: $.body.status, op: eq, expected: PAID }
    backoff_ms: [500, 1000]
    timeout_ms: 30000
  - id: check
    type: assert
    target: create
    assertions:
      - { path: $.status, op: eq, expected: 201 }
  - id: lookup
    type: db_query
    connection: orders-ro
    sql: select 1
  - id: login
    type: subworkflow
    dag: auth-login
  - id: cleanup
    type: cleanup
edges:
  - { from: create, to: wait_paid }
  - { from: wait_paid, to: check }
meta:
  owner: qa
"#;

fn parse_err(result: Result<Dag, ModelError>) -> (Format, String, String) {
    match result.expect_err("expected parse error") {
        ModelError::Parse {
            format,
            path,
            message,
        } => (format, path, message),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn yaml_parses_all_six_node_types() {
    let dag = Dag::from_yaml_str(SAMPLE_YAML).unwrap();
    let types: Vec<_> = dag.nodes.iter().map(|n| n.kind.type_name()).collect();
    assert_eq!(types, NodeKind::TYPE_NAMES);
    assert_eq!(dag.nodes[0].name, "创建订单");
    assert_eq!(dag.edges.len(), 2);
    assert_eq!(dag.version, None);
    assert!(matches!(
        dag.node("check").unwrap().kind,
        NodeKind::Assert(_)
    ));
}

#[test]
fn yaml_json_round_trip_is_lossless() {
    let from_yaml = Dag::from_yaml_str(SAMPLE_YAML).unwrap();
    let json = from_yaml.to_json_pretty().unwrap();
    let from_json = Dag::from_json_str(&json).unwrap();
    assert_eq!(from_yaml, from_json);

    let yaml = from_json.to_yaml().unwrap();
    assert_eq!(Dag::from_yaml_str(&yaml).unwrap(), from_yaml);
}

#[test]
fn node_serializes_as_flat_object_with_type_tag() {
    let dag = Dag::from_yaml_str(SAMPLE_YAML).unwrap();
    let value = serde_json::to_value(&dag.nodes[0]).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "id": "create", "name": "创建订单", "type": "http",
            "method": "POST", "path": "/orders"
        })
    );
}

#[test]
fn unknown_node_type_is_rejected_with_location() {
    let input = r#"{"id":"d","nodes":[{"id":"a","type":"cleanup"},{"id":"b","type":"grpc"}]}"#;
    let (format, path, message) = parse_err(Dag::from_json_str(input));
    assert_eq!(format, Format::Json);
    assert_eq!(path, "nodes[1]");
    assert!(message.contains("unknown node type 'grpc'"), "{message}");
}

#[test]
fn unknown_config_field_is_rejected_with_node_id() {
    let input =
        "id: d\nnodes:\n  - id: a\n    type: http\n    method: GET\n    path: /\n    retries: 3\n";
    let (format, path, message) = parse_err(Dag::from_yaml_str(input));
    assert_eq!(format, Format::Yaml);
    assert_eq!(path, "nodes[0]");
    assert!(message.contains("node 'a'"), "{message}");
    assert!(message.contains("retries"), "{message}");
}

#[test]
fn unknown_top_level_field_is_rejected() {
    let (_, path, message) = parse_err(Dag::from_json_str(r#"{"id":"d","nodes":[],"x":1}"#));
    assert_eq!(path, "x");
    assert!(message.contains("unknown field"), "{message}");
}

#[test]
fn missing_node_type_is_rejected() {
    let (_, path, message) = parse_err(Dag::from_json_str(r#"{"id":"d","nodes":[{"id":"a"}]}"#));
    assert_eq!(path, "nodes[0]");
    assert!(message.contains("type"), "{message}");
}
