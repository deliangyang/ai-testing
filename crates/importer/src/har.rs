//! HAR 1.x → deduplicated [`Vec<ApiSpec>`] by method + path (develop.md §4.10).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};
use crate::heuristic::{
    derive_api_id, extract_path_from_url, header_param, infer_body_schema, infer_json_schema,
    infer_string_schema, merge_params, parse_query_pairs, query_pairs_from_url,
};

#[derive(Debug, thiserror::Error)]
pub enum HarParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HAR root must be a JSON object")]
    NotObject,
    #[error("missing or invalid `log` object")]
    MissingLog,
    #[error("missing `log.entries` array")]
    MissingEntries,
}

#[derive(Debug, Deserialize)]
struct HarRoot {
    log: Log,
}

#[derive(Debug, Deserialize)]
struct Log {
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    request: Request,
    response: Response,
}

#[derive(Debug, Deserialize)]
struct Request {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<NameValue>,
    #[serde(default, rename = "queryString")]
    query_string: Vec<NameValue>,
    #[serde(default, rename = "postData")]
    post_data: Option<PostData>,
}

#[derive(Debug, Deserialize)]
struct NameValue {
    name: String,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostData {
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "params")]
    form_params: Vec<NameValue>,
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    status: i64,
    #[serde(default)]
    content: Content,
}

