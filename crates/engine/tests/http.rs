#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use common::{FixedClock, Timestamp};
use engine::{Environment, ExecContext, ExecError, ExecErrorKind, HttpExecutor, NodeExecutor};
use model::{Dag, Node};
use serde_json::{json, Value};
use wiremock::matchers::{body_json, body_string, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn executor() -> HttpExecutor {
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_794_117_600_000));
    HttpExecutor::new(Arc::new(clock)).unwrap()
}

fn ctx(base_url: &str) -> ExecContext {
    ExecContext {
        run_id: "run-1".into(),
        outputs: BTreeMap::from([(
            "login".to_owned(),
            json!({"body": {"token": "t-1", "user": {"id": 7}}}),
        )]),
        env: Arc::new(
            Environment::new(
                "local",
                base_url,
                BTreeMap::from([("TENANT".to_owned(), "acme".to_owned())]),
            )
            .unwrap(),
        ),
        ..ExecContext::default()
    }
}

fn node(value: Value) -> Node {
    let mut value = value;
    value["id"] = json!("n");
    value["type"] = json!("http");
    Dag::from_json_str(&json!({"id": "d", "nodes": [value]}).to_string())
        .unwrap()
        .nodes
        .remove(0)
}

async fn run(server_uri: &str, config: Value) -> Result<Value, ExecError> {
    executor()
        .execute(&node(config), ctx(server_uri))
        .await
        .map(|done| done.output)
}

#[tokio::test]
async fn get_renders_path_query_and_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/7"))
        .and(query_param("tenant", "acme"))
        .and(query_param("limit", "10"))
        .and(header("authorization", "Bearer t-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Request-Id", "r-1")
                .set_body_json(json!({"id": 7, "name": "ann"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = run(
        &format!("{}/", server.uri()),
        json!({
            "method": "GET",
            "path": "/users/${login.output.body.user.id}",
            "query": {"tenant": "${env:TENANT}", "limit": "10"},
            "headers": {"Authorization": "Bearer ${login.output.body.token}"}
        }),
    )
    .await
    .unwrap();

    assert_eq!(output["status"], json!(200));
    assert_eq!(output["body"], json!({"id": 7, "name": "ann"}));
    assert_eq!(output["headers"]["x-request-id"], json!("r-1"));
    assert_eq!(output["headers"]["content-type"], json!("application/json"));
    assert_eq!(output["timing"], json!({"duration_ms": 0}));
    assert_eq!(
        output.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["body", "headers", "status", "timing"]
    );
}

#[tokio::test]
async fn post_sends_rendered_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/orders"))
        .and(header("content-type", "application/json"))
        .and(body_json(
            json!({"user": 7, "tenant": "acme", "items": [{"sku": "A", "qty": 2}]}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"order_id": "o-1"})))
        .expect(1)
        .mount(&server)
        .await;

    let output = run(
        &server.uri(),
        json!({
            "method": "POST",
            "path": "orders",
            "body": {
                "user": "${login.output.body.user.id}",
                "tenant": "${env:TENANT}",
                "items": [{"sku": "A", "qty": 2}]
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(output["status"], json!(201));
    assert_eq!(output["body"]["order_id"], json!("o-1"));
}

#[tokio::test]
async fn string_body_with_explicit_content_type_is_sent_raw() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/note"))
        .and(header("content-type", "text/plain"))
        .and(body_string("hello acme"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = run(
        &server.uri(),
        json!({
            "method": "PUT", "path": "/note",
            "headers": {"Content-Type": "text/plain"},
            "body": "hello ${env:TENANT}"
        }),
    )
    .await
    .unwrap();
    assert_eq!(output["status"], json!(204));
    assert_eq!(output["body"], Value::Null);
}

#[tokio::test]
async fn error_statuses_and_non_json_bodies_complete_normally() {
    let server = MockServer::start().await;
    Mock::given(path("/boom"))
        .respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "application/json")
                .set_body_string("<html>oops</html>"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/text"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not here"))
        .mount(&server)
        .await;
    Mock::given(path("/moved"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/elsewhere"))
        .mount(&server)
        .await;

    let boom = run(&server.uri(), json!({"method": "GET", "path": "/boom"}))
        .await
        .unwrap();
    assert_eq!(boom["status"], json!(500));
    assert_eq!(boom["body"], json!("<html>oops</html>"));

    let text = run(&server.uri(), json!({"method": "GET", "path": "/text"}))
        .await
        .unwrap();
    assert_eq!(text["status"], json!(404));
    assert_eq!(text["body"], json!("not here"));

    let moved = run(&server.uri(), json!({"method": "GET", "path": "/moved"}))
        .await
        .unwrap();
    assert_eq!(moved["status"], json!(302));
    assert_eq!(moved["headers"]["location"], json!("/elsewhere"));
}

#[tokio::test]
async fn slow_response_times_out() {
    let server = MockServer::start().await;
    Mock::given(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;

    let err = run(
        &server.uri(),
        json!({"method": "GET", "path": "/slow?x=secret", "timeout_ms": 200}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Timeout);
    assert_eq!(err.message, "request timed out after 200 ms");
    assert_eq!(
        err.detail,
        Some(json!({"method": "GET", "url": format!("{}/slow", server.uri()), "timeout_ms": 200}))
    );
}

#[tokio::test]
async fn default_timeout_applies_without_timeout_ms() {
    let server = MockServer::start().await;
    Mock::given(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let err = executor()
        .with_default_timeout(Duration::from_millis(150))
        .execute(
            &node(json!({"method": "GET", "path": "/slow"})),
            ctx(&server.uri()),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Timeout);
}

#[tokio::test]
async fn refused_connection_is_a_transport_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let err = run(
        &format!("http://127.0.0.1:{port}"),
        json!({"method": "GET", "path": "/"}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Transport);
    assert_eq!(err.detail.unwrap()["connect"], json!(true));
}

#[tokio::test]
async fn absolute_urls_ignore_base_url() {
    let server = MockServer::start().await;
    Mock::given(path("/abs"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let output = run(
        "http://unused.invalid",
        json!({"method": "GET", "path": format!("{}/abs", server.uri())}),
    )
    .await
    .unwrap();
    assert_eq!(output["status"], json!(200));
}

#[tokio::test]
async fn configuration_problems_fail_before_sending() {
    let err = run("", json!({"method": "GET", "path": "/x"}))
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    assert!(err.message.contains("has no base_url"), "{}", err.message);

    let err = run(
        "http://unused.invalid",
        json!({"method": "GET", "path": "/", "headers": {"X-Id": "${login.output.body.nope}"}}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Template);
    let detail = err.detail.unwrap();
    assert_eq!(detail["pointer"], json!("/headers/X-Id"));
    assert_eq!(detail["reason"], json!("missing_path"));

    let err = run(
        "http://unused.invalid",
        json!({"method": "POST", "path": "/", "body": {"a": ["${env:NOPE}"]}}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.detail.unwrap()["pointer"], json!("/body/a/0"));
}

#[tokio::test]
async fn other_node_types_are_unsupported() {
    let assert_node = Dag::from_json_str(
        &json!({"id": "d", "nodes": [
            {"id": "x", "type": "assert", "target": "a",
             "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]}
        ]})
        .to_string(),
    )
    .unwrap()
    .nodes
    .remove(0);
    let err = executor()
        .execute(&assert_node, ctx("http://unused.invalid"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Unsupported);
}
