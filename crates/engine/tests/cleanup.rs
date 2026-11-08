#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{dag_with_cleanup, keys, MockExecutor};
use engine::NodeStatus::{Completed, Failed, Skipped};
use engine::{
    CleanupExecutor, Environment, ExecContext, ExecErrorKind, HttpExecutor, NodeExecutor,
    RunOutcome, Scheduler,
};
use model::{Dag, Node};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn run(exec: &Arc<MockExecutor>, dag: &Dag, parallel: usize) -> RunOutcome {
    Scheduler::new(exec.clone())
        .with_max_parallel(parallel)
        .run("run-1", dag)
        .await
        .unwrap()
}

#[tokio::test]
async fn cleanup_runs_last_after_upstream_success() {
    // teardown only depends on a, yet waits for b and c.
    let d = dag_with_cleanup(
        &["a", "b", "c"],
        &["teardown"],
        &[("a", "b"), ("b", "c"), ("a", "teardown")],
    );
    for parallel in [1, 4] {
        let exec = Arc::new(MockExecutor::default());
        let outcome = run(&exec, &d, parallel).await;
        assert_eq!(exec.order(), ["a", "b", "c", "teardown"]);
        assert!(outcome.succeeded());
        assert_eq!(keys(&exec.context_of("teardown").outputs), ["a"]);
    }
}

#[tokio::test]
async fn cleanup_runs_after_upstream_failure() {
    let d = dag_with_cleanup(&["a", "b"], &["teardown"], &[("a", "b"), ("a", "teardown")]);
    let exec = Arc::new(MockExecutor::failing(&["a"]));
    let outcome = run(&exec, &d, 1).await;
    assert_eq!(outcome.nodes["a"].status, Failed);
    assert_eq!(outcome.nodes["b"].status, Skipped);
    assert_eq!(outcome.nodes["teardown"].status, Completed);
    assert_eq!(exec.order(), ["a", "teardown"]);
    assert!(exec.context_of("teardown").outputs.is_empty());
    assert!(!outcome.succeeded());
}

#[tokio::test]
async fn cleanup_runs_after_a_skipped_predecessor() {
    let d = dag_with_cleanup(&["a", "b"], &["teardown"], &[("a", "b"), ("b", "teardown")]);
    let exec = Arc::new(MockExecutor::failing(&["a"]));
    let outcome = run(&exec, &d, 1).await;
    assert_eq!(outcome.nodes["b"].status, Skipped);
    assert_eq!(outcome.nodes["teardown"].status, Completed);
}

#[tokio::test]
async fn cleanups_run_in_level_order_and_a_failing_one_does_not_stop_the_others() {
    let d = dag_with_cleanup(
        &["a"],
        &["z_first", "b_second", "c_after"],
        &[("a", "z_first"), ("a", "b_second"), ("z_first", "c_after")],
    );
    for parallel in [1, 4] {
        let exec = Arc::new(MockExecutor::failing(&["z_first"]));
        let outcome = run(&exec, &d, parallel).await;
        assert_eq!(exec.order(), ["a", "b_second", "z_first", "c_after"]);
        assert_eq!(outcome.nodes["z_first"].status, Failed);
        assert_eq!(outcome.nodes["c_after"].status, Completed);
        assert_eq!(outcome.nodes["b_second"].status, Completed);
    }
}

#[tokio::test]
async fn reports_with_cleanup_do_not_depend_on_parallelism() {
    let d = dag_with_cleanup(
        &["a", "b", "c"],
        &["t1", "t2"],
        &[("a", "b"), ("a", "c"), ("b", "t1"), ("c", "t2")],
    );
    let one = run(&Arc::new(MockExecutor::failing(&["b"])), &d, 1).await;
    let four = run(&Arc::new(MockExecutor::failing(&["b"])), &d, 4).await;
    assert_eq!(one.report(), four.report());
}

fn cleanup_node(requests: serde_json::Value) -> Node {
    Dag::from_json_str(
        &json!({"id": "d", "nodes": [{"id": "teardown", "type": "cleanup", "requests": requests}]})
            .to_string(),
    )
    .unwrap()
    .nodes
    .remove(0)
}

fn ctx(base_url: &str) -> ExecContext {
    ExecContext {
        run_id: "run-1".into(),
        outputs: BTreeMap::from([("create".to_owned(), json!({"body": {"id": "o-1"}}))]),
        env: Arc::new(Environment::new("local", base_url, BTreeMap::new()).unwrap()),
        ..ExecContext::default()
    }
}

fn executor() -> CleanupExecutor {
    let clock = Arc::new(common_clock());
    CleanupExecutor::new(Arc::new(HttpExecutor::new(clock).unwrap()))
}

fn common_clock() -> ::common::FixedClock {
    ::common::FixedClock::new(::common::Timestamp::from_unix_millis(0))
}

#[tokio::test]
async fn cleanup_executor_runs_every_request_in_order() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/orders/o-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/carts/o-1"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let done = executor()
        .execute(
            &cleanup_node(json!([
                {"method": "DELETE", "path": "/orders/${create.output.body.id}"},
                {"method": "DELETE", "path": "/carts/${create.output.body.id}"}
            ])),
            ctx(&server.uri()),
        )
        .await
        .unwrap();
    assert_eq!(
        done.output,
        json!({"results": [{"index": 0, "status": 204}, {"index": 1, "status": 404}]})
    );
}

#[tokio::test]
async fn cleanup_executor_continues_past_failures() {
    let server = MockServer::start().await;
    Mock::given(path("/users/1"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(path("/orders/o-1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let err = executor()
        .execute(
            &cleanup_node(json!([
                {"method": "DELETE", "path": "/gone/${missing.output.body.id}"},
                {"method": "DELETE", "path": "/users/1"},
                {"method": "DELETE", "path": "/orders/${create.output.body.id}"}
            ])),
            ctx(&server.uri()),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert_eq!(err.message, "2 of 3 cleanup requests failed");
    let results = err.detail.unwrap()["results"].clone();
    assert_eq!(results[0]["error"]["kind"], json!("template"));
    assert!(results[0]["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("/requests/0/path"));
    assert_eq!(results[1]["status"], json!(500));
    assert_eq!(results[1]["error"]["kind"], json!("assertion"));
    assert_eq!(results[2], json!({"index": 2, "status": 200}));
}

#[tokio::test]
async fn empty_cleanup_completes() {
    let done = executor()
        .execute(&cleanup_node(json!([])), ctx("http://unused.invalid"))
        .await
        .unwrap();
    assert_eq!(done.output, json!({"results": []}));
}
