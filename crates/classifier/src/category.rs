//! Failure categories (develop.md §4.5) and the stable error-code catalog.
//!
//! Both are persisted (`failure_category`, `error_code`) and drive the autonomy policy, so
//! names never change once released; new codes may be added. `docs/error-codes.md` is
//! generated from this module.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    ConnectionError,
    Timeout,
    AssertionFailedStatus,
    AssertionFailedBodyShape,
    CaptureError,
    BusinessRuleViolation,
    Unknown,
}

impl FailureCategory {
    pub const ALL: [FailureCategory; 7] = [
        FailureCategory::ConnectionError,
        FailureCategory::Timeout,
        FailureCategory::AssertionFailedStatus,
        FailureCategory::AssertionFailedBodyShape,
        FailureCategory::CaptureError,
        FailureCategory::BusinessRuleViolation,
        FailureCategory::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FailureCategory::ConnectionError => "connection_error",
            FailureCategory::Timeout => "timeout",
            FailureCategory::AssertionFailedStatus => "assertion_failed_status",
            FailureCategory::AssertionFailedBodyShape => "assertion_failed_body_shape",
            FailureCategory::CaptureError => "capture_error",
            FailureCategory::BusinessRuleViolation => "business_rule_violation",
            FailureCategory::Unknown => "unknown",
        }
    }

    /// What puts a failure in this category (develop.md §4.5).
    pub fn description(self) -> &'static str {
        match self {
            FailureCategory::ConnectionError => "Connection refused, DNS failure, TLS failure.",
            FailureCategory::Timeout => "A request or a poll ran out of time.",
            FailureCategory::AssertionFailedStatus => "The HTTP status code did not match.",
            FailureCategory::AssertionFailedBodyShape => {
                "The response body structure or a field value did not match."
            }
            FailureCategory::CaptureError => {
                "Upstream checks passed but a value could not be extracted (response shape drift)."
            }
            FailureCategory::BusinessRuleViolation => {
                "The system under test rejected the operation by a business rule."
            }
            FailureCategory::Unknown => "Could not be classified; handled manually.",
        }
    }

    /// Codes of this category, in catalog order.
    pub fn codes(self) -> impl Iterator<Item = ErrorCode> {
        ErrorCode::ALL
            .into_iter()
            .filter(move |code| code.category() == self)
    }
}

impl fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} '{value}'")]
pub struct ParseError {
    pub kind: &'static str,
    pub value: String,
}

