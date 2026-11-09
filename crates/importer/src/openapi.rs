//! Deterministic OpenAPI 3.x and Swagger 2.0 → [`ApiSpec`] parsing (no LLM).
//!
//! Supported subset:
//! - Document roots: `openapi` 3.0.x / 3.1.x, `swagger` 2.0
//! - Paths object only; HTTP methods get/post/put/patch/delete/head/options
//! - Parameters: query, path, header; request body (OAS3) / body (Swagger 2) → [`ParamLocation::Body`]
//! - Response schema: status 200 → 201 → 204 → `default`; OAS3 prefers `application/json`, else lexicographically first content entry
//! - Local `$ref` only (`#/...` JSON Pointer into the same document), including nested refs on schemas
//! - Swagger `basePath` prefix; shared path-level parameters merged before operation parameters (operation wins on name+location)
//! - Global / operation `security` → first scheme name on [`ApiSpec::auth_scheme_ref`]

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};

const HTTP_METHODS: &[&str] = &["delete", "get", "head", "options", "patch", "post", "put"];

/// Errors while parsing OpenAPI / Swagger documents.
#[derive(Debug, thiserror::Error)]
pub enum OpenApiImportError {
    #[error("unsupported document: expected OpenAPI 3.x or Swagger 2.0 root fields")]
    UnknownFormat,
    #[error("unsupported OpenAPI version: {0}")]
    UnsupportedOpenApiVersion(String),
    #[error("unsupported Swagger version: {0}")]
    UnsupportedSwaggerVersion(String),
    #[error("external $ref is not supported: {0}")]
    ExternalRef(String),
    #[error("$ref target not found: {0}")]
    RefNotFound(String),
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid YAML: {0}")]
    InvalidYaml(String),
    #[error("{0}")]
    Other(String),
}

/// Parse OpenAPI / Swagger from a JSON string.
pub fn parse_openapi_json(input: &str) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    let root: Value = serde_json::from_str(input)?;
    parse_openapi_value(&root)
}

/// Parse OpenAPI / Swagger from a YAML string.
pub fn parse_openapi_yaml(input: &str) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    let root: Value =
        yaml_serde::from_str(input).map_err(|e| OpenApiImportError::InvalidYaml(e.to_string()))?;
    parse_openapi_value(&root)
}

/// Parse when the wire format is unknown: leading `{` → JSON, otherwise YAML.
pub fn parse_openapi(input: &str) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') {
        parse_openapi_json(input)
    } else {
        parse_openapi_yaml(input)
    }
}

/// Parse an already-deserialized JSON value (typically the full document root).
pub fn parse_openapi_value(root: &Value) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    if root.get("openapi").is_some() {
        parse_openapi3(root)
    } else if root.get("swagger").is_some() {
        parse_swagger2(root)
    } else {
        Err(OpenApiImportError::UnknownFormat)
    }
}

fn parse_openapi3(root: &Value) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    let version = root
        .get("openapi")
        .and_then(Value::as_str)
        .ok_or(OpenApiImportError::UnknownFormat)?;
    if !version.starts_with('3') {
        return Err(OpenApiImportError::UnsupportedOpenApiVersion(
            version.to_owned(),
        ));
    }

    let paths = root.get("paths").and_then(Value::as_object);
    let global_security = root.get("security");
    let mut specs = Vec::new();

    if let Some(paths) = paths {
        for path_key in paths.keys() {
            let path_item = &paths[path_key];
            let path_level_params = path_item.get("parameters").cloned().unwrap_or(Value::Null);
            if let Some(path_item_obj) = path_item.as_object() {
                for method in HTTP_METHODS {
                    let Some(operation) = path_item_obj.get(*method) else {
                        continue;
                    };
                    let mut spec = build_spec_oas3(
                        root,
                        path_key,
                        method,
                        operation,
                        &path_level_params,
                        operation.get("security").or(global_security),
                    )?;
                    spec = normalize_api_spec(spec);
                    specs.push(spec);
                }
            }
        }
    }

    specs.sort_by(|a, b| {
        (a.path.as_str(), a.method.as_str(), a.id.as_str()).cmp(&(
            b.path.as_str(),
            b.method.as_str(),
            b.id.as_str(),
        ))
    });
    Ok(specs)
}

