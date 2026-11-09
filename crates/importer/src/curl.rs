//! One-line cURL command → a single normalized [`ApiSpec`] (request restore + light schema inference).

use std::collections::BTreeMap;

use thiserror::Error;

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};
use crate::heuristic::{
    derive_api_id, extract_path_from_url as extract_path, header_param, infer_body_schema,
    infer_string_schema, parse_query_pairs,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CurlParseError {
    #[error("empty cURL command")]
    Empty,
    #[error("no URL found in cURL command")]
    MissingUrl,
    #[error("unexpected end of arguments while reading flag {flag}")]
    UnexpectedEof { flag: String },
    #[error("malformed header (expected Name: value)")]
    MalformedHeader,
    #[error("unsupported cURL option: {option}")]
    UnsupportedOption { option: String },
    #[error("invalid UTF-8 in argument")]
    InvalidUtf8,
}

/// Parse a single-line `curl …` command into one [`ApiSpec`].
///
/// `response_schema` is not available from cURL alone and stays `None` unless future heuristics apply.
pub fn parse_curl_command(line: &str) -> Result<ApiSpec, CurlParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(CurlParseError::Empty);
    }
    let request = parse_curl_tokens(tokenize(trimmed)?)?;
    Ok(normalize_api_spec(request.into_api_spec()))
}

#[derive(Debug, Default)]
struct ParsedRequest {
    method: Option<String>,
    url: Option<String>,
    headers: BTreeMap<String, String>,
    body: Option<String>,
    force_get: bool,
    head: bool,
}

impl ParsedRequest {
    fn effective_method(&self) -> String {
        if self.head {
            return "HEAD".to_string();
        }
        if let Some(m) = &self.method {
            return m.to_uppercase();
        }
        if self.force_get {
            return "GET".to_string();
        }
        if self.body.is_some() {
            return "POST".to_string();
        }
        "GET".to_string()
    }

    fn into_api_spec(self) -> ApiSpec {
        let method = self.effective_method();
        let url = self.url.unwrap_or_else(|| "/".to_string());
        let path = extract_path(&url);
        let query_pairs = query_pairs_from_url_line(&url);
        let id = derive_api_id(&method, &path);
        let name = format!("{method} {path}");

        let mut params: Vec<ApiParam> = Vec::new();
        for (name, value) in query_pairs {
            params.push(ApiParam {
                name,
                location: ParamLocation::Query,
                required: true,
                schema: Some(infer_string_schema(&value)),
            });
        }

        if self.force_get {
            if let Some(data) = &self.body {
                for (name, value) in parse_form_pairs(data) {
                    params.push(ApiParam {
                        name,
                        location: ParamLocation::Query,
                        required: true,
                        schema: Some(infer_string_schema(&value)),
                    });
                }
            }
        } else if let Some(data) = self.body {
            let content_type = self
                .headers
                .get("content-type")
                .map(|s| s.to_ascii_lowercase());
            if matches!(
                content_type.as_deref(),
                Some(ct) if ct.contains("application/x-www-form-urlencoded")
            ) || looks_like_form_urlencoded(&data)
            {
                for (name, value) in parse_form_pairs(&data) {
                    params.push(ApiParam {
                        name,
                        location: ParamLocation::Query,
                        required: true,
                        schema: Some(infer_string_schema(&value)),
                    });
                }
            } else {
                params.push(ApiParam {
                    name: "body".to_string(),
                    location: ParamLocation::Body,
                    required: true,
                    schema: Some(infer_body_schema(&data)),
                });
            }
        }

        for (header_name, header_value) in self.headers {
            let mut param = header_param(header_name, &header_value);
            param.required = true;
            params.push(param);
        }

        ApiSpec {
            id,
            name,
            method,
            path,
            version: None,
            params,
            response_schema: None,
            auth_scheme_ref: None,
        }
    }
}

