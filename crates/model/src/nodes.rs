//! Type-specific node configuration.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// Active `api_specs` row to resolve response shape when set; overrides inline `response_schema` for gate checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_spec_id: Option<String>,
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
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
    /// Accepted `expected` values of `type_is`. A number with no fractional part is both an
    /// `integer` and a `number`.
    pub const TYPE_NAMES: [&'static str; 7] = [
        "null", "boolean", "integer", "number", "string", "array", "object",
    ];

    /// `exists` / `not_exists` take no `expected` value; every other operator requires one.
    pub fn needs_expected(self) -> bool {
        !matches!(self, Operator::Exists | Operator::NotExists)
    }
}

/// A mechanically decidable check: JSONPath into a node output (`{status, headers, body}`),
/// an operator and the expected value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// e.g. `$.body.status`
    pub path: String,
    pub op: Operator,
    /// An explicit `null` is a value to compare against, distinct from an absent key.
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected: Option<Value>,
}

fn present_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// `poll` node: repeat `request` until `condition` holds, waiting `backoff_ms[i]` between
/// attempts (the last interval repeats) and failing once `timeout_ms` has elapsed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssertConfig {
    /// Id of the node whose output is checked; must be an ancestor.
    pub target: String,
    pub assertions: Vec<Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResultMode {
    /// Output `{ rows: [...] }`.
    #[default]
    Rows,
    /// Output `{ row: {...} }`; zero rows is a failure.
    FirstRow,
    /// Output `{ value: ... }` from the first column of the first row.
    Scalar,
}

/// `db_query` node: parameterized SQL against a named connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DbQueryConfig {
    /// Name of a connection asset bound to the current environment.
    pub connection: String,
    /// Uses the driver's positional placeholders; values never get interpolated into SQL text.
    pub sql: String,
    /// Positional parameters; string leaves are templates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Value>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub result: QueryResultMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
}

/// Pin to a stored DAG. `version: None` resolves to the active version at run time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DagRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
}

/// Pin to a stored Skill. `version_req: None` resolves to the active Skill at run time;
/// otherwise the highest matching manifest semver among activated revisions wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_req: Option<String>,
}

/// `subworkflow` node: runs a child DAG in its own variable scope.
///
/// The child sees only `inputs`, as the reserved node id `input` (`${input.output.<name>}`).
/// The parent sees only `outputs`, as `${<this node>.output.<name>}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubworkflowConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dag: Option<DagRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillRef>,
    /// Input name -> value; templates evaluated in the parent scope.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Value>,
    /// Output name -> template evaluated in the child scope, e.g. `${login.output.body.token}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, String>,
}

impl SubworkflowConfig {
    /// Exactly one of [`Self::dag`] or [`Self::skill`] must be set.
    pub fn validate_target(&self) -> Result<(), String> {
        match (&self.dag, &self.skill) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(_), Some(_)) => Err("subworkflow cannot set both dag and skill".into()),
            (None, None) => Err("subworkflow requires dag or skill".into()),
        }
    }
}

/// `cleanup` node: always runs when the DAG reaches a terminal state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanupConfig {
    /// Executed in order; a failing request does not stop the remaining ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<HttpRequest>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
