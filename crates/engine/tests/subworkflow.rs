#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use common::{FixedClock, Timestamp};
use engine::NodeStatus::{Completed, Failed};
use engine::{
    ExecContext, ExecErrorKind, ExecFuture, NodeExecutor, RunOutcome, Scheduler, StoreDagSource,
    SubworkflowExecutor,
};
use model::{Dag, Node, NodeKind};
use serde_json::{json, Value};
use store::{DagRepository, Db};

/// What a leaf node saw: id, frame path (child DAG ids) and the keys of its outputs.
type Seen = (String, Vec<String>, Vec<String>);

/// Subworkflow nodes go to the real executor; other nodes output their resolved input.
struct Router {
    sub: SubworkflowExecutor,
    seen: Mutex<Vec<Seen>>,
}

impl NodeExecutor for Router {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        if matches!(node.kind, NodeKind::Subworkflow(_)) {
            return self.sub.execute(node, ctx);
        }
        self.seen.lock().unwrap().push((
            node.id.clone(),
            ctx.frames.iter().map(|f| f.dag.clone()).collect(),
            ctx.outputs.keys().cloned().collect(),
        ));
        Box::pin(async move { Ok(ctx.vars().resolve_node(node)?.into()) })
    }
}

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(Timestamp::from_unix_millis(
        1_794_117_600_000,
    )))
}

fn dag(value: Value) -> Dag {
    Dag::from_json_str(&value.to_string()).unwrap()
}

