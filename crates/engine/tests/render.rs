#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use engine::NodeStatus::{Completed, Failed, Skipped};
use engine::{
    Environment, ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor, RenderError,
    RenderFailure, Scheduler, VarContext,
};
use model::{Dag, Node};
use serde_json::{json, Value};

fn env() -> Environment {
    Environment::new(
        "local",
        "http://sut",
        BTreeMap::from([("TENANT".to_owned(), "acme".to_owned())]),
    )
    .unwrap()
}

fn outputs() -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "login".to_owned(),
            json!({"status": 200, "body": {"token": "t-1", "ok": true, "n": null}}),
        ),
        (
            "create".to_owned(),
            json!({"body": {"id": 42, "items": [{"sku": "A"}, {"sku": "B"}], "a/b": "slash"}}),
        ),
    ])
}

fn render(input: Value) -> Result<Value, RenderError> {
    let (env, outputs) = (env(), outputs());
    VarContext {
        env: &env,
        outputs: &outputs,
    }
    .render_value(&input)
}

#[test]
fn single_references_keep_their_json_type() {
    let cases = [
        ("${create.output.body.id}", json!(42)),
        ("${login.output.body.ok}", json!(true)),
        ("${login.output.body.n}", json!(null)),
        ("${create.output.body.items[1]}", json!({"sku": "B"})),
        (
            "${create.output.body.items}",
            json!([{"sku": "A"}, {"sku": "B"}]),
        ),
        ("${create.output.body[\"a/b\"]}", json!("slash")),
        ("${env:TENANT}", json!("acme")),
        ("${env:BASE_URL}", json!("http://sut")),
        ("${login.output}", outputs()["login"].clone()),
    ];
    for (template, expected) in cases {
        assert_eq!(render(json!(template)).unwrap(), expected, "{template}");
    }
}

#[test]
fn mixed_templates_are_stringified() {
    let cases = [
        ("/orders/${create.output.body.id}", "/orders/42"),
        ("Bearer ${login.output.body.token}", "Bearer t-1"),
        ("${env:BASE_URL}/t/${env:TENANT}", "http://sut/t/acme"),
        ("x=${create.output.body.items[0]}", "x={\"sku\":\"A\"}"),
        (
            "ok=${login.output.body.ok};n=${login.output.body.n}",
            "ok=true;n=null",
        ),
        ("$${literal} ${env:TENANT}", "${literal} acme"),
        ("no refs", "no refs"),
    ];
    for (template, expected) in cases {
        assert_eq!(
            render(json!(template)).unwrap(),
            json!(expected),
            "{template}"
        );
    }
}

#[test]
fn nested_values_are_rendered_and_non_strings_kept() {
    let rendered = render(json!({
        "a": [1, "${create.output.body.id}", {"b": "${env:TENANT}"}],
        "c": false
    }))
    .unwrap();
    assert_eq!(rendered, json!({"a": [1, 42, {"b": "acme"}], "c": false}));
}

#[test]
fn failures_carry_pointer_reference_and_reason() {
    let cases = [
        (
            json!({"h": {"X": "${ghost.output.body}"}}),
            "/h/X",
            Some("${ghost.output.body}"),
            RenderFailure::MissingNode {
                node: "ghost".into(),
            },
        ),
        (
            json!({"p": ["ok", "/o/${create.output.body.user.id}"]}),
            "/p/1",
            Some("${create.output.body.user.id}"),
            RenderFailure::MissingPath {
                node: "create".into(),
                found: "${create.output.body}".into(),
            },
        ),
        (
            json!({"a/b": "${create.output.body.items[5].sku}"}),
            "/a~1b",
            Some("${create.output.body.items[5].sku}"),
            RenderFailure::MissingPath {
                node: "create".into(),
                found: "${create.output.body.items}".into(),
            },
        ),
        (
            json!("${create.output.body.id.x}"),
            "",
            Some("${create.output.body.id.x}"),
            RenderFailure::MissingPath {
                node: "create".into(),
                found: "${create.output.body.id}".into(),
            },
        ),
        (
            json!({"q": "${env:REGION}"}),
            "/q",
            Some("${env:REGION}"),
            RenderFailure::MissingEnv {
                name: "REGION".into(),
            },
        ),
        (
            json!({"h": "Bearer ${auth.token}"}),
            "/h",
            Some("${auth.token}"),
            RenderFailure::AuthUnavailable {
                name: "token".into(),
            },
        ),
        (
            json!({"bad": "x ${oops"}),
            "/bad",
            None,
            RenderFailure::InvalidTemplate {
                offset: 2,
                message: "unterminated '${', missing '}'".into(),
            },
        ),
    ];
    for (input, pointer, reference, failure) in cases {
        let err = render(input.clone()).unwrap_err();
        assert_eq!(
            err,
            RenderError {
                pointer: pointer.into(),
                reference: reference.map(str::to_owned),
                failure,
            },
            "{input}"
        );
    }
}