fn parse_curl_tokens(tokens: Vec<String>) -> Result<ParsedRequest, CurlParseError> {
    let mut req = ParsedRequest::default();
    let mut i = 0usize;
    if tokens
        .first()
        .is_some_and(|t| t.eq_ignore_ascii_case("curl"))
    {
        i = 1;
    }

    while i < tokens.len() {
        let token = tokens[i].as_str();
        i += 1;

        if is_url_token(token) {
            req.url = Some(unquote_url(token).to_string());
            continue;
        }

        if token == "-G" || token == "--get" {
            req.force_get = true;
            continue;
        }

        if token == "-I" || token == "--head" {
            req.head = true;
            continue;
        }

        if let Some(method) = token.strip_prefix("-X") {
            if method.is_empty() {
                let value = next_arg(&tokens, &mut i, "-X")?;
                req.method = Some(value);
            } else {
                req.method = Some(method.to_string());
            }
            continue;
        }

        if token == "-X" || token == "--request" {
            let value = next_arg(&tokens, &mut i, token)?;
            req.method = Some(value);
            continue;
        }

        if let Some(data) = token.strip_prefix("-d") {
            let value = if data.is_empty() {
                next_arg(&tokens, &mut i, "-d")?
            } else {
                data.to_string()
            };
            assign_body(&mut req, value, "-d")?;
            continue;
        }

        if matches!(
            token,
            "-d" | "--data" | "--data-raw" | "--data-ascii" | "--data-binary" | "--data-urlencode"
        ) {
            let value = next_arg(&tokens, &mut i, token)?;
            assign_body(&mut req, value, token)?;
            continue;
        }

        if let Some(header) = token.strip_prefix("-H") {
            if header.is_empty() {
                let value = next_arg(&tokens, &mut i, "-H")?;
                insert_header(&mut req.headers, &value)?;
            } else {
                insert_header(&mut req.headers, header)?;
            }
            continue;
        }

        if token == "-H" || token == "--header" {
            let value = next_arg(&tokens, &mut i, token)?;
            insert_header(&mut req.headers, &value)?;
            continue;
        }

        if token == "--url" {
            let value = next_arg(&tokens, &mut i, "--url")?;
            req.url = Some(value);
            continue;
        }

        if token == "-u" || token == "--user" {
            let _credentials = next_arg(&tokens, &mut i, token)?;
            req.headers
                .insert("authorization".to_string(), "Basic ****".to_string());
            continue;
        }

        if is_ignored_flag(token) {
            if consumes_next_arg(token) {
                let _ = next_arg(&tokens, &mut i, token)?;
            }
            continue;
        }

        if token.starts_with('-') {
            if token.len() > 2 && !token.starts_with("--") {
                for ch in token.chars().skip(1) {
                    let short = format!("-{ch}");
                    if short == "-X" || short == "-d" || short == "-H" {
                        return Err(CurlParseError::UnsupportedOption {
                            option: token.to_string(),
                        });
                    }
                }
            }
            continue;
        }
    }

    if req.url.is_none() {
        return Err(CurlParseError::MissingUrl);
    }
    Ok(req)
}

fn assign_body(req: &mut ParsedRequest, value: String, flag: &str) -> Result<(), CurlParseError> {
    if value.starts_with('@') {
        return Err(CurlParseError::UnsupportedOption {
            option: format!("{flag} @file"),
        });
    }
    req.body = Some(merge_body(req.body.take(), value));
    Ok(())
}

fn merge_body(existing: Option<String>, next: String) -> String {
    match existing {
        Some(prev) => format!("{prev}{next}"),
        None => next,
    }
}

fn next_arg(tokens: &[String], index: &mut usize, flag: &str) -> Result<String, CurlParseError> {
    let value = tokens
        .get(*index)
        .ok_or_else(|| CurlParseError::UnexpectedEof {
            flag: flag.to_string(),
        })?;
    *index += 1;
    Ok(value.clone())
}

fn insert_header(headers: &mut BTreeMap<String, String>, raw: &str) -> Result<(), CurlParseError> {
    let (name, value) = raw.split_once(':').ok_or(CurlParseError::MalformedHeader)?;
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(CurlParseError::MalformedHeader);
    }
    headers.insert(name, value.trim().to_string());
    Ok(())
}

