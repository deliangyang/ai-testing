#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};

use classifier::ErrorKind::{
    Assertion, Extraction, Internal, Template, Timeout, Transport, Unsupported,
};
use classifier::{Classifier, ErrorCode, FailedCheck, FailureCategory, FailureInput};
use serde_json::{json, Value};

fn input(kind: classifier::ErrorKind, message: &str) -> FailureInput {
    FailureInput::new(kind, message)
}

fn reason(mut i: FailureInput, reason: &str) -> FailureInput {
    i.reason = Some(reason.to_owned());
    i
}

fn status(mut i: FailureInput, status: u16) -> FailureInput {
    i.http_status = Some(status);
    i
}

fn body(mut i: FailureInput, text: &str) -> FailureInput {
    i.response_text = Some(text.to_owned());
    i
}

fn check(
    path: &str,
    operator: &str,
    expected: Option<Value>,
    actual: Option<Value>,
) -> FailedCheck {
    FailedCheck {
        path: path.to_owned(),
        operator: operator.to_owned(),
        expected,
        actual,
    }
}

fn checks(mut i: FailureInput, checks: Vec<FailedCheck>) -> FailureInput {
    i.checks = checks;
    i
}

fn assertion() -> FailureInput {
    input(Assertion, "1 of 1 assertions failed")
}