fn parse_swagger2(root: &Value) -> Result<Vec<ApiSpec>, OpenApiImportError> {
    let version = root
        .get("swagger")
        .and_then(Value::as_str)
        .ok_or(OpenApiImportError::UnknownFormat)?;
    if version != "2.0" {
        return Err(OpenApiImportError::UnsupportedSwaggerVersion(
            version.to_owned(),
        ));
    }

    let base_path = root
        .get("basePath")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim_end_matches('/');

    let paths = root.get("paths").and_then(Value::as_object);
    let global_security = root.get("security");
    let mut specs = Vec::new();

    if let Some(paths) = paths {
        for path_key in paths.keys() {
            let path_item = &paths[path_key];
            let full_path = join_base_path(base_path, path_key);
            let path_level_params = path_item.get("parameters").cloned().unwrap_or(Value::Null);
            if let Some(path_item_obj) = path_item.as_object() {
                for method in HTTP_METHODS {
                    let Some(operation) = path_item_obj.get(*method) else {
                        continue;
                    };
                    let mut spec = build_spec_swagger2(
                        root,
                        &full_path,
                        method,
                        operation,
                        &path_level_params,
                        operation.get("security").or(global_security),
                    )?;
                    spec = normalize_api_spec(spec);
                    specs.push(spec);
                }
            }
        }
    }

    specs.sort_by(|a, b| {
        (a.path.as_str(), a.method.as_str(), a.id.as_str()).cmp(&(
            b.path.as_str(),
            b.method.as_str(),
            b.id.as_str(),
        ))
    });
    Ok(specs)
}

