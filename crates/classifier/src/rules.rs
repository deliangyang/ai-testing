//! The deterministic classifier: an ordered rule table over facts derived from a failure.
//!
//! The input is first reduced to [`Facts`] (error kind, detail reason, HTTP status, the kind of
//! the first failed check, and the message text). Rules are tried in order and the first match
//! wins, so specific rules come before general ones. Text patterns match the engine's message
//! and the response excerpt; transport patterns also match OS error numbers because OS error
//! texts are localized.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::category::{ErrorCode, FailureCategory};

/// What failed, as reported by the engine. Names match the engine's `ExecErrorKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Template,
    Transport,
    Timeout,
    Assertion,
    Extraction,
    Unsupported,
    Internal,
}

/// One check that did not hold: `{path, operator, expected?, actual?}` as evaluated by the
/// engine. `actual: None` means nothing was found at `path`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FailedCheck {
    pub path: String,
    pub operator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureInput {
    pub kind: ErrorKind,
    /// The raw error message.
    pub message: String,
    /// Status of the response the failure is about, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    /// Machine-readable sub-reason from the error detail: a render failure (`missing_path`,
    /// `missing_node`, `missing_env`, `auth_unavailable`, `invalid_template`) or why a poll
    /// gave up (`timeout`, `max_attempts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Failed checks in evaluation order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<FailedCheck>,
    /// Start of the response body, for business-rule messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
}

impl FailureInput {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            http_status: None,
            reason: None,
            checks: Vec::new(),
            response_text: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Classification {
    pub category: FailureCategory,
    pub code: ErrorCode,
    /// Name of the rule that matched, for explaining the decision.
    pub rule: &'static str,
}

/// How the first failed check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckKind {
    Status,
    BodyMissing,
    BodyType,
    BodyValue,
}

#[derive(Debug)]
struct Facts<'a> {
    kind: ErrorKind,
    reason: Option<&'a str>,
    status: Option<u16>,
    check: Option<CheckKind>,
    text: String,
}

#[derive(Debug, Clone, Copy)]
enum StatusRange {
    Exactly(u16),
    Between(u16, u16),
}

impl StatusRange {
    fn contains(self, status: Option<u16>) -> bool {
        match (self, status) {
            (StatusRange::Exactly(want), Some(s)) => s == want,
            (StatusRange::Between(lo, hi), Some(s)) => (lo..=hi).contains(&s),
            (_, None) => false,
        }
    }
}

/// Every condition that is set must hold.
#[derive(Debug, Clone, Copy)]
struct RuleSpec {
    name: &'static str,
    code: ErrorCode,
    kind: Option<ErrorKind>,
    reason: Option<&'static str>,
    status: Option<StatusRange>,
    check: Option<CheckKind>,
    pattern: Option<&'static str>,
}

const fn rule(name: &'static str, code: ErrorCode) -> RuleSpec {
    RuleSpec {
        name,
        code,
        kind: None,
        reason: None,
        status: None,
        check: None,
        pattern: None,
    }
}

impl RuleSpec {
    const fn kind(mut self, kind: ErrorKind) -> Self {
        self.kind = Some(kind);
        self
    }
    const fn reason(mut self, reason: &'static str) -> Self {
        self.reason = Some(reason);
        self
    }
    const fn status(mut self, status: StatusRange) -> Self {
        self.status = Some(status);
        self
    }
    const fn check(mut self, check: CheckKind) -> Self {
        self.check = Some(check);
        self
    }
    const fn pattern(mut self, pattern: &'static str) -> Self {
        self.pattern = Some(pattern);
        self
    }
}

use CheckKind::{BodyMissing, BodyType, BodyValue, Status};
use ErrorCode as E;
use ErrorKind::{Assertion, Extraction, Internal, Template, Timeout, Transport, Unsupported};
use StatusRange::{Between, Exactly};

