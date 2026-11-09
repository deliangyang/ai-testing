//! Postman Collection v2.1 → [`ApiSpec`] (develop.md §4.10).

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};

const V21_SCHEMA_MARKER: &str = "v2.1.0";

#[derive(Debug, thiserror::Error)]
pub enum PostmanParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("collection must be a JSON object")]
    NotObject,
    #[error("missing or invalid `info` object")]
    MissingInfo,
    #[error("unsupported Postman collection schema (expected v2.1.0, got {found})")]
    UnsupportedSchema { found: String },
    #[error("missing `item` array")]
    MissingItems,
    #[error("request `{name}` has no URL")]
    MissingUrl { name: String },
}

#[derive(Debug, Deserialize)]
struct Collection {
    info: Info,
    #[serde(default)]
    item: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Info {
    name: String,
    #[serde(default, rename = "schema")]
    schema_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Item {
    name: String,
    #[serde(default)]
    item: Vec<Item>,
    request: Option<Request>,
}

#[derive(Debug, Deserialize)]
struct Request {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    header: Vec<Header>,
    body: Option<Body>,
    #[serde(default)]
    url: Option<UrlField>,
}

#[derive(Debug, Deserialize)]
struct Header {
    key: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct Body {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default, rename = "urlencoded")]
    url_encoded: Vec<KeyValue>,
    #[serde(default)]
    formdata: Vec<KeyValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct KeyValue {
    key: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UrlField {
    Text(String),
    Object(UrlObject),
}

#[derive(Debug, Deserialize)]
struct UrlObject {
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    host: Vec<String>,
    #[serde(default)]
    path: Vec<String>,
    #[serde(default)]
    query: Vec<KeyValue>,
    #[serde(default)]
    variable: Vec<KeyValue>,
}

/// Parse a Postman Collection v2.1 JSON document.
pub fn parse_postman_collection_str(input: &str) -> Result<Vec<ApiSpec>, PostmanParseError> {
    let value: Value = serde_json::from_str(input)?;
    parse_postman_collection(&value)
}

/// Parse a Postman Collection v2.1 value (must be a JSON object).
pub fn parse_postman_collection(value: &Value) -> Result<Vec<ApiSpec>, PostmanParseError> {
    let root = value.as_object().ok_or(PostmanParseError::NotObject)?;
    let collection: Collection = serde_json::from_value(Value::Object(root.clone()))?;
    validate_schema(collection.info.schema_url.as_deref())?;

    let mut specs = Vec::new();
    let mut seen_ids = BTreeSet::new();
    collect_items(
        &collection.item,
        &collection.info.name,
        &mut Vec::new(),
        &mut specs,
        &mut seen_ids,
    )?;

    specs.sort_by(|a, b| a.id.cmp(&b.id));
    for spec in &mut specs {
        *spec = normalize_api_spec(spec.clone());
    }
    Ok(specs)
}

fn validate_schema(schema_url: Option<&str>) -> Result<(), PostmanParseError> {
    let Some(url) = schema_url else {
        return Ok(());
    };
    if url.contains(V21_SCHEMA_MARKER) {
        return Ok(());
    }
    Err(PostmanParseError::UnsupportedSchema {
        found: url.to_owned(),
    })
}

fn collect_items(
    items: &[Item],
    collection_name: &str,
    folder_stack: &mut Vec<String>,
    out: &mut Vec<ApiSpec>,
    seen_ids: &mut BTreeSet<String>,
) -> Result<(), PostmanParseError> {
    for item in items {
        if let Some(request) = &item.request {
            let display_name = build_display_name(collection_name, folder_stack, &item.name);
            let spec = request_to_spec(&item.name, &display_name, request)?;
            let spec = ensure_unique_id(spec, seen_ids);
            seen_ids.insert(spec.id.clone());
            out.push(spec);
        }
        if !item.item.is_empty() {
            folder_stack.push(item.name.clone());
            collect_items(&item.item, collection_name, folder_stack, out, seen_ids)?;
            folder_stack.pop();
        }
    }
    Ok(())
}

fn build_display_name(collection_name: &str, folders: &[String], leaf: &str) -> String {
    let mut parts = vec![collection_name.to_owned()];
    parts.extend(folders.iter().cloned());
    parts.push(leaf.to_owned());
    parts.join(" / ")
}

fn request_to_spec(
    item_name: &str,
    display_name: &str,
    request: &Request,
) -> Result<ApiSpec, PostmanParseError> {
    let method = request
        .method
        .as_deref()
        .unwrap_or("GET")
        .trim()
        .to_string();
    let url_field = request.url.as_ref().ok_or(PostmanParseError::MissingUrl {
        name: item_name.to_owned(),
    })?;
    let (path, query_pairs, path_vars) = resolve_url(url_field)?;

    let mut params = Vec::new();
    for var in path_vars {
        if var.disabled {
            continue;
        }
        params.push(ApiParam {
            name: var.key,
            location: ParamLocation::Path,
            required: true,
            schema: None,
        });
    }
    for q in query_pairs {
        if q.disabled || q.key.trim().is_empty() {
            continue;
        }
        params.push(ApiParam {
            name: q.key,
            location: ParamLocation::Query,
            required: false,
            schema: value_schema(q.value.as_deref()),
        });
    }
    for h in &request.header {
        if h.disabled || h.key.trim().is_empty() {
            continue;
        }
        params.push(ApiParam {
            name: h.key.clone(),
            location: ParamLocation::Header,
            required: false,
            schema: value_schema(h.value.as_deref()),
        });
    }
    if let Some(body) = &request.body {
        params.extend(body_params(body)?);
    }

    params.sort_by(|a, b| a.name.cmp(&b.name));

    let id = make_spec_id(&method, &path, item_name);

    Ok(ApiSpec {
        id,
        name: display_name.to_owned(),
        method,
        path,
        version: None,
        params,
        response_schema: None,
        auth_scheme_ref: None,
    })
}

fn resolve_url(
    url: &UrlField,
) -> Result<(String, Vec<KeyValue>, Vec<KeyValue>), PostmanParseError> {
    match url {
        UrlField::Text(text) => {
            let (path, query) = split_path_and_query(text);
            let path = normalize_path(&path);
            Ok((path, query, Vec::new()))
        }
        UrlField::Object(obj) => {
            let mut path_vars = obj.variable.clone();
            let mut query = obj.query.clone();
            let path = if !obj.path.is_empty() {
                let joined = obj
                    .path
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("/");
                normalize_path(&format!("/{joined}"))
            } else if !obj.host.is_empty() {
                let host = obj.host.join(".");
                let joined = obj
                    .path
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("/");
                let path_part = if joined.is_empty() {
                    format!("/{host}")
                } else {
                    format!("/{host}/{joined}")
                };
                normalize_path(&path_part)
            } else if let Some(raw) = obj.raw.as_deref() {
                let (p, q) = split_path_and_query(raw);
                query.extend(q);
                normalize_path(&p)
            } else {
                normalize_path("/")
            };
            if path_vars.is_empty() {
                for segment in &obj.path {
                    if let Some(name) = segment.strip_prefix(':') {
                        path_vars.push(KeyValue {
                            key: name.to_owned(),
                            value: None,
                            disabled: false,
                        });
                    } else if let Some(name) = segment
                        .strip_prefix("{{")
                        .and_then(|s| s.strip_suffix("}}"))
                    {
                        path_vars.push(KeyValue {
                            key: name.to_owned(),
                            value: None,
                            disabled: false,
                        });
                    }
                }
            }
            Ok((path, query, path_vars))
        }
    }
}

fn split_path_and_query(raw: &str) -> (String, Vec<KeyValue>) {
    let trimmed = raw.trim();
    let (base, query_str) = match trimmed.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (trimmed, None),
    };
    let path = strip_host_prefix(base);
    let query = query_str.map(parse_query_string).unwrap_or_default();
    (path, query)
}

fn strip_host_prefix(base: &str) -> String {
    let without_vars = base.replace("{{", "").replace("}}", "");
    if let Some(idx) = without_vars.find("://") {
        let after = &without_vars[idx + 3..];
        match after.find('/') {
            Some(i) => after[i..].to_owned(),
            None => "/".to_owned(),
        }
    } else if base.starts_with('/') {
        base.to_owned()
    } else {
        format!("/{base}")
    }
}

fn parse_query_string(q: &str) -> Vec<KeyValue> {
    q.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .map(|(k, v)| (k.to_owned(), Some(v.to_owned())))
                .unwrap_or((pair.to_owned(), None));
            KeyValue {
                key,
                value,
                disabled: false,
            }
        })
        .collect()
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    let trimmed = trimmed.replace("{{", "{").replace("}}", "}");
    let segments: Vec<String> = trimmed
        .split('/')
        .map(|segment| {
            if let Some(rest) = segment.strip_prefix(':') {
                format!("{{{rest}}}")
            } else {
                segment.to_owned()
            }
        })
        .collect();
    let mut joined = segments.join("/");
    if joined.is_empty() {
        return "/".to_owned();
    }
    if !joined.starts_with('/') {
        joined = format!("/{joined}");
    }
    joined
}

