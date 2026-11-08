//! `poll` nodes: repeat a request until a condition holds.
//!
//! Attempt `n` waits `backoff_ms[n-1]` before the next one (the last interval repeats). An
//! attempt is only scheduled if it would start no later than `timeout_ms` after the first;
//! otherwise, or once `max_attempts` is reached, the node fails with
//! [`ExecErrorKind::Timeout`]. All waiting goes through the injected [`Sleeper`].
//!
//! Transport errors and request timeouts count as unsatisfied attempts; template errors fail
//! immediately. Every attempt is recorded in the trace:
//! `{ts, attempt, passed, actual?, expected?, diff?, error?}`.

use std::sync::Arc;
use std::time::Duration;

use common::{Clock, Sleeper};
use model::nodes::{Condition, PollConfig};
use model::{Node, NodeKind};
use serde_json::{json, Map, Value};

use crate::condition::{evaluate, ConditionResult};
use crate::executor::{
    ExecContext, ExecError, ExecErrorKind, ExecFuture, ExecOutput, NodeExecutor,
};
use crate::http::HttpExecutor;
use crate::render::RenderError;

pub struct PollExecutor {
    http: Arc<HttpExecutor>,
    clock: Arc<dyn Clock>,
    sleeper: Arc<dyn Sleeper>,
}

impl PollExecutor {
    pub fn new(http: Arc<HttpExecutor>, clock: Arc<dyn Clock>, sleeper: Arc<dyn Sleeper>) -> Self {
        Self {
            http,
            clock,
            sleeper,
        }
    }

    async fn poll(&self, cfg: &PollConfig, ctx: &ExecContext) -> Result<ExecOutput, ExecError> {
        let expected =
            match &cfg.condition.expected {
                Some(value) => Some(ctx.vars().render_value(value).map_err(
                    |mut e: RenderError| {
                        e.pointer = format!("/condition/expected{}", e.pointer);
                        ExecError::from(e)
                    },
                )?),
                None => None,
            };
        let timeout = Duration::from_millis(cfg.timeout_ms);
        let deadline = self.clock.now().saturating_add(timeout);
        let mut trace: Vec<Value> = Vec::new();

        for attempt in 1u32.. {
            let mut entry = Map::new();
            entry.insert("ts".into(), json!(self.clock.now().to_string()));
            entry.insert("attempt".into(), json!(attempt));
            match self.http.send(&cfg.request, ctx, "/request").await {
                Ok(output) => {
                    let result = evaluate(&cfg.condition, expected.as_ref(), &output);
                    record(&mut entry, &result);
                    trace.push(Value::Object(entry));
                    if result.passed {
                        return Ok(ExecOutput {
                            output,
                            trace: Some(Value::Array(trace)),
                            ..ExecOutput::default()
                        });
                    }
                }
                Err(e) if e.kind == ExecErrorKind::Template => {
                    return Err(e.with_trace(Value::Array(trace)));
                }
                Err(e) => {
                    entry.insert("passed".into(), json!(false));
                    if let Some(expected) = &expected {
                        entry.insert("expected".into(), expected.clone());
                    }
                    entry.insert(
                        "error".into(),
                        json!({"kind": e.kind, "message": e.message}),
                    );
                    trace.push(Value::Object(entry));
                }
            }

            if cfg.max_attempts.is_some_and(|max| attempt >= max) {
                return Err(gave_up(cfg, attempt, "max_attempts", trace));
            }
            let delay = backoff(cfg, attempt);
            if self.clock.now().saturating_add(delay) > deadline {
                return Err(gave_up(cfg, attempt, "timeout", trace));
            }
            self.sleeper.sleep(delay).await;
        }
        Err(gave_up(cfg, u32::MAX, "max_attempts", trace))
    }
}

impl NodeExecutor for PollExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            match &node.kind {
                NodeKind::Poll(cfg) => self.poll(cfg, &ctx).await,
                other => Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!("poll executor cannot run '{}' nodes", other.type_name()),
                )),
            }
        })
    }
}

/// Wait after attempt `attempt` (1-based); the last interval repeats.
fn backoff(cfg: &PollConfig, attempt: u32) -> Duration {
    let index = usize::try_from(attempt - 1).unwrap_or(usize::MAX);
    let millis = cfg
        .backoff_ms
        .get(index)
        .or(cfg.backoff_ms.last())
        .copied()
        .unwrap_or(0);
    Duration::from_millis(millis)
}

fn record(entry: &mut Map<String, Value>, result: &ConditionResult) {
    entry.insert("passed".into(), json!(result.passed));
    if let Some(actual) = &result.actual {
        entry.insert("actual".into(), actual.clone());
    }
    if let Some(expected) = &result.expected {
        entry.insert("expected".into(), expected.clone());
    }
    if !result.passed {
        entry.insert("diff".into(), json!(diff(result)));
    }
}

/// Why the condition did not hold, in one line.
fn diff(result: &ConditionResult) -> String {
    if let Some(message) = &result.message {
        return message.clone();
    }
    let op = serde_json::to_value(result.operator).unwrap_or(Value::Null);
    let op = op.as_str().unwrap_or_default().to_owned();
    match (&result.actual, &result.expected) {
        (None, _) => format!("no value at {}", result.path),
        (Some(actual), Some(expected)) => format!("{actual} {op} {expected} is false"),
        (Some(actual), None) => format!("{actual} fails {op}"),
    }
}

fn gave_up(cfg: &PollConfig, attempts: u32, reason: &str, trace: Vec<Value>) -> ExecError {
    let condition = describe(&cfg.condition);
    let message = match reason {
        "timeout" => format!(
            "condition {condition} not met within {} ms after {attempts} attempts",
            cfg.timeout_ms
        ),
        _ => format!("condition {condition} not met after {attempts} attempts"),
    };
    let last = trace.last().cloned().unwrap_or(Value::Null);
    ExecError::new(ExecErrorKind::Timeout, message)
        .with_detail(json!({
            "reason": reason,
            "attempts": attempts,
            "timeout_ms": cfg.timeout_ms,
            "last": last,
        }))
        .with_trace(Value::Array(trace))
}

fn describe(condition: &Condition) -> String {
    let op = serde_json::to_value(condition.op).unwrap_or(Value::Null);
    let mut text = format!("{} {}", condition.path, op.as_str().unwrap_or_default());
    if let Some(expected) = &condition.expected {
        text.push_str(&format!(" {expected}"));
    }
    text
}