fn is_url_token(token: &str) -> bool {
    let t = unquote_url(token);
    t.starts_with("http://")
        || t.starts_with("https://")
        || (t.starts_with('/') && !t.starts_with("-"))
}

fn unquote_url(token: &str) -> &str {
    token.trim_matches(|c| c == '\'' || c == '"')
}

fn is_ignored_flag(token: &str) -> bool {
    matches!(
        token,
        "-s" | "-S"
            | "-f"
            | "-k"
            | "-L"
            | "-v"
            | "--silent"
            | "--show-error"
            | "--fail"
            | "--insecure"
            | "--location"
            | "--verbose"
            | "--compressed"
            | "-o"
            | "--output"
            | "--connect-timeout"
            | "--max-time"
            | "-m"
            | "-b"
            | "--cookie"
            | "-c"
            | "--cookie-jar"
            | "-A"
            | "--user-agent"
            | "-e"
            | "--referer"
    )
}

fn consumes_next_arg(token: &str) -> bool {
    matches!(
        token,
        "-o" | "--output"
            | "--connect-timeout"
            | "--max-time"
            | "-m"
            | "-b"
            | "--cookie"
            | "-c"
            | "--cookie-jar"
            | "-A"
            | "--user-agent"
            | "-e"
            | "--referer"
    )
}

fn query_pairs_from_url_line(url: &str) -> Vec<(String, String)> {
    let bare = unquote_url(url);
    bare.split_once('?')
        .map(|(_, q)| parse_query_pairs(q))
        .unwrap_or_default()
}

fn parse_form_pairs(data: &str) -> Vec<(String, String)> {
    parse_query_pairs(data)
}

fn looks_like_form_urlencoded(data: &str) -> bool {
    !data.trim_start().starts_with('{') && data.contains('=') && !data.contains(':')
}

