#![allow(clippy::unwrap_used)]

//! End to end: a real failure goes through the executors, the scheduler and the recorder, and
//! lands in the store classified.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{FixedClock, ManualSleeper, SeqIdGen, Timestamp};
use engine::{
    AssertExecutor, CleanupExecutor, Environment, ExecContext, ExecFuture, HttpExecutor,
    NodeExecutor, NodeStatus, PollExecutor, RuleClassifier, RunRecorder, Scheduler, StoreRecorder,
};
use model::{Dag, Node, NodeKind};
use serde_json::{json, Value};
use store::{
    DagRepository, Db, ExecutionKey, FailureRecordRepository, NewRun, NodeExecutionRepository,
    RunMode,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Router {
    http: Arc<HttpExecutor>,
    poll: PollExecutor,
    cleanup: CleanupExecutor,
}

impl NodeExecutor for Router {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        match node.kind {
            NodeKind::Http(_) => self.http.execute(node, ctx),
            NodeKind::Poll(_) => self.poll.execute(node, ctx),
            NodeKind::Cleanup(_) => self.cleanup.execute(node, ctx),
            _ => AssertExecutor.execute(node, ctx),
        }
    }
}

struct Classified {
    category: String,
    code: String,
    raw_message: String,
}

/// Runs `nodes` against `server`, then returns the classification of the failed node `failed`
/// from `node_executions`, checking that `failure_records` agrees.
async fn classify(server: &MockServer, nodes: Value, edges: Value, failed: &str) -> Classified {
    let dag = Dag::from_json_str(&json!({"id": "e2e", "nodes": nodes, "edges": edges}).to_string())
        .unwrap();
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_794_117_600_000));
    let sleeper = ManualSleeper::new(clock.clone());
    let clock = Arc::new(clock);
    let db = Db::open_in_memory(clock.as_ref()).unwrap();
    DagRepository::new(db.conn(), clock.as_ref())
        .save(&dag, "test")
        .unwrap();
    let db = Arc::new(Mutex::new(db));
    let ids = Arc::new(SeqIdGen::new("id"));
    let recorder = Arc::new(StoreRecorder::new(
        Arc::clone(&db),
        clock.clone(),
        ids.clone(),
        Arc::new(RuleClassifier::new().unwrap()),
    ));
    let http = Arc::new(
        HttpExecutor::new(clock.clone())
            .unwrap()
            .with_default_timeout(Duration::from_secs(5)),
    );
    let router = Router {
        http: Arc::clone(&http),
        poll: PollExecutor::new(Arc::clone(&http), clock.clone(), Arc::new(sleeper)),
        cleanup: CleanupExecutor::new(http),
    };
    let run = recorder
        .begin_run(&NewRun {
            dag_id: "e2e".into(),
            dag_version: 1,
            env_name: "local".into(),
            mode: RunMode::Ci,
        })
        .unwrap();
    let env = Environment::new("local", &server.uri(), BTreeMap::new()).unwrap();
    let sink: Arc<dyn RunRecorder> = recorder.clone();
    let outcome = Scheduler::new(Arc::new(router))
        .with_environment(env)
        .with_recorder(Some(sink))
        .run(&run.id, &dag)
        .await
        .unwrap();
    recorder.finish_run(&run.id, &outcome).unwrap();
    assert_eq!(
        outcome.nodes[failed].status,
        NodeStatus::Failed,
        "{outcome:#?}"
    );

    let db = db.lock().unwrap();
    let row = NodeExecutionRepository::new(db.conn(), clock.as_ref())
        .get(&ExecutionKey::new(&run.id, failed, 1))
        .unwrap()
        .unwrap();
    let failure = row.failure.unwrap();
    let records = FailureRecordRepository::new(db.conn(), clock.as_ref(), ids.as_ref())
        .list_for_run(&run.id)
        .unwrap();
    assert_eq!(records.len(), 1, "{records:#?}");
    let record = &records[0];
    assert_eq!(record.node_id.as_deref(), Some(failed));
    assert_eq!(record.failure_category, failure.category);
    assert_eq!(
        record.error_code.as_deref(),
        Some(failure.error_code.as_str())
    );
    Classified {
        category: failure.category,
        code: failure.error_code,
        raw_message: record.raw_message.clone(),
    }
}

fn http(id: &str, path: &str) -> Value {
    json!({"id": id, "type": "http", "method": "GET", "path": path})
}

fn assert_on(target: &str, path: &str, expected: Value) -> Value {
    json!({"id": "check", "type": "assert", "target": target,
           "assertions": [{"path": path, "op": "eq", "expected": expected}]})
}

fn then_check() -> Value {
    json!([{"from": "call", "to": "check"}])
}