fn join_base_path(base: &str, path: &str) -> String {
    if base.is_empty() {
        return path.to_owned();
    }
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn build_spec_oas3(
    root: &Value,
    path: &str,
    method: &str,
    operation: &Value,
    path_level_params: &Value,
    security: Option<&Value>,
) -> Result<ApiSpec, OpenApiImportError> {
    let name = operation_display_name(operation, method, path);
    let id = operation_id(operation, method, path);
    let params = merge_parameters_oas3(root, path_level_params, operation)?;
    let response_schema = extract_response_schema_oas3(root, operation)?;
    let auth_scheme_ref = first_security_scheme(security);

    Ok(ApiSpec {
        id,
        name,
        method: method.to_ascii_uppercase(),
        path: path.to_owned(),
        version: None,
        params,
        response_schema,
        auth_scheme_ref,
    })
}

fn build_spec_swagger2(
    root: &Value,
    path: &str,
    method: &str,
    operation: &Value,
    path_level_params: &Value,
    security: Option<&Value>,
) -> Result<ApiSpec, OpenApiImportError> {
    let name = operation_display_name(operation, method, path);
    let id = operation_id(operation, method, path);
    let params = merge_parameters_swagger2(root, path_level_params, operation)?;
    let response_schema = extract_response_schema_swagger2(root, operation)?;
    let auth_scheme_ref = first_security_scheme(security);

    Ok(ApiSpec {
        id,
        name,
        method: method.to_ascii_uppercase(),
        path: path.to_owned(),
        version: None,
        params,
        response_schema,
        auth_scheme_ref,
    })
}

fn operation_display_name(operation: &Value, method: &str, path: &str) -> String {
    if let Some(summary) = operation.get("summary").and_then(Value::as_str) {
        return summary.trim().to_owned();
    }
    if let Some(operation_id) = operation.get("operationId").and_then(Value::as_str) {
        return operation_id.trim().to_owned();
    }
    format!("{} {}", method.to_ascii_uppercase(), path)
}

fn operation_id(operation: &Value, method: &str, path: &str) -> String {
    if let Some(id) = operation.get("operationId").and_then(Value::as_str) {
        return sanitize_operation_id(id);
    }
    let mut segment = path.trim();
    if segment.is_empty() || segment == "/" {
        return format!("root.{}", method.to_ascii_lowercase());
    }
    segment = segment.trim_start_matches('/');
    let dotted: String = segment
        .split('/')
        .map(|part| part.trim_matches(|c| c == '{' || c == '}'))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    if dotted.is_empty() {
        format!("root.{}", method.to_ascii_lowercase())
    } else {
        format!("{}.{}", dotted, method.to_ascii_lowercase())
    }
}

fn sanitize_operation_id(id: &str) -> String {
    id.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn first_security_scheme(security: Option<&Value>) -> Option<String> {
    let arr = security?.as_array()?;
    for entry in arr {
        let obj = entry.as_object()?;
        if let Some(name) = obj.keys().next() {
            return Some(name.clone());
        }
    }
    None
}

fn merge_parameters_oas3(
    root: &Value,
    path_level: &Value,
    operation: &Value,
) -> Result<Vec<ApiParam>, OpenApiImportError> {
    let mut by_key: BTreeMap<(ParamLocation, String), ApiParam> = BTreeMap::new();

    if let Some(list) = path_level.as_array() {
        for param in list {
            let resolved = resolve_value(root, param)?;
            if let Some(api_param) = param_from_oas3(root, &resolved)? {
                by_key.insert((api_param.location, api_param.name.clone()), api_param);
            }
        }
    }

    if let Some(list) = operation.get("parameters").and_then(Value::as_array) {
        for param in list {
            let resolved = resolve_value(root, param)?;
            if let Some(api_param) = param_from_oas3(root, &resolved)? {
                by_key.insert((api_param.location, api_param.name.clone()), api_param);
            }
        }
    }

    if let Some(body) = operation.get("requestBody") {
        let resolved = resolve_value(root, body)?;
        if let Some(body_param) = body_param_from_oas3(root, &resolved)? {
            by_key.insert((body_param.location, body_param.name.clone()), body_param);
        }
    }

    Ok(by_key.into_values().collect())
}

fn merge_parameters_swagger2(
    root: &Value,
    path_level: &Value,
    operation: &Value,
) -> Result<Vec<ApiParam>, OpenApiImportError> {
    let mut by_key: BTreeMap<(ParamLocation, String), ApiParam> = BTreeMap::new();

    let mut push_list = |list: &[Value]| -> Result<(), OpenApiImportError> {
        for param in list {
            let resolved = resolve_value(root, param)?;
            if let Some(api_param) = param_from_swagger2(root, &resolved)? {
                by_key.insert((api_param.location, api_param.name.clone()), api_param);
            }
        }
        Ok(())
    };

    if let Some(list) = path_level.as_array() {
        push_list(list)?;
    }
    if let Some(list) = operation.get("parameters").and_then(Value::as_array) {
        push_list(list)?;
    }

    Ok(by_key.into_values().collect())
}

fn param_from_oas3(root: &Value, param: &Value) -> Result<Option<ApiParam>, OpenApiImportError> {
    let obj = param
        .as_object()
        .ok_or_else(|| OpenApiImportError::Other("parameter must be an object".into()))?;
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if name.is_empty() {
        return Ok(None);
    }
    let location = match obj.get("in").and_then(Value::as_str) {
        Some("query") => ParamLocation::Query,
        Some("path") => ParamLocation::Path,
        Some("header") => ParamLocation::Header,
        Some("cookie") => return Ok(None),
        Some(other) => {
            return Err(OpenApiImportError::Other(format!(
                "unsupported parameter location: {other}"
            )));
        }
        None => return Ok(None),
    };
    let required = obj
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let schema = obj
        .get("schema")
        .map(|s| resolve_value(root, s))
        .transpose()?;
    Ok(Some(ApiParam {
        name,
        location,
        required,
        schema,
    }))
}

fn body_param_from_oas3(
    root: &Value,
    body: &Value,
) -> Result<Option<ApiParam>, OpenApiImportError> {
    let obj = body.as_object();
    let required = obj
        .and_then(|o| o.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content = obj
        .and_then(|o| o.get("content"))
        .and_then(Value::as_object);
    let Some(content) = content else {
        return Ok(None);
    };
    let schema = pick_content_schema(root, content)?;
    Ok(Some(ApiParam {
        name: "body".to_owned(),
        location: ParamLocation::Body,
        required,
        schema,
    }))
}

fn pick_content_schema(
    root: &Value,
    content: &Map<String, Value>,
) -> Result<Option<Value>, OpenApiImportError> {
    let media = if let Some(json) = content.get("application/json") {
        json
    } else {
        let first_key = content.keys().next();
        let Some(key) = first_key else {
            return Ok(None);
        };
        content
            .get(key)
            .ok_or_else(|| OpenApiImportError::Other("missing content entry".into()))?
    };
    let schema = media.get("schema");
    schema.map(|s| resolve_value(root, s)).transpose()
}

fn param_from_swagger2(
    root: &Value,
    param: &Value,
) -> Result<Option<ApiParam>, OpenApiImportError> {
    let obj = param
        .as_object()
        .ok_or_else(|| OpenApiImportError::Other("parameter must be an object".into()))?;
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if name.is_empty() {
        return Ok(None);
    }
    let location = match obj.get("in").and_then(Value::as_str) {
        Some("query") => ParamLocation::Query,
        Some("path") => ParamLocation::Path,
        Some("header") => ParamLocation::Header,
        Some("body") => ParamLocation::Body,
        Some("formData") => return Ok(None),
        Some(other) => {
            return Err(OpenApiImportError::Other(format!(
                "unsupported parameter location: {other}"
            )));
        }
        None => return Ok(None),
    };
    let required = obj
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let schema = swagger_param_schema(root, obj)?;
    Ok(Some(ApiParam {
        name: if location == ParamLocation::Body {
            "body".to_owned()
        } else {
            name
        },
        location,
        required,
        schema,
    }))
}

fn swagger_param_schema(
    root: &Value,
    param: &Map<String, Value>,
) -> Result<Option<Value>, OpenApiImportError> {
    if let Some(schema_ref) = param.get("schema") {
        return resolve_value(root, schema_ref).map(Some);
    }
    if param.get("in").and_then(Value::as_str) == Some("body") {
        return resolve_value(root, &Value::Object(param.clone())).map(Some);
    }
    let mut schema = Map::new();
    if let Some(t) = param.get("type") {
        schema.insert("type".to_owned(), t.clone());
    }
    if let Some(format) = param.get("format") {
        schema.insert("format".to_owned(), format.clone());
    }
    if let Some(items) = param.get("items") {
        let resolved = resolve_value(root, items)?;
        schema.insert("items".to_owned(), resolved);
    }
    if schema.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(schema)))
    }
}