impl FromStr for FailureCategory {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FailureCategory::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| ParseError {
                kind: "failure category",
                value: s.to_owned(),
            })
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ConnRefused,
    ConnDns,
    ConnTls,
    ConnReset,
    ConnOther,
    TimeoutConnect,
    TimeoutRequest,
    TimeoutPoll,
    TimeoutPollAttempts,
    AssertStatusMismatch,
    AssertStatusClientError,
    AssertStatusServerError,
    AssertBodyValue,
    AssertBodyMissingField,
    AssertBodyType,
    CaptureMissingPath,
    CaptureMissingNode,
    CaptureExtraction,
    BizConflict,
    BizUnprocessable,
    BizStateTransition,
    ConfigMissingEnv,
    ConfigAuthUnavailable,
    ConfigInvalidTemplate,
    EngineUnsupported,
    EngineInternal,
    Unknown,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 27] = [
        ErrorCode::ConnRefused,
        ErrorCode::ConnDns,
        ErrorCode::ConnTls,
        ErrorCode::ConnReset,
        ErrorCode::ConnOther,
        ErrorCode::TimeoutConnect,
        ErrorCode::TimeoutRequest,
        ErrorCode::TimeoutPoll,
        ErrorCode::TimeoutPollAttempts,
        ErrorCode::AssertStatusMismatch,
        ErrorCode::AssertStatusClientError,
        ErrorCode::AssertStatusServerError,
        ErrorCode::AssertBodyValue,
        ErrorCode::AssertBodyMissingField,
        ErrorCode::AssertBodyType,
        ErrorCode::CaptureMissingPath,
        ErrorCode::CaptureMissingNode,
        ErrorCode::CaptureExtraction,
        ErrorCode::BizConflict,
        ErrorCode::BizUnprocessable,
        ErrorCode::BizStateTransition,
        ErrorCode::ConfigMissingEnv,
        ErrorCode::ConfigAuthUnavailable,
        ErrorCode::ConfigInvalidTemplate,
        ErrorCode::EngineUnsupported,
        ErrorCode::EngineInternal,
        ErrorCode::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        self.entry().0
    }

    pub fn category(self) -> FailureCategory {
        self.entry().1
    }

    pub fn summary(self) -> &'static str {
        self.entry().2
    }

    fn entry(self) -> (&'static str, FailureCategory, &'static str) {
        use FailureCategory as C;
        match self {
            ErrorCode::ConnRefused => (
                "CONN_REFUSED",
                C::ConnectionError,
                "The target refused the connection.",
            ),
            ErrorCode::ConnDns => (
                "CONN_DNS",
                C::ConnectionError,
                "The host name could not be resolved.",
            ),
            ErrorCode::ConnTls => (
                "CONN_TLS",
                C::ConnectionError,
                "TLS handshake or certificate verification failed.",
            ),
            ErrorCode::ConnReset => (
                "CONN_RESET",
                C::ConnectionError,
                "The connection was reset or closed before a response arrived.",
            ),
            ErrorCode::ConnOther => (
                "CONN_OTHER",
                C::ConnectionError,
                "Any other transport failure.",
            ),
            ErrorCode::TimeoutConnect => (
                "TIMEOUT_CONNECT",
                C::Timeout,
                "Establishing the connection timed out.",
            ),
            ErrorCode::TimeoutRequest => (
                "TIMEOUT_REQUEST",
                C::Timeout,
                "A request got no complete response within its timeout.",
            ),
            ErrorCode::TimeoutPoll => (
                "TIMEOUT_POLL",
                C::Timeout,
                "A poll condition did not hold within `timeout_ms`.",
            ),
            ErrorCode::TimeoutPollAttempts => (
                "TIMEOUT_POLL_ATTEMPTS",
                C::Timeout,
                "A poll condition did not hold within `max_attempts`.",
            ),
            ErrorCode::AssertStatusMismatch => (
                "ASSERT_STATUS_MISMATCH",
                C::AssertionFailedStatus,
                "The status code differs from the expected one (1xx-3xx actual).",
            ),
            ErrorCode::AssertStatusClientError => (
                "ASSERT_STATUS_CLIENT_ERROR",
                C::AssertionFailedStatus,
                "An unexpected 4xx status code.",
            ),
            ErrorCode::AssertStatusServerError => (
                "ASSERT_STATUS_SERVER_ERROR",
                C::AssertionFailedStatus,
                "An unexpected 5xx status code.",
            ),
            ErrorCode::AssertBodyValue => (
                "ASSERT_BODY_VALUE",
                C::AssertionFailedBodyShape,
                "A field has a different value than expected.",
            ),
            ErrorCode::AssertBodyMissingField => (
                "ASSERT_BODY_MISSING_FIELD",
                C::AssertionFailedBodyShape,
                "An asserted field is absent from the response.",
            ),
            ErrorCode::AssertBodyType => (
                "ASSERT_BODY_TYPE",
                C::AssertionFailedBodyShape,
                "A field has a different JSON type than expected.",
            ),
            ErrorCode::CaptureMissingPath => (
                "CAPTURE_MISSING_PATH",
                C::CaptureError,
                "A referenced field is absent from an upstream output.",
            ),
            ErrorCode::CaptureMissingNode => (
                "CAPTURE_MISSING_NODE",
                C::CaptureError,
                "A referenced upstream node produced no output.",
            ),
            ErrorCode::CaptureExtraction => (
                "CAPTURE_EXTRACTION",
                C::CaptureError,
                "A value could not be extracted from a response.",
            ),
            ErrorCode::BizConflict => (
                "BIZ_CONFLICT",
                C::BusinessRuleViolation,
                "The target reported a conflict with its current state (e.g. 409).",
            ),
            ErrorCode::BizUnprocessable => (
                "BIZ_UNPROCESSABLE",
                C::BusinessRuleViolation,
                "The target rejected a well-formed request as invalid (e.g. 422).",
            ),
            ErrorCode::BizStateTransition => (
                "BIZ_STATE_TRANSITION",
                C::BusinessRuleViolation,
                "The target reported an illegal state transition.",
            ),
            ErrorCode::ConfigMissingEnv => (
                "CONFIG_MISSING_ENV",
                C::Unknown,
                "A referenced environment variable is not defined.",
            ),
            ErrorCode::ConfigAuthUnavailable => (
                "CONFIG_AUTH_UNAVAILABLE",
                C::Unknown,
                "A referenced credential could not be resolved.",
            ),
            ErrorCode::ConfigInvalidTemplate => (
                "CONFIG_INVALID_TEMPLATE",
                C::Unknown,
                "A template could not be parsed at run time.",
            ),
            ErrorCode::EngineUnsupported => (
                "ENGINE_UNSUPPORTED",
                C::Unknown,
                "No executor handles this node type.",
            ),
            ErrorCode::EngineInternal => (
                "ENGINE_INTERNAL",
                C::Unknown,
                "An engine-side failure (store, subworkflow loading, crash).",
            ),
            ErrorCode::Unknown => ("UNKNOWN", C::Unknown, "No rule matched."),
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ErrorCode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ErrorCode::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| ParseError {
                kind: "error code",
                value: s.to_owned(),
            })
    }
}

/// The generated part of `docs/error-codes.md`.
pub fn catalog_markdown() -> String {
    let mut out = String::new();
    for category in FailureCategory::ALL {
        out.push_str(&format!(
            "## `{category}`\n\n{}\n\n| error_code | meaning |\n|---|---|\n",
            category.description()
        ));
        for code in category.codes() {
            out.push_str(&format!("| `{code}` | {} |\n", code.summary()));
        }
        out.push('\n');
    }
    out
}
