//! Shared heuristic helpers for cURL / HAR import (request restore + JSON Schema inference).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::api_spec::{ApiParam, ParamLocation};

pub fn derive_api_id(method: &str, path: &str) -> String {
    let method = method.to_ascii_lowercase();
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return format!("root.{method}");
    }
    let slug = trimmed.replace('/', ".").replace(['{', '}'], "");
    format!("{slug}.{method}")
}

pub fn extract_path_from_url(url: &str) -> String {
    let bare = url.trim_matches(|c| c == '\'' || c == '"');
    let (base, _) = bare.split_once('?').map_or((bare, ""), |(b, q)| (b, q));
    if let Some(idx) = base.find("://") {
        let rest = &base[idx + 3..];
        if let Some(slash) = rest.find('/') {
            return rest[slash..].to_string();
        }
        return "/".to_string();
    }
    if base.starts_with('/') {
        return base.to_string();
    }
    format!("/{base}")
}

pub fn parse_query_pairs(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        let (name, value) = part.split_once('=').map_or((part, ""), |(n, v)| (n, v));
        pairs.push((name.to_string(), value.to_string()));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

pub fn query_pairs_from_url(url: &str) -> Vec<(String, String)> {
    url.split_once('?')
        .map(|(_, q)| parse_query_pairs(q))
        .unwrap_or_default()
}

pub fn infer_string_schema(sample: &str) -> Value {
    if sample == "true" || sample == "false" {
        return json!({ "type": "boolean" });
    }
    if sample.parse::<i64>().is_ok() {
        return json!({ "type": "integer" });
    }
    if sample.parse::<f64>().is_ok() {
        return json!({ "type": "number" });
    }
    json!({ "type": "string" })
}

pub fn infer_body_schema(data: &str) -> Value {
    if let Ok(value) = serde_json::from_str::<Value>(data) {
        return infer_json_schema(&value);
    }
    json!({ "type": "string" })
}

pub fn infer_json_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut properties = BTreeMap::new();
            for (key, val) in map {
                properties.insert(key.clone(), infer_json_schema(val));
            }
            json!({
                "type": "object",
                "properties": properties
            })
        }
        Value::Array(items) => {
            let item_schema = items
                .first()
                .map(infer_json_schema)
                .unwrap_or(json!({ "type": "string" }));
            json!({
                "type": "array",
                "items": item_schema
            })
        }
        Value::String(_) => json!({ "type": "string" }),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                json!({ "type": "integer" })
            } else {
                json!({ "type": "number" })
            }
        }
        Value::Bool(_) => json!({ "type": "boolean" }),
        Value::Null => json!({ "type": "null" }),
    }
}

pub fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "x-api-key" | "api-key" | "x-auth-token"
    )
}

pub fn header_param(name: String, value: &str) -> ApiParam {
    let lower = name.to_ascii_lowercase();
    let sensitive = is_sensitive_header(&lower);
    ApiParam {
        name: lower,
        location: ParamLocation::Header,
        required: false,
        schema: if sensitive {
            Some(json!({ "type": "string" }))
        } else {
            Some(json!({
                "type": "string",
                "const": value
            }))
        },
    }
}

pub fn merge_params(existing: &mut Vec<ApiParam>, incoming: Vec<ApiParam>) {
    let mut index: BTreeMap<(ParamLocation, String), usize> = BTreeMap::new();
    for (i, p) in existing.iter().enumerate() {
        index.insert((p.location, p.name.clone()), i);
    }
    for param in incoming {
        let key = (param.location, param.name.clone());
        if let Some(&i) = index.get(&key) {
            if existing[i].schema.is_none() {
                existing[i].schema = param.schema;
            }
        } else {
            index.insert(key, existing.len());
            existing.push(param);
        }
    }
    existing.sort_by(|a, b| a.name.cmp(&b.name));
}