async fn respond(server: &MockServer, route: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn connection_refused_is_a_connection_error() {
    let server = MockServer::start().await;
    let got = classify(
        &server,
        json!([http("call", "http://127.0.0.1:1/orders")]),
        json!([]),
        "call",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("connection_error", "CONN_REFUSED")
    );
    assert!(
        got.raw_message.contains("127.0.0.1:1"),
        "{}",
        got.raw_message
    );
}

#[tokio::test]
async fn a_slow_response_is_a_request_timeout() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/slow",
        ResponseTemplate::new(200).set_delay(Duration::from_millis(500)),
    )
    .await;
    let mut call = http("call", "/slow");
    call["timeout_ms"] = json!(50);
    let got = classify(&server, json!([call]), json!([]), "call").await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("timeout", "TIMEOUT_REQUEST")
    );
}

#[tokio::test]
async fn a_poll_that_never_succeeds_is_a_poll_timeout() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/orders/1",
        ResponseTemplate::new(200).set_body_json(json!({"status": "PENDING"})),
    )
    .await;
    let poll = json!({"id": "wait", "type": "poll",
        "request": {"method": "GET", "path": "/orders/1"},
        "condition": {"path": "$.body.status", "op": "eq", "expected": "PAID"},
        "backoff_ms": [100], "timeout_ms": 250});
    let got = classify(&server, json!([poll]), json!([]), "wait").await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("timeout", "TIMEOUT_POLL")
    );
}

#[tokio::test]
async fn an_unexpected_404_is_a_status_assertion_failure() {
    let server = MockServer::start().await;
    respond(&server, "/orders/1", ResponseTemplate::new(404)).await;
    let got = classify(
        &server,
        json!([
            http("call", "/orders/1"),
            assert_on("call", "$.status", json!(200))
        ]),
        then_check(),
        "check",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("assertion_failed_status", "ASSERT_STATUS_CLIENT_ERROR")
    );
}

#[tokio::test]
async fn a_201_instead_of_200_is_a_status_mismatch() {
    let server = MockServer::start().await;
    respond(&server, "/orders", ResponseTemplate::new(201)).await;
    let got = classify(
        &server,
        json!([
            http("call", "/orders"),
            assert_on("call", "$.status", json!(200))
        ]),
        then_check(),
        "check",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("assertion_failed_status", "ASSERT_STATUS_MISMATCH")
    );
}

#[tokio::test]
async fn a_wrong_field_value_is_a_body_shape_failure() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/orders/1",
        ResponseTemplate::new(200).set_body_json(json!({"state": "NEW"})),
    )
    .await;
    let got = classify(
        &server,
        json!([
            http("call", "/orders/1"),
            assert_on("call", "$.body.state", json!("PAID"))
        ]),
        then_check(),
        "check",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("assertion_failed_body_shape", "ASSERT_BODY_VALUE")
    );
}

#[tokio::test]
async fn a_body_check_on_a_500_is_blamed_on_the_status() {
    let server = MockServer::start().await;
    respond(&server, "/orders/1", ResponseTemplate::new(500)).await;
    let got = classify(
        &server,
        json!([
            http("call", "/orders/1"),
            assert_on("call", "$.body.state", json!("PAID"))
        ]),
        then_check(),
        "check",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("assertion_failed_status", "ASSERT_STATUS_SERVER_ERROR")
    );
}

#[tokio::test]
async fn a_missing_upstream_field_is_a_capture_error() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/orders",
        ResponseTemplate::new(200).set_body_json(json!({"order": {"uid": "o-1"}})),
    )
    .await;
    let got = classify(
        &server,
        json!([
            http("create", "/orders"),
            http("read", "/orders/${create.output.body.order.id}")
        ]),
        json!([{"from": "create", "to": "read"}]),
        "read",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("capture_error", "CAPTURE_MISSING_PATH")
    );
}

#[tokio::test]
async fn an_illegal_transition_is_a_business_rule_violation() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/orders/1/ship",
        ResponseTemplate::new(409)
            .set_body_json(json!({"error": "illegal state transition NEW -> SHIPPED"})),
    )
    .await;
    let got = classify(
        &server,
        json!([
            http("call", "/orders/1/ship"),
            assert_on("call", "$.status", json!(200))
        ]),
        then_check(),
        "check",
    )
    .await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("business_rule_violation", "BIZ_STATE_TRANSITION")
    );
}

#[tokio::test]
async fn a_rejected_cleanup_request_is_classified_by_its_status() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/orders/1"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let cleanup = json!({"id": "tidy", "type": "cleanup",
        "requests": [{"method": "DELETE", "path": "/orders/1"}]});
    let got = classify(&server, json!([cleanup]), json!([]), "tidy").await;
    assert_eq!(
        (got.category.as_str(), got.code.as_str()),
        ("assertion_failed_status", "ASSERT_STATUS_CLIENT_ERROR")
    );
}