fn http(id: &str, path: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

fn sub(id: &str, dag_id: &str, inputs: Value, outputs: Value) -> Value {
    json!({"id": id, "type": "subworkflow", "dag": {"id": dag_id},
           "inputs": inputs, "outputs": outputs})
}

/// Saves every DAG and activates it, except ids listed in `pending_only`.
fn store_with(dags: &[Dag], pending_only: &[&str]) -> Arc<Mutex<Db>> {
    let clock = clock();
    let db = Db::open_in_memory(clock.as_ref()).unwrap();
    {
        let repo = DagRepository::new(db.conn(), clock.as_ref());
        for d in dags {
            let saved = repo.save(d, "test").unwrap();
            if !pending_only.contains(&d.id.as_str()) {
                repo.activate(&d.id, saved.info.version, None).unwrap();
            }
        }
    }
    Arc::new(Mutex::new(db))
}

/// auth: token(input.user)            -> outputs {token}
/// checkout: login=auth(input.user) -> pay(login.token) -> outputs {paid, token}
fn library() -> Vec<Dag> {
    vec![
        dag(json!({"id": "auth", "nodes": [http("token", "/token/${input.output.user}")]})),
        dag(json!({
            "id": "checkout",
            "nodes": [
                sub("login", "auth", json!({"user": "${input.output.user}"}),
                    json!({"token": "${token.output.path}"})),
                http("pay", "/pay/${login.output.token}")
            ],
            "edges": [{"from": "login", "to": "pay"}]
        })),
    ]
}

async fn run_root(db: Arc<Mutex<Db>>, root: &Dag) -> (RunOutcome, Arc<Router>) {
    let source = Arc::new(StoreDagSource::new(db, clock()));
    let router = Arc::new(Router {
        sub: SubworkflowExecutor::new(source),
        seen: Mutex::new(Vec::new()),
    });
    let outcome = Scheduler::new(router.clone())
        .run("run-1", root)
        .await
        .unwrap();
    (outcome, router)
}

fn root() -> Dag {
    dag(json!({
        "id": "root",
        "nodes": [
            http("start", "u-1"),
            sub("buy", "checkout", json!({"user": "${start.output.path}", "n": 2}),
                json!({"paid": "${pay.output.path}", "token": "${login.output.token}",
                       "user": "${input.output.user}"})),
            http("after", "/after${buy.output.paid}")
        ],
        "edges": [{"from": "start", "to": "buy"}, {"from": "buy", "to": "after"}]
    }))
}

#[tokio::test]
async fn two_level_nesting_maps_inputs_and_outputs() {
    let (outcome, router) = run_root(store_with(&library(), &[]), &root()).await;
    assert!(outcome.succeeded(), "{outcome:#?}");
    assert_eq!(
        outcome.nodes.keys().collect::<Vec<_>>(),
        [
            "after",
            "buy",
            "buy/login",
            "buy/login/token",
            "buy/pay",
            "start"
        ]
    );
    assert_eq!(
        outcome.nodes["buy"].output,
        Some(json!({"paid": "/pay//token/u-1", "token": "/token/u-1", "user": "u-1"}))
    );
    assert_eq!(
        outcome.nodes["buy/login"].output,
        Some(json!({"token": "/token/u-1"}))
    );
    assert_eq!(outcome.nodes["buy/login/token"].status, Completed);
    assert_eq!(
        outcome.nodes["after"].output.as_ref().unwrap()["path"],
        json!("/after/pay//token/u-1")
    );

    let seen = router.seen.lock().unwrap().clone();
    let of = |id: &str| seen.iter().find(|(n, _, _)| n == id).cloned().unwrap();
    assert_eq!(of("token").1, ["checkout", "auth"]);
    assert_eq!(of("token").2, ["input"]);
    assert_eq!(of("pay").1, ["checkout"]);
    assert_eq!(of("pay").2, ["input", "login"]);
    assert!(of("start").1.is_empty());
}

#[tokio::test]
async fn child_cannot_read_parent_outputs() {
    let leaky = dag(json!({"id": "leaky", "nodes": [http("leak", "/x/${start.output.path}")]}));
    let root = dag(json!({
        "id": "root",
        "nodes": [http("start", "u-1"), sub("child", "leaky", json!({}), json!({}))],
        "edges": [{"from": "start", "to": "child"}]
    }));
    let (outcome, _) = run_root(store_with(&[leaky], &[]), &root).await;

    let child = &outcome.nodes["child"];
    assert_eq!(child.status, Failed);
    let err = child.error.as_ref().unwrap();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(err.message, "subworkflow 'leaky' failed at leak");
    assert_eq!(
        err.detail,
        Some(json!({"dag": {"id": "leaky", "version": 1}, "failed": ["leak"]}))
    );
    assert!(
        err.children.is_empty(),
        "children are moved into the outcome"
    );
    let leak = &outcome.nodes["child/leak"];
    assert_eq!(leak.status, Failed);
    assert_eq!(
        leak.error.as_ref().unwrap().detail.as_ref().unwrap()["reason"],
        json!("missing_node")
    );
}

#[tokio::test]
async fn output_mapping_errors_point_at_the_output() {
    let child = dag(json!({"id": "child", "nodes": [http("a", "/a")]}));
    let root = dag(json!({
        "id": "root",
        "nodes": [sub("s", "child", json!({}), json!({"x": "${a.output.nope}"}))]
    }));
    let (outcome, _) = run_root(store_with(&[child], &[]), &root).await;
    let err = outcome.nodes["s"].error.as_ref().unwrap();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(err.detail.as_ref().unwrap()["pointer"], json!("/outputs/x"));
    assert_eq!(outcome.nodes["s/a"].status, Completed);
}

#[tokio::test]
async fn only_activated_versions_run() {
    let child = dag(json!({"id": "draft", "nodes": [http("a", "/a")]}));
    let unpinned = dag(json!({"id": "root", "nodes": [sub("s", "draft", json!({}), json!({}))]}));
    let (outcome, _) = run_root(
        store_with(std::slice::from_ref(&child), &["draft"]),
        &unpinned,
    )
    .await;
    let err = outcome.nodes["s"].error.as_ref().unwrap();
    assert_eq!(err.kind, ExecErrorKind::Internal);
    assert!(
        err.message.contains("has no active version"),
        "{}",
        err.message
    );

    let pinned = dag(json!({"id": "root", "nodes": [
        {"id": "s", "type": "subworkflow", "dag": {"id": "draft", "version": 1}}
    ]}));
    let (outcome, _) = run_root(store_with(&[child], &["draft"]), &pinned).await;
    let err = outcome.nodes["s"].error.as_ref().unwrap();
    assert!(
        err.message.contains("pending and has never been activated"),
        "{}",
        err.message
    );
    assert!(!outcome.nodes.contains_key("s/a"));
}

#[tokio::test]
async fn recursive_subworkflows_are_stopped_at_run_time() {
    let looping = dag(json!({"id": "loop", "nodes": [sub("again", "loop", json!({}), json!({}))]}));
    let root = dag(json!({"id": "root", "nodes": [sub("s", "loop", json!({}), json!({}))]}));
    let (outcome, _) = run_root(store_with(&[looping], &[]), &root).await;
    assert_eq!(outcome.nodes["s"].status, Failed);
    let inner = outcome.nodes["s/again"].error.as_ref().unwrap();
    assert_eq!(inner.message, "subworkflow cycle: loop -> loop");
}
