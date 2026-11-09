//! Normalized HTTP API definition (develop.md §3.1 `api_specs`).

use std::collections::BTreeMap;

use model::to_canonical_json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use store::ApiSpecSaveInput;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ParamLocation {
    Query,
    Path,
    Header,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiParam {
    pub name: String,
    pub location: ParamLocation,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

/// Domain model for an imported API endpoint; aligned with the `api_specs` table columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiSpec {
    pub id: String,
    pub name: String,
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ApiParam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_scheme_ref: Option<String>,
}

pub fn api_spec_schema() -> schemars::Schema {
    schemars::schema_for!(ApiSpec)
}

pub fn api_spec_schema_json() -> String {
    let mut out = serde_json::to_string_pretty(&api_spec_schema()).unwrap_or_default();
    out.push('\n');
    out
}

/// Deterministic cleanup before persistence (method casing, path shape, sorted params).
pub fn normalize_api_spec(mut spec: ApiSpec) -> ApiSpec {
    spec.id = spec.id.trim().to_owned();
    spec.name = spec.name.trim().to_owned();
    spec.method = spec.method.trim().to_uppercase();
    let path = spec.path.trim();
    spec.path = if path.is_empty() || path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    spec.params.sort_by(|a, b| a.name.cmp(&b.name));
    for param in &mut spec.params {
        param.name = param.name.trim().to_owned();
    }
    if let Some(schema) = spec.response_schema.as_mut() {
        if let Ok(canonical) = to_canonical_json(schema) {
            if let Ok(parsed) = serde_json::from_str(&canonical) {
                *schema = parsed;
            }
        }
    }
    spec
}

impl ApiSpec {
    /// Reconstruct a spec from a store row (for merge against active catalog).
    pub fn from_save_input(input: &ApiSpecSaveInput) -> Self {
        let params: Vec<ApiParam> = input
            .params
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        Self {
            id: input.id.clone(),
            name: input.name.clone(),
            method: input.method.clone(),
            path: input.path.clone(),
            version: None,
            params,
            response_schema: input.response_schema.clone(),
            auth_scheme_ref: input.auth_scheme_ref.clone(),
        }
    }

    pub fn into_save_input(self) -> ApiSpecSaveInput {
        let normalized = normalize_api_spec(self);
        ApiSpecSaveInput {
            id: normalized.id,
            name: normalized.name,
            method: normalized.method,
            path: normalized.path,
            params: params_to_json(&normalized.params),
            response_schema: normalized.response_schema,
            auth_scheme_ref: normalized.auth_scheme_ref,
        }
    }

    /// Canonical JSON of the spec document (params as a sorted array, stable key order).
    pub fn to_canonical_json(&self) -> Result<String, model::CanonicalError> {
        let normalized = normalize_api_spec(self.clone());
        let mut map = BTreeMap::new();
        map.insert("id", Value::String(normalized.id));
        map.insert("method", Value::String(normalized.method));
        map.insert("name", Value::String(normalized.name));
        map.insert("path", Value::String(normalized.path));
        if let Some(auth) = normalized.auth_scheme_ref {
            map.insert("auth_scheme_ref", Value::String(auth));
        }
        if !normalized.params.is_empty() {
            map.insert(
                "params",
                serde_json::to_value(&normalized.params).unwrap_or(Value::Null),
            );
        }
        if let Some(schema) = normalized.response_schema {
            map.insert("response_schema", schema);
        }
        if let Some(version) = normalized.version {
            map.insert("version", Value::Number(serde_json::Number::from(version)));
        }
        to_canonical_json(&map)
    }
}

fn params_to_json(params: &[ApiParam]) -> Option<Value> {
    if params.is_empty() {
        return None;
    }
    serde_json::to_value(params).ok()
}
