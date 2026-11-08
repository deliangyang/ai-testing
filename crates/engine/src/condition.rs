//! Evaluating a structured condition against a node output (used by assert and poll).
//!
//! - `eq`/`ne`: JSON equality; numbers compare by value, so `1` equals `1.0`.
//! - `gt`/`gte`/`lt`/`lte`: both sides numbers.
//! - `contains`: substring of a string, element of an array, or key of an object.
//! - `exists`/`not_exists`: whether the path selects a value (`null` counts as existing).
//! - `regex`: unanchored match against a string.
//! - `len_eq`: characters of a string, elements of an array or keys of an object.
//! - `type_is`: one of [`Operator::TYPE_NAMES`].

use model::nodes::{Condition, Operator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jsonpath::select;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionResult {
    pub path: String,
    pub operator: Operator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    /// Absent when the path selects nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    pub passed: bool,
    /// Why the check could not be decided as written (wrong operand type, bad path...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// `expected` is the condition's expected value with templates already rendered.
pub fn evaluate(
    condition: &Condition,
    expected: Option<&Value>,
    output: &Value,
) -> ConditionResult {
    let mut result = ConditionResult {
        path: condition.path.clone(),
        operator: condition.op,
        expected: expected.cloned(),
        actual: None,
        passed: false,
        message: None,
    };
    let actual = match select(output, &condition.path) {
        Ok(actual) => actual,
        Err(e) => {
            result.message = Some(e.to_string());
            return result;
        }
    };
    result.actual = actual.cloned();
    match check(condition.op, actual, expected) {
        Ok(passed) => result.passed = passed,
        Err(message) => result.message = Some(message),
    }
    result
}

fn check(op: Operator, actual: Option<&Value>, expected: Option<&Value>) -> Result<bool, String> {
    match op {
        Operator::Exists => return Ok(actual.is_some()),
        Operator::NotExists => return Ok(actual.is_none()),
        _ => {}
    }
    let Some(actual) = actual else {
        return Ok(false);
    };
    let expected =
        expected.ok_or_else(|| format!("operator '{}' needs an expected value", name(op)))?;
    match op {
        Operator::Eq => Ok(json_eq(actual, expected)),
        Operator::Ne => Ok(!json_eq(actual, expected)),
        Operator::Gt | Operator::Gte | Operator::Lt | Operator::Lte => {
            let (Some(a), Some(e)) = (actual.as_f64(), expected.as_f64()) else {
                return Err(format!(
                    "operator '{}' compares numbers, got {} and {}",
                    name(op),
                    type_name(actual),
                    type_name(expected)
                ));
            };
            Ok(match op {
                Operator::Gt => a > e,
                Operator::Gte => a >= e,
                Operator::Lt => a < e,
                _ => a <= e,
            })
        }
        Operator::Contains => match (actual, expected) {
            (Value::String(a), Value::String(e)) => Ok(a.contains(e.as_str())),
            (Value::Array(items), e) => Ok(items.iter().any(|item| json_eq(item, e))),
            (Value::Object(map), Value::String(key)) => Ok(map.contains_key(key)),
            (a, e) => Err(format!(
                "operator 'contains' cannot look for {} in {}",
                type_name(e),
                type_name(a)
            )),
        },
        Operator::Regex => {
            let Value::String(pattern) = expected else {
                return Err("operator 'regex' needs a string pattern".into());
            };
            let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
            match actual {
                Value::String(a) => Ok(re.is_match(a)),
                other => Err(format!(
                    "operator 'regex' matches strings, got {}",
                    type_name(other)
                )),
            }
        }
        Operator::LenEq => {
            let want = expected
                .as_u64()
                .ok_or("operator 'len_eq' needs a non-negative integer")?;
            let len = match actual {
                Value::String(s) => s.chars().count(),
                Value::Array(items) => items.len(),
                Value::Object(map) => map.len(),
                other => {
                    return Err(format!(
                        "operator 'len_eq' needs a string, array or object, got {}",
                        type_name(other)
                    ))
                }
            };
            Ok(u64::try_from(len).is_ok_and(|len| len == want))
        }
        Operator::TypeIs => match expected.as_str() {
            Some(t) if Operator::TYPE_NAMES.contains(&t) => Ok(is_type(actual, t)),
            _ => Err(format!(
                "operator 'type_is' needs one of {}",
                Operator::TYPE_NAMES.join("/")
            )),
        },
        Operator::Exists | Operator::NotExists => Ok(false),
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) if x.is_f64() || y.is_f64() => {
            x.as_f64() == y.as_f64()
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| json_eq(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

fn is_type(value: &Value, name: &str) -> bool {
    match name {
        "integer" => {
            value.is_i64() || value.is_u64() || value.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        "number" => value.is_number(),
        other => type_name(value) == other,
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn name(op: Operator) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}
