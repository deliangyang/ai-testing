#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use common::{FixedClock, ManualSleeper, Timestamp};
use engine::{
    Environment, ExecContext, ExecError, ExecErrorKind, ExecOutput, HttpExecutor, NodeExecutor,
    PollExecutor,
};
use model::{Dag, Node};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const START: i64 = 1_794_117_600_000;

struct Harness {
    sleeper: ManualSleeper,
    poll: PollExecutor,
}

fn harness() -> Harness {
    let clock = FixedClock::new(Timestamp::from_unix_millis(START));
    let sleeper = ManualSleeper::new(clock.clone());
    let clock = Arc::new(clock);
    let http = Arc::new(HttpExecutor::new(clock.clone()).unwrap());
    Harness {
        poll: PollExecutor::new(http, clock, Arc::new(sleeper.clone())),
        sleeper,
    }
}

impl Harness {
    async fn run(&self, base_url: &str, config: Value) -> Result<ExecOutput, ExecError> {
        let mut config = config;
        config["id"] = json!("wait");
        config["type"] = json!("poll");
        let node: Node = Dag::from_json_str(&json!({"id": "d", "nodes": [config]}).to_string())
            .unwrap()
            .nodes
            .remove(0);
        let ctx = ExecContext {
            run_id: "run-1".into(),
            outputs: BTreeMap::from([("create".to_owned(), json!({"body": {"id": "o-1"}}))]),
            env: Arc::new(Environment::new("local", base_url, BTreeMap::new()).unwrap()),
        };
        self.poll.execute(&node, ctx).await
    }

    fn sleeps(&self) -> Vec<u64> {
        self.sleeper
            .requests()
            .iter()
            .map(|d| u64::try_from(d.as_millis()).unwrap())
            .collect()
    }
}

fn ts(offset_ms: i64) -> String {
    Timestamp::from_unix_millis(START + offset_ms).to_string()
}

fn status_body(status: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"id": "o-1", "status": status}))
}