fn extract_response_schema_oas3(
    root: &Value,
    operation: &Value,
) -> Result<Option<Value>, OpenApiImportError> {
    let responses = operation.get("responses").and_then(Value::as_object);
    let Some(responses) = responses else {
        return Ok(None);
    };
    let response = pick_response_entry(responses);
    let Some(response) = response else {
        return Ok(None);
    };
    let resolved = resolve_value(root, response)?;
    if let Some(content) = resolved.get("content").and_then(Value::as_object) {
        return pick_content_schema(root, content);
    }
    Ok(None)
}

fn extract_response_schema_swagger2(
    root: &Value,
    operation: &Value,
) -> Result<Option<Value>, OpenApiImportError> {
    let responses = operation.get("responses").and_then(Value::as_object);
    let Some(responses) = responses else {
        return Ok(None);
    };
    let response = pick_response_entry(responses);
    let Some(response) = response else {
        return Ok(None);
    };
    let resolved = resolve_value(root, response)?;
    if let Some(schema) = resolved.get("schema") {
        return resolve_value(root, schema).map(Some);
    }
    Ok(None)
}

fn pick_response_entry(responses: &Map<String, Value>) -> Option<&Value> {
    for code in ["200", "201", "204", "default"] {
        if let Some(r) = responses.get(code) {
            return Some(r);
        }
    }
    None
}

/// Resolve a JSON value, inlining same-document `$ref` (depth-first).
pub fn resolve_value(root: &Value, value: &Value) -> Result<Value, OpenApiImportError> {
    if let Some(ref_str) = value.get("$ref").and_then(Value::as_str) {
        if !ref_str.starts_with('#') {
            return Err(OpenApiImportError::ExternalRef(ref_str.to_owned()));
        }
        let pointer = ref_str.trim_start_matches('#');
        let target = root
            .pointer(pointer)
            .ok_or_else(|| OpenApiImportError::RefNotFound(ref_str.to_owned()))?;
        let mut resolved = resolve_value(root, target)?;
        if let Some(obj) = resolved.as_object_mut() {
            obj.remove("$ref");
        }
        return Ok(resolved);
    }

    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(root, v)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_value(root, item)?);
            }
            Ok(Value::Array(out))
        }
        _ => Ok(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn local_ref_resolves_nested_schema() {
        let root = json!({
            "components": {
                "schemas": {
                    "Pet": { "type": "object", "properties": { "name": { "type": "string" } } }
                }
            },
            "dummy": { "$ref": "#/components/schemas/Pet" }
        });
        let resolved = resolve_value(&root, &root["dummy"]).unwrap();
        assert_eq!(resolved["type"], "object");
    }

    #[test]
    fn operation_id_from_path_when_missing() {
        let op = json!({});
        assert_eq!(operation_id(&op, "get", "/pets/{petId}"), "pets.petId.get");
    }
}
