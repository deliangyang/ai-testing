#![allow(clippy::unwrap_used)]

use common::clock::FixedClock;
use model::nodes::{AssertConfig, Condition, HttpConfig};
use model::nodes::{HttpMethod, Operator};
use model::{Dag, Edge, Node, NodeKind};
use skills::{
    BundledAssertion, SkillField, SkillIoContract, SkillManifest, SkillPackage, SkillValueType,
};
use store::{SkillRepository, SkillStatus, StoreError};

fn clock() -> FixedClock {
    FixedClock::new(common::clock::Timestamp::from_unix_millis(
        1_700_000_000_000,
    ))
}

fn sample_package(id: &str, semver: &str) -> SkillPackage {
    SkillPackage {
        manifest: SkillManifest {
            id: id.into(),
            name: format!("{id} name"),
            version: semver.into(),
            dependencies: vec![],
        },
        inputs: SkillIoContract {
            fields: vec![SkillField {
                name: "username".into(),
                ty: SkillValueType::String,
                required: true,
                description: None,
                json_schema: None,
            }],
        },
        outputs: SkillIoContract { fields: vec![] },
        dag: Dag {
            id: format!("{id}-inner"),
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
            id: "status".into(),
            target: "call".into(),
            assertion: Condition {
                path: "$.status".into(),
                op: Operator::Eq,
                expected: Some(serde_json::json!(200)),
            },
        }],
    }
}

fn statuses(repo: &SkillRepository<'_>, id: &str) -> Vec<(u32, SkillStatus)> {
    repo.versions(id)
        .unwrap()
        .into_iter()
        .map(|v| (v.version, v.status))
        .collect()
}

#[test]
fn save_assigns_activation_revisions_independent_of_manifest_semver() {
    let db = store::Db::open_in_memory(&clock()).unwrap();
    let repo = SkillRepository::new(db.conn());

    let mut first = sample_package("auth-login", "1.0.0");
    let saved = repo.save(&first).unwrap();
    assert_eq!(saved.info.version, 1);
    assert_eq!(saved.info.manifest_version, "1.0.0");
    assert_eq!(saved.info.status, SkillStatus::Pending);

    first.manifest.version = "2.0.0".into();
    let second = repo.save(&first).unwrap();
    assert_eq!(second.info.version, 2);
    assert_eq!(second.info.manifest_version, "2.0.0");

    let loaded = repo.get("auth-login", 1).unwrap().unwrap();
    assert_eq!(loaded.package.manifest.version, "1.0.0");
    assert_eq!(loaded.package.inputs.fields[0].name, "username");
}

#[test]
fn contracts_are_stored_in_dedicated_columns() {
    let db = store::Db::open_in_memory(&clock()).unwrap();
    let repo = SkillRepository::new(db.conn());
    repo.save(&sample_package("ping", "0.1.0")).unwrap();

    let input: String = db
        .conn()
        .query_row(
            "SELECT input_contract_json FROM skills WHERE id = 'ping' AND version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(input.contains("username"));

    let content: String = db
        .conn()
        .query_row(
            "SELECT content_json FROM skills WHERE id = 'ping' AND version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!content.contains("input_contract"));
    assert!(content.contains("manifest"));
    assert!(content.contains("dag"));
}

#[test]
fn activate_keeps_one_active_per_skill_id() {
    let db = store::Db::open_in_memory(&clock()).unwrap();
    let repo = SkillRepository::new(db.conn());
    repo.save(&sample_package("orders", "1.0.0")).unwrap();
    repo.save(&sample_package("orders", "1.1.0")).unwrap();

    repo.activate("orders", 1, None).unwrap();
    assert_eq!(
        statuses(&repo, "orders"),
        [(1, SkillStatus::Active), (2, SkillStatus::Pending),]
    );

    repo.activate("orders", 2, Some(1)).unwrap();
    assert_eq!(
        statuses(&repo, "orders"),
        [(1, SkillStatus::Archived), (2, SkillStatus::Active),]
    );
    assert!(repo.active("orders").unwrap().unwrap().info.version == 2);
}

#[test]
fn activate_conflict_when_expected_active_mismatch() {
    let db = store::Db::open_in_memory(&clock()).unwrap();
    let repo = SkillRepository::new(db.conn());
    repo.save(&sample_package("x", "1.0.0")).unwrap();
    repo.save(&sample_package("x", "1.0.1")).unwrap();
    repo.activate("x", 1, None).unwrap();

    let err = repo.activate("x", 2, None).unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)));
    assert_eq!(
        statuses(&repo, "x"),
        [(1, SkillStatus::Active), (2, SkillStatus::Pending)]
    );
}

#[test]
fn list_orders_by_id_then_revision() {
    let db = store::Db::open_in_memory(&clock()).unwrap();
    let repo = SkillRepository::new(db.conn());
    repo.save(&sample_package("b", "1.0.0")).unwrap();
    repo.save(&sample_package("a", "1.0.0")).unwrap();
    repo.save(&sample_package("a", "1.0.1")).unwrap();

    let rows: Vec<(String, u32)> = repo
        .list()
        .unwrap()
        .into_iter()
        .map(|v| (v.id, v.version))
        .collect();
    assert_eq!(
        rows,
        [
            ("a".to_owned(), 1),
            ("a".to_owned(), 2),
            ("b".to_owned(), 1),
        ],
    );
}