fn tokenize(input: &str) -> Result<Vec<String>, CurlParseError> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'\'' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'\'' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(CurlParseError::UnexpectedEof {
                    flag: "quoted string".to_string(),
                });
            }
            let piece =
                std::str::from_utf8(&bytes[start..i]).map_err(|_| CurlParseError::InvalidUtf8)?;
            tokens.push(piece.to_string());
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            i += 1;
            let start = i;
            let mut out = String::new();
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    if let Ok(s) = std::str::from_utf8(&bytes[i - 1..i]) {
                        out.push_str(s);
                    }
                    continue;
                }
                if bytes[i] == b'"' {
                    break;
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            if i >= bytes.len() {
                return Err(CurlParseError::UnexpectedEof {
                    flag: "quoted string".to_string(),
                });
            }
            let _ = start;
            tokens.push(out);
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let piece =
            std::str::from_utf8(&bytes[start..i]).map_err(|_| CurlParseError::InvalidUtf8)?;
        tokens.push(piece.to_string());
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::api_spec::ParamLocation;

    struct Case {
        name: &'static str,
        input: &'static str,
        method: &'static str,
        path: &'static str,
        check: fn(&ApiSpec),
    }

    fn param<'a>(spec: &'a ApiSpec, location: ParamLocation, name: &str) -> Option<&'a ApiParam> {
        spec.params
            .iter()
            .find(|p| p.location == location && p.name == name)
    }

    #[test]
    fn table_driven_curl_examples() {
        let cases: &[Case] = &[
            Case {
                name: "simple_get_quoted_url",
                input: "curl 'https://api.example.com/v1/ping'",
                method: "GET",
                path: "/v1/ping",
                check: |spec| {
                    assert_eq!(spec.id, "v1.ping.get");
                    assert!(spec.response_schema.is_none());
                },
            },
            Case {
                name: "get_double_quoted_url",
                input: r#"curl "https://example.com/health""#,
                method: "GET",
                path: "/health",
                check: |spec| assert_eq!(spec.id, "health.get"),
            },
            Case {
                name: "explicit_post_x",
                input: "curl -X POST https://example.com/users -d '{\"name\":\"ada\"}'",
                method: "POST",
                path: "/users",
                check: |spec| {
                    let body = param(spec, ParamLocation::Body, "body").expect("body param");
                    assert_eq!(body.schema.as_ref().unwrap()["type"], "object");
                    assert_eq!(
                        body.schema.as_ref().unwrap()["properties"]["name"]["type"],
                        "string"
                    );
                },
            },
            Case {
                name: "default_post_with_d",
                input: "curl https://example.com/orders -d '{}'",
                method: "POST",
                path: "/orders",
                check: |spec| {
                    assert!(param(spec, ParamLocation::Body, "body").is_some());
                },
            },
            Case {
                name: "header_h",
                input: "curl -H 'Content-Type: application/json' https://example.com/items",
                method: "GET",
                path: "/items",
                check: |spec| {
                    let h = param(spec, ParamLocation::Header, "content-type").unwrap();
                    assert_eq!(h.schema.as_ref().unwrap()["const"], "application/json");
                },
            },
            Case {
                name: "data_raw_flag",
                input: "curl --data-raw '{\"active\":true}' https://example.com/flags",
                method: "POST",
                path: "/flags",
                check: |spec| {
                    let body = param(spec, ParamLocation::Body, "body").unwrap();
                    assert_eq!(
                        body.schema.as_ref().unwrap()["properties"]["active"]["type"],
                        "boolean"
                    );
                },
            },
            Case {
                name: "query_in_url",
                input: "curl 'https://example.com/search?q=rust&page=2'",
                method: "GET",
                path: "/search",
                check: |spec| {
                    let q = param(spec, ParamLocation::Query, "q").unwrap();
                    assert_eq!(q.schema.as_ref().unwrap()["type"], "string");
                    let page = param(spec, ParamLocation::Query, "page").unwrap();
                    assert_eq!(page.schema.as_ref().unwrap()["type"], "integer");
                },
            },
            Case {
                name: "get_with_g_and_data",
                input: "curl -G https://example.com/filter -d 'status=open'",
                method: "GET",
                path: "/filter",
                check: |spec| {
                    assert!(param(spec, ParamLocation::Body, "body").is_none());
                    assert!(param(spec, ParamLocation::Query, "status").is_some());
                },
            },
            Case {
                name: "request_long_flag",
                input: "curl --request PUT https://example.com/resources/1",
                method: "PUT",
                path: "/resources/1",
                check: |spec| assert_eq!(spec.id, "resources.1.put"),
            },
            Case {
                name: "glued_xpost",
                input: "curl -XPOST https://example.com/login --data-raw 'user=me'",
                method: "POST",
                path: "/login",
                check: |spec| assert_eq!(spec.method, "POST"),
            },
            Case {
                name: "sensitive_authorization_header",
                input: "curl -H 'Authorization: Bearer sekret' https://example.com/me",
                method: "GET",
                path: "/me",
                check: |spec| {
                    let h = param(spec, ParamLocation::Header, "authorization").unwrap();
                    assert!(h.schema.as_ref().unwrap().get("const").is_none());
                },
            },
            Case {
                name: "head_method",
                input: "curl -I https://example.com/status",
                method: "HEAD",
                path: "/status",
                check: |spec| assert_eq!(spec.method, "HEAD"),
            },
        ];

        assert!(cases.len() >= 10);
        for case in cases {
            let spec = parse_curl_command(case.input).unwrap_or_else(|e| {
                panic!("case {} failed: {e}", case.name);
            });
            assert_eq!(spec.method, case.method, "case {}", case.name);
            assert_eq!(spec.path, case.path, "case {}", case.name);
            (case.check)(&spec);
        }
    }

    #[test]
    fn errors_without_url() {
        assert_eq!(
            parse_curl_command("curl -d '{}'").unwrap_err(),
            CurlParseError::MissingUrl
        );
    }

    #[test]
    fn rejects_data_file() {
        let err = parse_curl_command("curl -d @payload.json https://example.com/x").unwrap_err();
        assert!(matches!(err, CurlParseError::UnsupportedOption { .. }));
    }
}
