//! Type-specific node configuration.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[serde(alias = "get")]
    Get,
    #[serde(alias = "post")]
    Post,
    #[serde(alias = "put")]
    Put,
    #[serde(alias = "patch")]
    Patch,
    #[serde(alias = "delete")]
    Delete,
    #[serde(alias = "head")]
    Head,
    #[serde(alias = "options")]
    Options,
}

/// A request description. String leaves of `path`, `query`, `headers` and `body` are templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpRequest {
    pub method: HttpMethod,
    /// Relative to the environment `base_url` unless it starts with `http://` or `https://`.
    pub path: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// `http` node: a request plus the expected shape of the response body.
///
/// The request fields are repeated instead of flattened so unknown keys are still rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    pub method: HttpMethod,
    pub path: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// JSON Schema of the response body; used by the gate to check `${node.output.body...}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
}

impl HttpConfig {
    pub fn request(&self) -> HttpRequest {
        HttpRequest {
            method: self.method,
            path: self.path.clone(),
            query: self.query.clone(),
            headers: self.headers.clone(),
            body: self.body.clone(),
            timeout_ms: self.timeout_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    Exists,
    NotExists,
    Regex,
    LenEq,
    TypeIs,
}

impl Operator {
    /// `exists` / `not_exists` take no `expected` value; every other operator requires one.
    pub fn needs_expected(self) -> bool {
        !matches!(self, Operator::Exists | Operator::NotExists)
    }
}

/// A mechanically decidable check: JSONPath into a node output (`{status, headers, body}`),
/// an operator and the expected value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// e.g. `$.body.status`
    pub path: String,
    pub op: Operator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
}

/// `poll` node: repeat `request` until `condition` holds, waiting `backoff_ms[i]` between
/// attempts (the last interval repeats) and failing once `timeout_ms` has elapsed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollConfig {
    pub request: HttpRequest,
    pub condition: Condition,
    pub backoff_ms: Vec<u64>,
    pub timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
}

/// `assert` node: checks evaluated against the output of `target`, reported one by one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertConfig {
    /// Id of the node whose output is checked; must be an ancestor.
    pub target: String,
    pub assertions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbQueryConfig {
    pub connection: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubworkflowConfig {
    pub dag: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupConfig {}