/// Serves `pending` PENDING responses, then PAID.
async fn server(pending: u64) -> MockServer {
    let server = MockServer::start().await;
    if pending > 0 {
        Mock::given(method("GET"))
            .and(path("/orders/o-1"))
            .respond_with(status_body("PENDING"))
            .up_to_n_times(pending)
            .with_priority(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/orders/o-1"))
        .respond_with(status_body("PAID"))
        .with_priority(2)
        .mount(&server)
        .await;
    server
}

fn config(backoff: &[u64], timeout_ms: u64) -> Value {
    json!({
        "request": {"method": "GET", "path": "/orders/${create.output.body.id}"},
        "condition": {"path": "$.body.status", "op": "eq", "expected": "PAID"},
        "backoff_ms": backoff,
        "timeout_ms": timeout_ms
    })
}

#[tokio::test]
async fn succeeds_once_the_condition_holds_with_an_exact_trace() {
    let server = server(2).await;
    let h = harness();
    let done = h
        .run(&server.uri(), config(&[100, 200], 10_000))
        .await
        .unwrap();

    assert_eq!(done.output["status"], json!(200));
    assert_eq!(done.output["body"]["status"], json!("PAID"));
    assert_eq!(h.sleeps(), [100, 200]);
    assert_eq!(
        done.trace.unwrap(),
        json!([
            {"ts": ts(0), "attempt": 1, "passed": false, "actual": "PENDING", "expected": "PAID",
             "diff": "\"PENDING\" eq \"PAID\" is false"},
            {"ts": ts(100), "attempt": 2, "passed": false, "actual": "PENDING", "expected": "PAID",
             "diff": "\"PENDING\" eq \"PAID\" is false"},
            {"ts": ts(300), "attempt": 3, "passed": true, "actual": "PAID", "expected": "PAID"}
        ])
    );
}

#[tokio::test]
async fn immediate_success_never_sleeps() {
    let server = server(0).await;
    let h = harness();
    let done = h.run(&server.uri(), config(&[100], 1_000)).await.unwrap();
    assert!(h.sleeps().is_empty());
    assert_eq!(done.trace.unwrap().as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn timeout_keeps_the_full_trace_and_repeats_the_last_interval() {
    let server = server(u64::MAX).await;
    let h = harness();
    // Attempts at 0, 100, 400, 700, 1000; the next would start at 1300 > 1000.
    let err = h
        .run(&server.uri(), config(&[100, 300], 1_000))
        .await
        .unwrap_err();

    assert_eq!(err.kind, ExecErrorKind::Timeout);
    assert_eq!(
        err.message,
        "condition $.body.status eq \"PAID\" not met within 1000 ms after 5 attempts"
    );
    assert_eq!(h.sleeps(), [100, 300, 300, 300]);
    let trace = err.trace.clone().unwrap();
    let stamps: Vec<&str> = trace
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["ts"].as_str().unwrap())
        .collect();
    assert_eq!(stamps, [ts(0), ts(100), ts(400), ts(700), ts(1000)]);
    assert!(trace
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["actual"] == json!("PENDING")));
    let detail = err.detail.unwrap();
    assert_eq!(detail["reason"], json!("timeout"));
    assert_eq!(detail["attempts"], json!(5));
    assert_eq!(detail["last"]["attempt"], json!(5));
}

#[tokio::test]
async fn max_attempts_stops_early() {
    let server = server(u64::MAX).await;
    let h = harness();
    let mut cfg = config(&[100], 60_000);
    cfg["max_attempts"] = json!(2);
    let err = h.run(&server.uri(), cfg).await.unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Timeout);
    assert_eq!(err.detail.unwrap()["reason"], json!("max_attempts"));
    assert_eq!(h.sleeps(), [100]);
    assert_eq!(err.trace.unwrap().as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn transport_failures_count_as_unsatisfied_attempts() {
    let server = MockServer::start().await;
    Mock::given(path("/orders/o-1"))
        .respond_with(status_body("PENDING").set_delay(Duration::from_secs(5)))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/orders/o-1"))
        .respond_with(status_body("PAID"))
        .with_priority(2)
        .mount(&server)
        .await;

    let h = harness();
    let mut cfg = config(&[50], 10_000);
    cfg["request"]["timeout_ms"] = json!(100);
    let done = h.run(&server.uri(), cfg).await.unwrap();
    let trace = done.trace.unwrap();
    assert_eq!(trace[0]["passed"], json!(false));
    assert_eq!(trace[0]["error"]["kind"], json!("timeout"));
    assert!(trace[0].get("actual").is_none());
    assert_eq!(trace[1]["passed"], json!(true));
    assert_eq!(h.sleeps(), [50]);
}

#[tokio::test]
async fn scheduler_keeps_the_trace_of_a_failed_poll() {
    let server = server(u64::MAX).await;
    let h = harness();
    let mut cfg = config(&[100], 250);
    cfg["id"] = json!("wait");
    cfg["type"] = json!("poll");
    cfg["request"]["path"] = json!("/orders/o-1");
    let dag = Dag::from_json_str(&json!({"id": "d", "nodes": [cfg]}).to_string()).unwrap();
    let outcome = engine::Scheduler::new(Arc::new(h.poll))
        .with_environment(Environment::new("local", &server.uri(), BTreeMap::new()).unwrap())
        .run("run-1", &dag)
        .await
        .unwrap();
    let result = &outcome.nodes["wait"];
    assert_eq!(result.status, engine::NodeStatus::Failed);
    assert_eq!(result.trace.as_ref().unwrap().as_array().unwrap().len(), 3);
    assert_eq!(result.error.as_ref().unwrap().trace, None);
}

#[tokio::test]
async fn template_errors_fail_without_polling() {
    let server = MockServer::start().await;
    Mock::given(path("/orders/o-1"))
        .respond_with(status_body("PAID"))
        .expect(0)
        .mount(&server)
        .await;
    let h = harness();

    let mut cfg = config(&[100], 1_000);
    cfg["condition"]["expected"] = json!("${create.output.body.nope}");
    let err = h.run(&server.uri(), cfg).await.unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(err.detail.unwrap()["pointer"], json!("/condition/expected"));

    let mut cfg = config(&[100], 1_000);
    cfg["request"]["path"] = json!("/orders/${env:NOPE}");
    let err = h.run(&server.uri(), cfg).await.unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(err.detail.unwrap()["pointer"], json!("/request/path"));
    assert!(h.sleeps().is_empty());
}