#[derive(Debug, Default, Deserialize)]
struct Content {
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// Parse a HAR JSON document into deduplicated [`ApiSpec`] rows (one per method + path).
pub fn parse_har_str(input: &str) -> Result<Vec<ApiSpec>, HarParseError> {
    let value: Value = serde_json::from_str(input)?;
    parse_har(&value)
}

/// Parse a HAR JSON value.
pub fn parse_har(value: &Value) -> Result<Vec<ApiSpec>, HarParseError> {
    if value.as_object().is_none() {
        return Err(HarParseError::NotObject);
    }
    let log = value
        .get("log")
        .and_then(Value::as_object)
        .ok_or(HarParseError::MissingLog)?;
    if log.get("entries").is_none() {
        return Err(HarParseError::MissingEntries);
    }
    let root: HarRoot = serde_json::from_value(value.clone())?;

    let mut by_endpoint: BTreeMap<(String, String), (ApiSpec, bool)> = BTreeMap::new();

    for entry in root.log.entries {
        let Some(partial) = entry_to_partial(&entry) else {
            continue;
        };
        let key = (partial.spec.method.clone(), partial.spec.path.clone());
        match by_endpoint.get_mut(&key) {
            None => {
                by_endpoint.insert(key, (partial.spec, partial.success_response));
            }
            Some((existing, success)) => {
                merge_params(&mut existing.params, partial.spec.params);
                if should_replace_response_schema(*success, partial.success_response) {
                    existing.response_schema = partial.spec.response_schema;
                    *success = partial.success_response;
                } else if existing.response_schema.is_none() {
                    existing.response_schema = partial.spec.response_schema;
                    *success = partial.success_response || *success;
                }
            }
        }
    }

    let mut specs: Vec<ApiSpec> = by_endpoint
        .into_values()
        .map(|(s, _)| normalize_api_spec(s))
        .collect();
    specs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(specs)
}

struct PartialSpec {
    spec: ApiSpec,
    success_response: bool,
}

fn entry_to_partial(entry: &Entry) -> Option<PartialSpec> {
    let method = entry.request.method.trim().to_uppercase();
    if method.is_empty() || method == "CONNECT" {
        return None;
    }
    let path = extract_path_from_url(&entry.request.url);
    let params = request_params(&entry.request);
    let response_schema = infer_response_schema(&entry.response);
    let has_schema = response_schema.is_some();
    let success = (200..300).contains(&entry.response.status);
    let id = derive_api_id(&method, &path);
    let name = format!("{method} {path}");

    Some(PartialSpec {
        spec: ApiSpec {
            id,
            name,
            method,
            path,
            version: None,
            params,
            response_schema,
            auth_scheme_ref: None,
        },
        success_response: success && has_schema,
    })
}

fn request_params(request: &Request) -> Vec<ApiParam> {
    let mut params = Vec::new();

    let mut query_keys: BTreeMap<String, Option<String>> = BTreeMap::new();
    for q in &request.query_string {
        let key = q.name.trim();
        if key.is_empty() {
            continue;
        }
        query_keys.insert(key.to_owned(), q.value.clone());
    }
    if query_keys.is_empty() {
        for (name, value) in query_pairs_from_url(&request.url) {
            query_keys.insert(name, Some(value));
        }
    }
    for (name, value) in query_keys {
        params.push(ApiParam {
            name,
            location: ParamLocation::Query,
            required: false,
            schema: value.as_deref().map(infer_string_schema),
        });
    }

    for h in &request.headers {
        let name = h.name.trim();
        if name.is_empty() || name.starts_with(':') {
            continue;
        }
        let value = h.value.as_deref().unwrap_or("");
        params.push(header_param(name.to_owned(), value));
    }

    if let Some(post) = &request.post_data {
        let mime = post.mime_type.as_deref().unwrap_or("").to_ascii_lowercase();
        if mime.contains("application/x-www-form-urlencoded") {
            for field in &post.form_params {
                if field.name.trim().is_empty() {
                    continue;
                }
                params.push(ApiParam {
                    name: field.name.clone(),
                    location: ParamLocation::Body,
                    required: false,
                    schema: field.value.as_deref().map(infer_string_schema),
                });
            }
            if let Some(text) = post.text.as_deref() {
                for (name, value) in parse_query_pairs(text) {
                    params.push(ApiParam {
                        name,
                        location: ParamLocation::Body,
                        required: false,
                        schema: Some(infer_string_schema(&value)),
                    });
                }
            }
        } else if let Some(text) = post.text.as_deref() {
            let trimmed = text.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            {
                params.push(ApiParam {
                    name: "body".to_string(),
                    location: ParamLocation::Body,
                    required: false,
                    schema: Some(infer_body_schema(trimmed)),
                });
            }
        }
    }

    params.sort_by(|a, b| a.name.cmp(&b.name));
    params
}

fn infer_response_schema(response: &Response) -> Option<Value> {
    let mime = response
        .content
        .mime_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let text = response.content.text.as_deref()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let looks_json = mime.contains("json") || trimmed.starts_with('{') || trimmed.starts_with('[');
    if !looks_json {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;
    Some(infer_json_schema(&value))
}

fn should_replace_response_schema(existing_success: bool, new_success: bool) -> bool {
    if new_success && !existing_success {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn deduplicates_same_method_and_path() {
        let har = r#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": {
                  "method": "GET",
                  "url": "https://example.com/v1/items?page=1",
                  "queryString": [{"name": "page", "value": "1"}]
                },
                "response": {
                  "status": 200,
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"items\":[{\"id\":1}]}"
                  }
                }
              },
              {
                "request": {
                  "method": "GET",
                  "url": "https://example.com/v1/items?page=2&limit=10",
                  "queryString": [
                    {"name": "page", "value": "2"},
                    {"name": "limit", "value": "10"}
                  ]
                },
                "response": {
                  "status": 200,
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"items\":[{\"id\":2,\"name\":\"b\"}]}"
                  }
                }
              }
            ]
          }
        }"#;
        let specs = parse_har_str(har).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].method, "GET");
        assert_eq!(specs[0].path, "/v1/items");
        let names: Vec<_> = specs[0].params.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"page"));
        assert!(names.contains(&"limit"));
        assert!(specs[0].response_schema.is_some());
    }

    #[test]
    fn rejects_missing_entries() {
        let har = r#"{"log": {"version": "1.2"}}"#;
        assert!(matches!(
            parse_har_str(har).unwrap_err(),
            HarParseError::MissingEntries
        ));
    }
}
