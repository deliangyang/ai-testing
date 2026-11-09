#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use common::{FixedClock, Timestamp};
use engine::{
    ExecContext, ExecFuture, NodeExecutor, RunOutcome, Scheduler, StoreDagSource,
    SubworkflowExecutor,
};
use model::nodes::{HttpConfig, HttpMethod};
use model::{Dag, Node, NodeKind};
use serde_json::{json, Value};
use skills::{SkillIoContract, SkillManifest, SkillPackage};
use store::{SkillRepository, SkillStatus};

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(Timestamp::from_unix_millis(
        1_794_117_600_000,
    )))
}

fn skill_package(id: &str, semver: &str, http_path: &str) -> SkillPackage {
    SkillPackage {
        manifest: SkillManifest {
            id: id.into(),
            name: id.into(),
            version: semver.into(),
            dependencies: vec![],
        },
        inputs: SkillIoContract { fields: vec![] },
        outputs: SkillIoContract { fields: vec![] },
        dag: Dag {
            id: format!("{id}-run"),
            version: None,
            name: String::new(),
            nodes: vec![Node {
                id: "call".into(),
                name: String::new(),
                kind: NodeKind::Http(HttpConfig {
                    method: HttpMethod::Get,
                    path: http_path.into(),
                    query: Default::default(),
                    headers: Default::default(),
                    body: None,
                    timeout_ms: None,
                    api_spec_id: None,
                    response_schema: None,
                }),
            }],
            edges: vec![],
            meta: Default::default(),
        },
        assertions: vec![],
    }
}

fn seed_skills(db: &store::Db) {
    let repo = SkillRepository::new(db.conn());
    let v1 = repo
        .save(&skill_package("auth", "1.0.0", "/skill-v1"))
        .unwrap();
    repo.activate("auth", v1.info.version, None).unwrap();
    let v2 = repo
        .save(&skill_package("auth", "1.1.0", "/skill-v2"))
        .unwrap();
    repo.activate("auth", v2.info.version, Some(1)).unwrap();
    repo.save(&skill_package("auth", "2.0.0", "/skill-v3"))
        .unwrap();
    assert_eq!(
        repo.versions("auth")
            .unwrap()
            .into_iter()
            .map(|v| (v.version, v.status))
            .collect::<Vec<_>>(),
        [
            (1, SkillStatus::Archived),
            (2, SkillStatus::Active),
            (3, SkillStatus::Pending),
        ]
    );
}

struct EchoRouter {
    sub: SubworkflowExecutor,
}

impl NodeExecutor for EchoRouter {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        if matches!(node.kind, NodeKind::Subworkflow(_)) {
            return self.sub.execute(node, ctx);
        }
        Box::pin(async move { Ok(ctx.vars().resolve_node(node)?.into()) })
    }
}

fn dag(value: Value) -> Dag {
    Dag::from_json_str(&value.to_string()).unwrap()
}

fn sub_skill(id: &str, version_req: Option<&str>) -> Value {
    let mut skill = json!({"id": id});
    if let Some(req) = version_req {
        skill["version_req"] = json!(req);
    }
    json!({
        "id": "wrap",
        "type": "subworkflow",
        "skill": skill,
        "outputs": {"path": "${call.output.path}"}
    })
}

async fn run_with_skill_constraint(version_req: Option<&str>) -> (RunOutcome, String) {
    let clock = clock();
    let db = store::Db::open_in_memory(clock.as_ref()).unwrap();
    seed_skills(&db);
    let db = Arc::new(Mutex::new(db));
    let source = Arc::new(StoreDagSource::new(db, clock));
    let router = Arc::new(EchoRouter {
        sub: SubworkflowExecutor::new(source),
    });
    let root = dag(json!({
        "id": "root",
        "nodes": [sub_skill("auth", version_req)]
    }));
    let outcome = Scheduler::new(router)
        .run("run-skill", &root)
        .await
        .unwrap();
    let path = outcome
        .nodes
        .get("wrap/call")
        .and_then(|n| n.output.as_ref())
        .and_then(|o| o.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    (outcome, path)
}

#[tokio::test]
async fn unpinned_skill_uses_active_manifest() {
    let (outcome, path) = run_with_skill_constraint(None).await;
    assert!(outcome.succeeded(), "{outcome:#?}");
    assert_eq!(path, "/skill-v2");
}

#[tokio::test]
async fn semver_range_resolves_highest_activated_match() {
    let (outcome, path) = run_with_skill_constraint(Some("^1.0.0")).await;
    assert!(outcome.succeeded(), "{outcome:#?}");
    assert_eq!(path, "/skill-v2");
}

#[tokio::test]
async fn exact_semver_can_bind_archived_revision() {
    let (outcome, path) = run_with_skill_constraint(Some("=1.0.0")).await;
    assert!(outcome.succeeded(), "{outcome:#?}");
    assert_eq!(path, "/skill-v1");
}