/// (sample name, input, expected code).
fn samples() -> Vec<(&'static str, FailureInput, ErrorCode)> {
    use ErrorCode as E;
    let url = "error sending request for url (http://127.0.0.1:9/orders)";
    vec![
        // connection_error
        (
            "refused, english",
            input(Transport, &format!("{url}: client error (Connect): tcp connect error: Connection refused (os error 111)")),
            E::ConnRefused,
        ),
        (
            "refused, localized windows",
            input(Transport, &format!("{url}: client error (Connect): tcp connect error: 由于目标计算机积极拒绝，无法连接。 (os error 10061)")),
            E::ConnRefused,
        ),
        (
            "dns",
            input(Transport, &format!("{url}: client error (Connect): dns error: failed to lookup address information: Name or service not known")),
            E::ConnDns,
        ),
        (
            "dns, localized windows",
            input(Transport, &format!("{url}: client error (Connect): dns error: 不知道这样的主机。 (os error 11001)")),
            E::ConnDns,
        ),
        (
            "tls",
            input(Transport, &format!("{url}: client error (Connect): invalid peer certificate: UnknownIssuer")),
            E::ConnTls,
        ),
        (
            "reset",
            input(Transport, &format!("{url}: client error (SendRequest): connection closed before message completed")),
            E::ConnReset,
        ),
        (
            "reset, localized windows",
            input(Transport, &format!("{url}: 远程主机强迫关闭了一个现有的连接。 (os error 10054)")),
            E::ConnReset,
        ),
        ("transport, unrecognized", input(Transport, "builder error"), E::ConnOther),
        // timeout
        ("request timeout", input(Timeout, "request timed out after 30000 ms"), E::TimeoutRequest),
        (
            "poll deadline",
            reason(input(Timeout, "condition $.body.status eq \"PAID\" not met within 10000 ms after 5 attempts"), "timeout"),
            E::TimeoutPoll,
        ),
        (
            "poll attempts",
            reason(input(Timeout, "condition $.body.status eq \"PAID\" not met after 3 attempts"), "max_attempts"),
            E::TimeoutPollAttempts,
        ),
        ("connect timeout", input(Timeout, "connect timed out after 5000 ms"), E::TimeoutConnect),
        (
            "connect timeout as transport",
            input(Transport, &format!("{url}: tcp connect error: 由于连接方在一段时间后没有正确答复。 (os error 10060)")),
            E::TimeoutConnect,
        ),
        // assertion_failed_status
        (
            "status 201 expected 200",
            checks(assertion(), vec![check("$.status", "eq", Some(json!(200)), Some(json!(201)))]),
            E::AssertStatusMismatch,
        ),
        (
            "status 404",
            checks(assertion(), vec![check("$.status", "eq", Some(json!(200)), Some(json!(404)))]),
            E::AssertStatusClientError,
        ),
        (
            "status 503",
            checks(assertion(), vec![check("$.status", "eq", Some(json!(200)), Some(json!(503)))]),
            E::AssertStatusServerError,
        ),
        (
            "body check against a 500 response",
            status(checks(assertion(), vec![check("$.body.id", "exists", None, None)]), 500),
            E::AssertStatusServerError,
        ),
        (
            "cleanup request rejected with 403",
            status(input(Assertion, "1 of 2 cleanup requests failed"), 403),
            E::AssertStatusClientError,
        ),
        // assertion_failed_body_shape
        (
            "value differs",
            checks(assertion(), vec![check("$.body.state", "eq", Some(json!("PAID")), Some(json!("NEW")))]),
            E::AssertBodyValue,
        ),
        (
            "field missing",
            checks(assertion(), vec![check("$.body.id", "exists", None, None)]),
            E::AssertBodyMissingField,
        ),
        (
            "field missing for eq",
            checks(assertion(), vec![check("$.body.total", "eq", Some(json!(10)), None)]),
            E::AssertBodyMissingField,
        ),
        (
            "type_is",
            checks(assertion(), vec![check("$.body.total", "type_is", Some(json!("number")), Some(json!("10")))]),
            E::AssertBodyType,
        ),
        (
            "eq with a different type",
            checks(assertion(), vec![check("$.body.total", "eq", Some(json!(10)), Some(json!("10")))]),
            E::AssertBodyType,
        ),
        (
            "unexpected field present",
            checks(assertion(), vec![check("$.body.error", "not_exists", None, Some(json!("boom")))]),
            E::AssertBodyValue,
        ),
        // capture_error
        (
            "missing path",
            reason(input(Template, "/path: ${create.output.body.id}: path not found in output (resolved up to create.output.body)"), "missing_path"),
            E::CaptureMissingPath,
        ),
        (
            "missing node",
            reason(input(Template, "/path: ${login.output.token}: no output from node 'login' in scope"), "missing_node"),
            E::CaptureMissingNode,
        ),
        ("extraction", input(Extraction, "cannot extract $.body.token"), E::CaptureExtraction),
        // business_rule_violation
        (
            "409 conflict",
            checks(assertion(), vec![check("$.status", "eq", Some(json!(200)), Some(json!(409)))]),
            E::BizConflict,
        ),
        (
            "422 from a body check",
            status(checks(assertion(), vec![check("$.body.id", "exists", None, None)]), 422),
            E::BizUnprocessable,
        ),
        (
            "illegal transition in body",
            body(
                status(checks(assertion(), vec![check("$.status", "eq", Some(json!(200)), Some(json!(400)))]), 400),
                r#"{"error": "Illegal state transition PAID -> NEW"}"#,
            ),
            E::BizStateTransition,
        ),
        (
            "illegal transition, chinese",
            body(status(assertion(), 400), r#"{"msg": "订单非法状态流转"}"#),
            E::BizStateTransition,
        ),
        // unknown
        (
            "missing env",
            reason(input(Template, "/headers/x: ${env:TOKEN}: environment variable 'TOKEN' is not defined"), "missing_env"),
            E::ConfigMissingEnv,
        ),
        (
            "auth unavailable",
            reason(input(Template, "/headers/authorization: ${auth.token}: auth value 'token' is not available"), "auth_unavailable"),
            E::ConfigAuthUnavailable,
        ),
        (
            "invalid template",
            reason(input(Template, "/path: invalid template at char 3: unterminated '${', missing '}'"), "invalid_template"),
            E::ConfigInvalidTemplate,
        ),
        (
            "no base url",
            input(Template, "relative path '/x' but environment 'local' has no base_url"),
            E::ConfigInvalidTemplate,
        ),
        ("unsupported", input(Unsupported, "HTTP executor cannot run 'db_query' nodes"), E::EngineUnsupported),
        ("internal", input(Internal, "cannot load subworkflow DAG: DAG 'x' has no active version"), E::EngineInternal),
        ("assertion without evidence", assertion(), E::Unknown),
    ]
}

#[test]
fn every_sample_gets_its_code() {
    let classifier = Classifier::new().unwrap();
    let mut failures = Vec::new();
    for (name, input, expected) in samples() {
        let got = classifier.classify(&input);
        if got.code != expected || got.category != expected.category() {
            failures.push(format!(
                "{name}: expected {expected}, got {} via {}",
                got.code, got.rule
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_category_has_at_least_three_samples() {
    let mut per_category: BTreeMap<FailureCategory, usize> = BTreeMap::new();
    for (_, _, code) in samples() {
        *per_category.entry(code.category()).or_default() += 1;
    }
    for category in FailureCategory::ALL {
        let count = per_category.get(&category).copied().unwrap_or(0);
        assert!(count >= 3, "{category}: {count} samples");
    }
}

#[test]
fn every_code_and_every_rule_is_exercised() {
    let classifier = Classifier::new().unwrap();
    let mut codes = BTreeSet::new();
    let mut rules = BTreeSet::new();
    for (_, input, _) in samples() {
        let got = classifier.classify(&input);
        codes.insert(got.code);
        rules.insert(got.rule);
    }
    let all: BTreeSet<ErrorCode> = ErrorCode::ALL.into_iter().collect();
    assert_eq!(codes, all);
    for name in classifier.rule_names() {
        assert!(rules.contains(name), "rule {name} is never used");
    }
    assert!(rules.contains("fallback"));
}

#[test]
fn classification_is_deterministic_and_explains_the_rule() {
    let classifier = Classifier::new().unwrap();
    let input = input(
        Transport,
        "tcp connect error: Connection refused (os error 111)",
    );
    let first = classifier.classify(&input);
    for _ in 0..10 {
        assert_eq!(classifier.classify(&input), first);
    }
    assert_eq!(first.rule, "transport.refused");
    assert_eq!(first.category, FailureCategory::ConnectionError);
}

#[test]
fn input_round_trips_through_json() {
    let input = body(
        status(
            checks(assertion(), vec![check("$.body.id", "exists", None, None)]),
            500,
        ),
        "oops",
    );
    let value = serde_json::to_value(&input).unwrap();
    assert_eq!(value["kind"], "assertion");
    assert_eq!(
        serde_json::from_value::<FailureInput>(value).unwrap(),
        input
    );
}
