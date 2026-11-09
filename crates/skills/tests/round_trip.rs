#![allow(clippy::unwrap_used, clippy::expect_used)]

use model::nodes::{HttpMethod, Operator};
use model::NodeKind;
use skills::{
    BundledAssertion, SkillField, SkillIoContract, SkillManifest, SkillPackage, SkillValueType,
};

const SAMPLE_YAML: &str = r#"
manifest:
  id: auth-login
  name: 用户登录
  version: "1.0.0"
  dependencies:
    - id: shared-http
      version_req: "^1.0.0"
inputs:
  fields:
    - name: username
      ty: string
      required: true
    - name: password
      ty: string
      required: true
outputs:
  fields:
    - name: token
      ty: string
      required: true
dag:
  id: auth-login-inner
  nodes:
    - id: login
      type: http
      method: POST
      path: /auth/login
    - id: check
      type: assert
      target: login
      assertions:
        - path: $.status
          op: eq
          expected: 200
  edges:
    - from: login
      to: check
assertions:
  - id: http_ok
    target: login
    assertion:
      path: $.status
      op: eq
      expected: 200
  - id: has_token
    target: login
    assertion:
      path: $.body.token
      op: exists
"#;

#[test]
fn yaml_json_round_trip_is_lossless() {
    let from_yaml = SkillPackage::from_yaml_str(SAMPLE_YAML).unwrap();
    assert_eq!(from_yaml.manifest.version, "1.0.0");
    assert_eq!(from_yaml.manifest.dependencies.len(), 1);
    assert_eq!(from_yaml.inputs.fields.len(), 2);
    assert_eq!(from_yaml.outputs.fields[0].name, "token");
    assert_eq!(from_yaml.assertions.len(), 2);

    let json = from_yaml.to_json_pretty().unwrap();
    let from_json = SkillPackage::from_json_str(&json).unwrap();
    assert_eq!(from_yaml, from_json);

    let yaml = from_json.to_yaml().unwrap();
    assert_eq!(SkillPackage::from_yaml_str(&yaml).unwrap(), from_yaml);
}

#[test]
fn constructs_minimal_package_in_memory() {
    use model::nodes::{AssertConfig, Condition, HttpConfig};
    use model::{Dag, Edge, Node};

    let pkg = SkillPackage {
        manifest: SkillManifest {
            id: "ping".into(),
            name: "Ping".into(),
            version: "0.1.0".into(),
            dependencies: vec![],
        },
        inputs: SkillIoContract { fields: vec![] },
        outputs: SkillIoContract {
            fields: vec![SkillField {
                name: "ok".into(),
                ty: SkillValueType::Boolean,
                required: true,
                description: None,
                json_schema: None,
            }],
        },
        dag: Dag {
            id: "ping-inner".into(),
            version: None,
            name: String::new(),
            nodes: vec![
                Node {
                    id: "call".into(),
                    name: String::new(),
                    kind: NodeKind::Http(HttpConfig {
                        method: HttpMethod::Get,
                        path: "/ping".into(),
                        query: Default::default(),
                        headers: Default::default(),
                        body: None,
                        timeout_ms: None,
                        api_spec_id: None,
                        response_schema: None,
                    }),
                },
                Node {
                    id: "ok".into(),
                    name: String::new(),
                    kind: NodeKind::Assert(AssertConfig {
                        target: "call".into(),
                        assertions: vec![Condition {
                            path: "$.status".into(),
                            op: Operator::Eq,
                            expected: Some(serde_json::json!(200)),
                        }],
                    }),
                },
            ],
            edges: vec![Edge {
                from: "call".into(),
                to: "ok".into(),
            }],
            meta: Default::default(),
        },
        assertions: vec![BundledAssertion {
            id: "status_200".into(),
            target: "call".into(),
            assertion: Condition {
                path: "$.status".into(),
                op: Operator::Eq,
                expected: Some(serde_json::json!(200)),
            },
        }],
    };
    let json = pkg.to_json_pretty().unwrap();
    assert_eq!(SkillPackage::from_json_str(&json).unwrap(), pkg);
}