const TLS: &str = r"(?i)\btls\b|certificate|handshake|InvalidCertificate|UnknownIssuer";
const DNS: &str = r"(?i)dns error|failed to lookup address|name or service not known|no such host|nodename nor servname|getaddrinfo|os error (11001|11004|-2|-3)\b";
const REFUSED: &str = r"(?i)connection refused|actively refused|os error (10061|111|61)\b";
const RESET: &str = r"(?i)connection reset|connection (was )?closed|broken pipe|IncompleteMessage|connection aborted|os error (10054|10053|104|54|32)\b";
const CONNECT_TIMEOUT: &str = r"(?i)connect(ion)? timed out|os error (10060|110|60)\b";
const STATE_TRANSITION: &str = r"(?i)(illegal|invalid|not allowed|cannot|can't)\s+(state\s+)?(transition|transit)|state transition (is )?(not allowed|invalid|illegal)|非法(的)?状态(流转|转换)|状态(流转|转换)非法";

/// The rule table, most specific first.
const RULES: &[RuleSpec] = &[
    // Rendering: a missing upstream value is response-shape drift, the rest is configuration.
    rule("template.missing_path", E::CaptureMissingPath)
        .kind(Template)
        .reason("missing_path"),
    rule("template.missing_node", E::CaptureMissingNode)
        .kind(Template)
        .reason("missing_node"),
    rule("template.missing_env", E::ConfigMissingEnv)
        .kind(Template)
        .reason("missing_env"),
    rule("template.auth_unavailable", E::ConfigAuthUnavailable)
        .kind(Template)
        .reason("auth_unavailable"),
    rule("template.other", E::ConfigInvalidTemplate).kind(Template),
    rule("extraction", E::CaptureExtraction).kind(Extraction),
    // Timeouts.
    rule("timeout.poll_deadline", E::TimeoutPoll)
        .kind(Timeout)
        .reason("timeout"),
    rule("timeout.poll_attempts", E::TimeoutPollAttempts)
        .kind(Timeout)
        .reason("max_attempts"),
    rule("timeout.connect", E::TimeoutConnect)
        .kind(Timeout)
        .pattern(r"(?i)\bconnect"),
    rule("timeout.request", E::TimeoutRequest).kind(Timeout),
    // Transport.
    rule("transport.tls", E::ConnTls)
        .kind(Transport)
        .pattern(TLS),
    rule("transport.dns", E::ConnDns)
        .kind(Transport)
        .pattern(DNS),
    rule("transport.refused", E::ConnRefused)
        .kind(Transport)
        .pattern(REFUSED),
    rule("transport.connect_timeout", E::TimeoutConnect)
        .kind(Transport)
        .pattern(CONNECT_TIMEOUT),
    rule("transport.reset", E::ConnReset)
        .kind(Transport)
        .pattern(RESET),
    rule("transport.other", E::ConnOther).kind(Transport),
    // Business rules reported by the system under test.
    rule("business.state_transition", E::BizStateTransition)
        .kind(Assertion)
        .pattern(STATE_TRANSITION),
    rule("business.conflict", E::BizConflict)
        .kind(Assertion)
        .status(Exactly(409)),
    rule("business.unprocessable", E::BizUnprocessable)
        .kind(Assertion)
        .status(Exactly(422)),
    // Status: an error status explains failed body checks too.
    rule("status.server_error", E::AssertStatusServerError)
        .kind(Assertion)
        .status(Between(500, 599)),
    rule("status.client_error", E::AssertStatusClientError)
        .kind(Assertion)
        .status(Between(400, 499)),
    rule("status.mismatch", E::AssertStatusMismatch)
        .kind(Assertion)
        .check(Status),
    // Body.
    rule("body.missing_field", E::AssertBodyMissingField)
        .kind(Assertion)
        .check(BodyMissing),
    rule("body.type", E::AssertBodyType)
        .kind(Assertion)
        .check(BodyType),
    rule("body.value", E::AssertBodyValue)
        .kind(Assertion)
        .check(BodyValue),
    // Engine.
    rule("engine.unsupported", E::EngineUnsupported).kind(Unsupported),
    rule("engine.internal", E::EngineInternal).kind(Internal),
];