fn body_params(body: &Body) -> Result<Vec<ApiParam>, PostmanParseError> {
    let mode = body.mode.as_deref().unwrap_or("raw");
    let mut params = Vec::new();
    match mode {
        "urlencoded" => {
            for field in &body.url_encoded {
                if field.disabled || field.key.trim().is_empty() {
                    continue;
                }
                params.push(ApiParam {
                    name: field.key.clone(),
                    location: ParamLocation::Body,
                    required: false,
                    schema: value_schema(field.value.as_deref()),
                });
            }
        }
        "formdata" => {
            for field in &body.formdata {
                if field.disabled || field.key.trim().is_empty() {
                    continue;
                }
                params.push(ApiParam {
                    name: field.key.clone(),
                    location: ParamLocation::Body,
                    required: false,
                    schema: value_schema(field.value.as_deref()),
                });
            }
        }
        "raw" => {
            if let Some(raw) = body.raw.as_deref() {
                let trimmed = raw.trim();
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                        if let Some(obj) = value.as_object() {
                            for key in obj.keys() {
                                params.push(ApiParam {
                                    name: key.clone(),
                                    location: ParamLocation::Body,
                                    required: false,
                                    schema: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(params)
}

fn value_schema(value: Option<&str>) -> Option<Value> {
    value.map(|v| {
        serde_json::json!({
            "type": "string",
            "default": v
        })
    })
}

fn make_spec_id(method: &str, path: &str, item_name: &str) -> String {
    let method = method.trim().to_lowercase();
    let path_part = path
        .trim()
        .trim_start_matches('/')
        .replace('/', ".")
        .replace(['{', '}'], "");
    let path_part = if path_part.is_empty() {
        "root".to_owned()
    } else {
        path_part
    };
    let base = sanitize_id(&format!("{method}.{path_part}"));
    if base.is_empty() {
        sanitize_id(item_name)
    } else {
        base
    }
}

fn ensure_unique_id(mut spec: ApiSpec, seen: &BTreeSet<String>) -> ApiSpec {
    if !seen.contains(&spec.id) {
        return spec;
    }
    let suffix = sanitize_id(spec.name.rsplit(" / ").next().unwrap_or(&spec.name));
    let mut candidate = format!("{}.{}", spec.id, suffix);
    let mut n = 2_u32;
    while seen.contains(&candidate) {
        candidate = format!("{}.{}", spec.id, n);
        n += 1;
    }
    spec.id = candidate;
    spec
}

fn sanitize_id(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out.trim_matches('.').to_owned()
}

/// Canonical JSON lines (sorted by `id`) for snapshot tests.
pub fn api_specs_canonical_lines(specs: &[ApiSpec]) -> Result<String, model::CanonicalError> {
    let mut lines: Vec<String> = specs
        .iter()
        .map(|s| s.to_canonical_json())
        .collect::<Result<Vec<_>, _>>()?;
    lines.sort();
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn rejects_non_v21_schema() {
        let json = r#"{
          "info": {
            "name": "Legacy",
            "schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"
          },
          "item": []
        }"#;
        let err = parse_postman_collection_str(json).unwrap_err();
        assert!(matches!(err, PostmanParseError::UnsupportedSchema { .. }));
    }

    #[test]
    fn path_colon_segment_becomes_brace_param() {
        let path = normalize_path("/users/:id");
        assert_eq!(path, "/users/{id}");
    }

    #[test]
    fn canonical_lines_sorted() {
        let specs = vec![
            normalize_api_spec(ApiSpec {
                id: "b".into(),
                name: "B".into(),
                method: "GET".into(),
                path: "/b".into(),
                version: None,
                params: vec![],
                response_schema: None,
                auth_scheme_ref: None,
            }),
            normalize_api_spec(ApiSpec {
                id: "a".into(),
                name: "A".into(),
                method: "GET".into(),
                path: "/a".into(),
                version: None,
                params: vec![],
                response_schema: None,
                auth_scheme_ref: None,
            }),
        ];
        let lines = api_specs_canonical_lines(&specs).unwrap();
        assert!(lines.starts_with(r#"{"id":"a""#));
    }

    #[test]
    fn content_hash_stable_for_parse_output() {
        let json = include_str!("../tests/fixtures/postman/sample_v21.json");
        let specs = parse_postman_collection_str(json).unwrap();
        let canonical = specs
            .iter()
            .map(|s| s.to_canonical_json().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let hash = model::content_hash(&canonical).unwrap();
        assert_eq!(hash.len(), 64);
        let _ = model::to_canonical_json(&serde_json::json!({ "hash": hash })).unwrap();
    }
}