#[test]
fn errors_become_template_exec_errors_with_serializable_detail() {
    let err = render(json!({"path": "/o/${create.output.body.nope}"})).unwrap_err();
    assert_eq!(
        err.to_string(),
        "/path: ${create.output.body.nope}: path not found in output (resolved up to ${create.output.body})"
    );
    let exec: ExecError = err.into();
    assert_eq!(exec.kind, ExecErrorKind::Template);
    assert_eq!(
        exec.detail,
        Some(json!({
            "pointer": "/path",
            "reference": "${create.output.body.nope}",
            "reason": "missing_path",
            "node": "create",
            "found": "${create.output.body}"
        }))
    );
}

fn node(value: Value) -> Node {
    Dag::from_json_str(&json!({"id": "d", "nodes": [value]}).to_string())
        .unwrap()
        .nodes
        .remove(0)
}

#[test]
fn resolve_node_renders_only_template_positions() {
    let (env, outputs) = (env(), outputs());
    let ctx = VarContext {
        env: &env,
        outputs: &outputs,
    };
    let resolved = ctx
        .resolve_node(&node(json!({
            "id": "pay", "type": "http", "method": "POST",
            "path": "/orders/${create.output.body.id}/pay",
            "headers": {"Authorization": "Bearer ${login.output.body.token}"},
            "body": {"amount": "${create.output.body.id}", "tenant": "${env:TENANT}"},
            "response_schema": {"description": "${not.output.rendered}"}
        })))
        .unwrap();
    assert_eq!(
        resolved,
        json!({
            "id": "pay", "type": "http", "method": "POST",
            "path": "/orders/42/pay",
            "headers": {"Authorization": "Bearer t-1"},
            "body": {"amount": 42, "tenant": "acme"},
            "response_schema": {"description": "${not.output.rendered}"}
        })
    );
    let text = model::to_canonical_json(&resolved).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), resolved);

    let assert_node = ctx
        .resolve_node(&node(json!({
            "id": "check", "type": "assert", "target": "create",
            "assertions": [{"path": "$.body.id", "op": "eq", "expected": "${create.output.body.id}"}]
        })))
        .unwrap();
    assert_eq!(assert_node["assertions"][0]["expected"], json!(42));
    assert_eq!(assert_node["assertions"][0]["path"], json!("$.body.id"));
}

/// Returns the node's resolved input as its output.
struct Resolving;

impl NodeExecutor for Resolving {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move { Ok(ctx.vars().resolve_node(node)?) })
    }
}

#[tokio::test]
async fn scheduler_passes_environment_and_template_failures_fail_the_node() {
    let dag = Dag::from_json_str(
        &json!({
            "id": "d",
            "nodes": [
                {"id": "a", "type": "http", "method": "GET", "path": "/t/${env:TENANT}"},
                {"id": "b", "type": "http", "method": "GET", "path": "${a.output.path}/x"},
                {"id": "c", "type": "http", "method": "GET", "path": "/${a.output.body.missing}"},
                {"id": "d", "type": "http", "method": "GET", "path": "/d"}
            ],
            "edges": [{"from": "a", "to": "b"}, {"from": "a", "to": "c"}, {"from": "c", "to": "d"}]
        })
        .to_string(),
    )
    .unwrap();
    let outcome = Scheduler::new(Arc::new(Resolving))
        .with_environment(env())
        .run("run-1", &dag)
        .await
        .unwrap();

    assert_eq!(outcome.nodes["a"].status, Completed);
    assert_eq!(outcome.nodes["b"].status, Completed);
    assert_eq!(
        outcome.nodes["b"].output.as_ref().unwrap()["path"],
        json!("/t/acme/x")
    );
    assert_eq!(outcome.nodes["c"].status, Failed);
    let error = outcome.nodes["c"].error.as_ref().unwrap();
    assert_eq!(error.kind, ExecErrorKind::Template);
    assert_eq!(error.detail.as_ref().unwrap()["pointer"], json!("/path"));
    assert_eq!(outcome.nodes["d"].status, Skipped);
}
