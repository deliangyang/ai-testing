//! Feeding executor errors to the rule-based classifier.

use classifier::{Classifier, ErrorKind, FailedCheck, FailureInput, RuleError};
use serde_json::Value;
use store::Failure;

use crate::executor::{ExecError, ExecErrorKind};
use crate::record::FailureClassifier;

/// The evidence the classifier rules look at, taken from the error and its detail:
/// `detail.reason` (render failures, poll give-up), failed `detail.results` (assertions),
/// `detail.response` (assertion target) and the status of the first failed cleanup request.
pub fn failure_input(error: &ExecError) -> FailureInput {
    let detail = error.detail.as_ref();
    let field = |name: &str| detail.and_then(|d| d.get(name));
    let mut input = FailureInput::new(kind(error.kind), error.message.clone());
    input.reason = field("reason").and_then(Value::as_str).map(str::to_owned);
    input.checks = field("results")
        .and_then(Value::as_array)
        .map(|results| results.iter().filter_map(failed_check).collect())
        .unwrap_or_default();
    let response = field("response");
    input.http_status = response
        .and_then(|r| r.get("status"))
        .or_else(|| first_failed_cleanup_status(field("results")))
        .and_then(Value::as_u64)
        .and_then(|s| u16::try_from(s).ok());
    input.response_text = response
        .and_then(|r| r.get("body_excerpt"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    input
}

fn kind(kind: ExecErrorKind) -> ErrorKind {
    match kind {
        ExecErrorKind::Template => ErrorKind::Template,
        ExecErrorKind::Transport => ErrorKind::Transport,
        ExecErrorKind::Timeout => ErrorKind::Timeout,
        ExecErrorKind::Assertion => ErrorKind::Assertion,
        ExecErrorKind::Extraction => ErrorKind::Extraction,
        ExecErrorKind::Unsupported => ErrorKind::Unsupported,
        // Cancelled nodes are recorded as SKIPPED and never classified.
        ExecErrorKind::Internal | ExecErrorKind::Cancelled => ErrorKind::Internal,
    }
}

/// A condition result (`{path, operator, passed, expected?, actual?}`) that did not pass.
fn failed_check(result: &Value) -> Option<FailedCheck> {
    if result.get("passed")?.as_bool()? {
        return None;
    }
    Some(FailedCheck {
        path: result.get("path")?.as_str()?.to_owned(),
        operator: result.get("operator")?.as_str()?.to_owned(),
        expected: result.get("expected").cloned(),
        actual: result.get("actual").cloned(),
    })
}

/// Cleanup results are `{index, status?, error?}`; the first one with an error.
fn first_failed_cleanup_status(results: Option<&Value>) -> Option<&Value> {
    results?
        .as_array()?
        .iter()
        .find(|r| r.get("error").is_some())?
        .get("status")
}

/// [`FailureClassifier`] backed by the standard rule table.
pub struct RuleClassifier {
    rules: Classifier,
}

impl RuleClassifier {
    pub fn new() -> Result<Self, RuleError> {
        Ok(Self {
            rules: Classifier::new()?,
        })
    }

    pub fn classify_error(&self, error: &ExecError) -> classifier::Classification {
        self.rules.classify(&failure_input(error))
    }
}

impl FailureClassifier for RuleClassifier {
    fn classify(&self, error: &ExecError) -> Failure {
        let result = self.classify_error(error);
        Failure {
            category: result.category.as_str().to_owned(),
            error_code: result.code.as_str().to_owned(),
        }
    }
}
