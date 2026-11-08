//! Canonical run report written by `--report-json`.
//!
//! The file is one canonical JSON object (`model::to_canonical_json`) with two partitions:
//! `deterministic` (node status, outputs without clocks, assertions, classification) is
//! byte-stable across `--no-llm` runs of the same DAG and environment; `volatile` holds
//! run ids, timestamps and durations. See `docs/run-report.md`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use engine::{Environment, ExecError, NodeResult, NodeStatus, RuleClassifier, RunOutcome};
use model::to_canonical_json;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Map, Value};
use store::RunRecord;

/// Published schema id; bump the number when the document shape changes.
pub const SCHEMA: &str = "aitest.run-report/1";

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RunReport {
    pub schema: String,
    pub deterministic: DeterministicReport,
    pub volatile: VolatileReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct DeterministicReport {
    pub dag_id: String,
    pub dag_version: u32,
    pub env: String,
    pub status: String,
    pub no_llm: bool,
    pub nodes: BTreeMap<String, DeterministicNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct DeterministicNode {
    pub status: NodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FailureRef {
    pub category: String,
    pub error_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct VolatileReport {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub nodes: BTreeMap<String, VolatileNode>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, JsonSchema)]
pub struct VolatileNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<Value>,
}

/// HTTP headers that change between otherwise identical responses.
const VOLATILE_HEADERS: &[&str] = &["date", "expires", "age", "retry-after"];

pub fn build(
    run: &RunRecord,
    outcome: &RunOutcome,
    env: &Environment,
    classifier: &RuleClassifier,
) -> RunReport {
    let mut det_nodes = BTreeMap::new();
    let mut vol_nodes = BTreeMap::new();
    for (id, result) in &outcome.nodes {
        let (det, vol) = split_node(result, env, classifier);
        det_nodes.insert(id.clone(), det);
        if vol != VolatileNode::default() {
            vol_nodes.insert(id.clone(), vol);
        }
    }
    RunReport {
        schema: SCHEMA.to_owned(),
        deterministic: DeterministicReport {
            dag_id: run.dag_id.clone(),
            dag_version: run.dag_version,
            env: run.env_name.clone(),
            status: run.status.to_string(),
            no_llm: true,
            nodes: det_nodes,
        },
        volatile: VolatileReport {
            run_id: run.id.clone(),
            started_at: run.started_at.map(|t| t.to_string()),
            finished_at: run.finished_at.map(|t| t.to_string()),
            nodes: vol_nodes,
        },
    }
}

pub fn write(path: &Path, report: &RunReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, to_canonical_json(report)?)?;
    Ok(())
}

#[cfg(test)]
fn schema_json() -> String {
    let mut out =
        serde_json::to_string_pretty(&schemars::schema_for!(RunReport)).unwrap_or_default();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn split_node(
    result: &NodeResult,
    env: &Environment,
    classifier: &RuleClassifier,
) -> (DeterministicNode, VolatileNode) {
    let mut vol = VolatileNode::default();
    let output = result.output.as_ref().map(|v| {
        let masked = env.mask(v);
        take_clocks(&masked, &mut vol)
    });
    let poll = result.trace.as_ref().map(|v| {
        let masked = env.mask(v);
        take_poll_clocks(&masked, &mut vol)
    });
    let error = result.error.as_ref().map(|e| {
        let masked = mask_error(e, env);
        let mut obj = serde_json::Map::new();
        obj.insert("kind".into(), json!(masked.kind));
        obj.insert("message".into(), json!(masked.message));
        if let Some(detail) = masked.detail {
            obj.insert("detail".into(), take_clocks(&detail, &mut vol));
        }
        Value::Object(obj)
    });
    let assertions = output
        .as_ref()
        .and_then(|o| o.get("results").cloned())
        .or_else(|| {
            error
                .as_ref()
                .and_then(|e| e.get("detail"))
                .and_then(|d| d.get("results").cloned())
        });
    let failure = (result.status == NodeStatus::Failed)
        .then_some(result.error.as_ref())
        .flatten()
        .map(|e| {
            let class = classifier.classify_error(e);
            FailureRef {
                category: class.category.as_str().to_owned(),
                error_code: class.code.as_str().to_owned(),
            }
        });
    (
        DeterministicNode {
            status: result.status,
            output,
            assertions,
            poll,
            error,
            failure,
        },
        vol,
    )
}

fn mask_error(error: &ExecError, env: &Environment) -> ExecError {
    ExecError {
        message: match env.mask(&Value::String(error.message.clone())) {
            Value::String(s) => s,
            _ => error.message.clone(),
        },
        detail: error.detail.as_ref().map(|d| env.mask(d)),
        trace: error.trace.as_ref().map(|t| env.mask(t)),
        ..ExecError::new(error.kind, String::new())
    }
}

fn take_clocks(value: &Value, vol: &mut VolatileNode) -> Value {
    match value {
        Value::Object(map) => {
            let mut kept = Map::new();
            for (key, child) in map {
                match key.as_str() {
                    "timing" => vol.timing = Some(child.clone()),
                    "headers" => {
                        let (stable, extra) = split_headers(child);
                        if extra != Value::Object(Map::new()) {
                            vol.headers = Some(extra);
                        }
                        kept.insert(key.clone(), stable);
                    }
                    "ts" => {}
                    _ => {
                        kept.insert(key.clone(), take_clocks(child, vol));
                    }
                }
            }
            Value::Object(kept)
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| take_clocks(v, vol)).collect()),
        other => other.clone(),
    }
}

fn take_poll_clocks(trace: &Value, vol: &mut VolatileNode) -> Value {
    let Value::Array(entries) = trace else {
        return take_clocks(trace, vol);
    };
    let mut times = Vec::new();
    let mut kept = Vec::new();
    for entry in entries {
        let mut item = take_clocks(entry, vol);
        if let Some(ts) = entry.get("ts") {
            let attempt = entry.get("attempt").cloned().unwrap_or(Value::Null);
            times.push(json!({"attempt": attempt, "ts": ts}));
        }
        if let Some(obj) = item.as_object_mut() {
            obj.remove("ts");
        }
        kept.push(item);
    }
    if !times.is_empty() {
        vol.poll = Some(Value::Array(times));
    }
    Value::Array(kept)
}

fn split_headers(headers: &Value) -> (Value, Value) {
    let Some(map) = headers.as_object() else {
        return (headers.clone(), json!({}));
    };
    let mut stable = Map::new();
    let mut extra = Map::new();
    for (name, value) in map {
        if VOLATILE_HEADERS
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name))
        {
            extra.insert(name.clone(), value.clone());
        } else {
            stable.insert(name.clone(), value.clone());
        }
    }
    (Value::Object(stable), Value::Object(extra))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use engine::ExecErrorKind;
    use serde_json::json;

    fn env() -> Environment {
        Environment::default()
    }

    #[test]
    fn strips_clocks_and_date_headers_from_the_deterministic_side() {
        let classifier = RuleClassifier::new().unwrap();
        let result = NodeResult {
            status: NodeStatus::Completed,
            output: Some(json!({
                "status": 200,
                "headers": {"content-type": "application/json", "date": "Tue, 01 Jan 2026 00:00:00 GMT"},
                "body": {"id": "o-1"},
                "timing": {"duration_ms": 17}
            })),
            error: None,
            trace: Some(json!([
                {"ts": "2026-01-01T00:00:00.000Z", "attempt": 1, "passed": false, "actual": "PENDING"},
                {"ts": "2026-01-01T00:00:00.100Z", "attempt": 2, "passed": true, "actual": "PAID"}
            ])),
        };
        let (det, vol) = split_node(&result, &env(), &classifier);
        let output = det.output.as_ref().unwrap();
        assert_eq!(output["timing"], Value::Null);
        assert!(output["headers"].get("date").is_none());
        assert_eq!(output["headers"]["content-type"], "application/json");
        assert_eq!(det.poll.as_ref().unwrap()[0].get("ts"), None);
        assert_eq!(vol.timing, Some(json!({"duration_ms": 17})));
        assert_eq!(
            vol.headers,
            Some(json!({"date": "Tue, 01 Jan 2026 00:00:00 GMT"}))
        );
        assert_eq!(vol.poll.unwrap()[1]["attempt"], 2);
    }

    #[test]
    fn failed_nodes_carry_classification_and_assertions() {
        let classifier = RuleClassifier::new().unwrap();
        let result = NodeResult {
            status: NodeStatus::Failed,
            output: None,
            error: Some(
                ExecError::new(ExecErrorKind::Assertion, "1 of 1 assertions failed")
                    .with_detail(json!({
                        "results": [{"path": "$.status", "operator": "eq", "expected": 201, "actual": 200, "passed": false}]
                    })),
            ),
            trace: None,
        };
        let (det, _) = split_node(&result, &env(), &classifier);
        assert_eq!(
            det.failure,
            Some(FailureRef {
                category: "assertion_failed_status".into(),
                error_code: "ASSERT_STATUS_MISMATCH".into(),
            })
        );
        assert_eq!(det.assertions.unwrap()[0]["passed"], false);
    }

    fn schema_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/run-report.schema.json")
    }

    #[test]
    fn schema_file_is_up_to_date() {
        let generated = schema_json();
        if std::env::var_os("UPDATE_SCHEMAS").is_some() {
            std::fs::write(schema_path(), &generated).unwrap();
        }
        let on_disk = std::fs::read_to_string(schema_path())
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            on_disk == generated,
            "schemas/run-report.schema.json is stale; regenerate with: \
             UPDATE_SCHEMAS=1 cargo test -p cli schema_file_is_up_to_date"
        );
    }

    #[test]
    fn sample_report_satisfies_the_schema() {
        let schema: Value = serde_json::from_str(&schema_json()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let sample = json!({
            "schema": SCHEMA,
            "deterministic": {
                "dag_id": "serial",
                "dag_version": 1,
                "env": "local",
                "status": "COMPLETED",
                "no_llm": true,
                "nodes": {
                    "check": {
                        "status": "COMPLETED",
                        "assertions": [{"path": "$.status", "operator": "eq", "expected": 200, "actual": 200, "passed": true}]
                    }
                }
            },
            "volatile": {
                "run_id": "run-000001",
                "nodes": {}
            }
        });
        let errors: Vec<String> = validator
            .iter_errors(&sample)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:#?}");
    }
}
