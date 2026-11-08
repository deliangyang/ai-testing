#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{FixedClock, Timestamp, TokioSleeper};
use engine::NodeStatus::Completed;
use engine::{
    Environment, ExecErrorKind, HttpExecutor, NodeRouter, NodeStatus, RunOutcome, Scheduler,
    StoreDagSource, SubworkflowExecutor,
};
use model::Dag;
use serde_json::{json, Value};
use store::{DagRepository, Db};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(Timestamp::from_unix_millis(
        1_794_117_600_000,
    )))
}

fn dag(value: Value) -> Dag {
    Dag::from_json_str(&value.to_string()).unwrap()
}

fn router(db: Option<Arc<Mutex<Db>>>) -> NodeRouter {
    let clock = clock();
    let http = Arc::new(
        HttpExecutor::new(clock.clone())
            .unwrap()
            .with_default_timeout(Duration::from_secs(5)),
    );
    let router = NodeRouter::new(http, clock.clone(), Arc::new(TokioSleeper));
    match db {
        Some(db) => router.with_subworkflows(SubworkflowExecutor::new(Arc::new(
            StoreDagSource::new(db, clock),
        ))),
        None => router,
    }
}

async fn run(router: NodeRouter, server: &MockServer, root: &Dag) -> RunOutcome {
    let env = Environment::new("local", &server.uri(), BTreeMap::new()).unwrap();
    Scheduler::new(Arc::new(router))
        .with_environment(env)
        .with_max_parallel(4)
        .run("run-1", root)
        .await
        .unwrap()
}

async fn serve(server: &MockServer, verb: &str, route: &str, body: Value) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn every_node_type_reaches_its_executor() {
    let server = MockServer::start().await;
    serve(&server, "POST", "/login", json!({"token": "t-1"})).await;
    serve(&server, "POST", "/orders", json!({"id": "o-1"})).await;
    serve(&server, "GET", "/orders/o-1", json!({"status": "PAID"})).await;
    serve(&server, "DELETE", "/orders/o-1", json!({})).await;

    let clock = clock();
    let db = Db::open_in_memory(clock.as_ref()).unwrap();
    {
        let repo = DagRepository::new(db.conn(), clock.as_ref());
        let auth = dag(json!({"id": "auth", "nodes": [
            {"id": "login", "type": "http", "method": "POST", "path": "/login"}
        ]}));
        let saved = repo.save(&auth, "test").unwrap();
        repo.activate("auth", saved.info.version, None).unwrap();
    }

    let root = dag(json!({
        "id": "order",
        "nodes": [
            {"id": "session", "type": "subworkflow", "dag": {"id": "auth"},
             "outputs": {"token": "${login.output.body.token}"}},
            {"id": "create", "type": "http", "method": "POST", "path": "/orders",
             "headers": {"authorization": "Bearer ${session.output.token}"}},
            {"id": "wait", "type": "poll",
             "request": {"method": "GET", "path": "/orders/${create.output.body.id}"},
             "condition": {"path": "$.body.status", "op": "eq", "expected": "PAID"},
             "backoff_ms": [10], "timeout_ms": 5000},
            {"id": "check", "type": "assert", "target": "create",
             "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]},
            {"id": "teardown", "type": "cleanup",
             "requests": [{"method": "DELETE", "path": "/orders/${create.output.body.id}"}]}
        ],
        "edges": [
            {"from": "session", "to": "create"},
            {"from": "create", "to": "wait"},
            {"from": "create", "to": "check"},
            {"from": "wait", "to": "teardown"}
        ]
    }));
    let outcome = run(router(Some(Arc::new(Mutex::new(db)))), &server, &root).await;
    assert!(outcome.succeeded(), "{outcome:#?}");
    assert_eq!(
        outcome.nodes["session"].output,
        Some(json!({"token": "t-1"}))
    );
    assert_eq!(
        outcome.nodes["wait"].output.as_ref().unwrap()["body"]["status"],
        "PAID"
    );
    let deletes = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method.as_str() == "DELETE")
        .count();
    assert_eq!(deletes, 1);
    assert!(outcome.nodes.values().all(|n| n.status == Completed));
}

#[tokio::test]
async fn node_types_without_an_executor_fail_as_unsupported() {
    let server = MockServer::start().await;
    let root = dag(json!({
        "id": "r",
        "nodes": [
            {"id": "q", "type": "db_query", "connection": "ro", "sql": "select 1"},
            {"id": "s", "type": "subworkflow", "dag": {"id": "child"}}
        ]
    }));
    let outcome = run(router(None), &server, &root).await;
    for id in ["q", "s"] {
        let node = &outcome.nodes[id];
        assert_eq!(node.status, NodeStatus::Failed);
        let err = node.error.as_ref().unwrap();
        assert_eq!(err.kind, ExecErrorKind::Unsupported);
        assert!(err.message.contains(&format!("node '{id}'")), "{err:?}");
    }
}