struct Compiled {
    spec: RuleSpec,
    pattern: Option<Regex>,
}

/// Rule-based failure classifier. Pure: the same input always yields the same result.
pub struct Classifier {
    rules: Vec<Compiled>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("rule '{rule}' has an invalid pattern: {message}")]
pub struct RuleError {
    pub rule: &'static str,
    pub message: String,
}

impl Classifier {
    /// The standard rule table.
    pub fn new() -> Result<Self, RuleError> {
        let rules = RULES
            .iter()
            .map(|spec| {
                let pattern = spec
                    .pattern
                    .map(Regex::new)
                    .transpose()
                    .map_err(|e| RuleError {
                        rule: spec.name,
                        message: e.to_string(),
                    })?;
                Ok(Compiled {
                    spec: *spec,
                    pattern,
                })
            })
            .collect::<Result<_, RuleError>>()?;
        Ok(Self { rules })
    }

    /// Rule names in evaluation order.
    pub fn rule_names(&self) -> Vec<&'static str> {
        self.rules.iter().map(|r| r.spec.name).collect()
    }

    pub fn classify(&self, input: &FailureInput) -> Classification {
        let facts = facts(input);
        for compiled in &self.rules {
            if matches(compiled, &facts) {
                return Classification {
                    category: compiled.spec.code.category(),
                    code: compiled.spec.code,
                    rule: compiled.spec.name,
                };
            }
        }
        Classification {
            category: FailureCategory::Unknown,
            code: ErrorCode::Unknown,
            rule: "fallback",
        }
    }
}

fn matches(compiled: &Compiled, facts: &Facts<'_>) -> bool {
    let spec = &compiled.spec;
    spec.kind.is_none_or(|k| k == facts.kind)
        && spec.reason.is_none_or(|r| facts.reason == Some(r))
        && spec.status.is_none_or(|s| s.contains(facts.status))
        && spec.check.is_none_or(|c| facts.check == Some(c))
        && compiled
            .pattern
            .as_ref()
            .is_none_or(|p| p.is_match(&facts.text))
}

/// JSONPaths that address the response status in the engine's `{status, headers, body}`.
const STATUS_PATHS: [&str; 2] = ["$.status", "$['status']"];

fn facts(input: &FailureInput) -> Facts<'_> {
    let first = input.checks.first();
    let check = first.map(check_kind);
    let status = input.http_status.or_else(|| {
        first
            .filter(|c| check_kind(c) == Status)
            .and_then(|c| c.actual.as_ref()?.as_u64())
            .and_then(|s| u16::try_from(s).ok())
    });
    let mut text = input.message.clone();
    if let Some(body) = &input.response_text {
        text.push('\n');
        text.push_str(body);
    }
    Facts {
        kind: input.kind,
        reason: input.reason.as_deref(),
        status,
        check,
        text,
    }
}

fn check_kind(check: &FailedCheck) -> CheckKind {
    if STATUS_PATHS.contains(&check.path.as_str()) {
        return Status;
    }
    match (check.operator.as_str(), &check.actual) {
        ("not_exists", _) => BodyValue,
        ("type_is", _) => BodyType,
        (_, None) => BodyMissing,
        (_, Some(actual)) => match &check.expected {
            Some(expected)
                if comparable(&check.operator) && json_type(expected) != json_type(actual) =>
            {
                BodyType
            }
            _ => BodyValue,
        },
    }
}

/// Operators whose expected value has the same type as the actual value.
fn comparable(operator: &str) -> bool {
    matches!(operator, "eq" | "ne" | "gt" | "gte" | "lt" | "lte")
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
