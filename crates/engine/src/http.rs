//! HTTP requests for `http` nodes (and later poll/cleanup).
//!
//! Output: `{status, headers, body, timing}`. Header names are lowercase and repeated headers
//! are joined with `", "`. A JSON content type yields parsed JSON when the body parses, other
//! bodies are UTF-8 (lossy) strings and an empty body is `null`. Any status, 5xx included, is
//! a completed request: judging the status is the job of assertions. Redirects are not
//! followed so they can be asserted on.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use common::Clock;
use model::nodes::HttpRequest;
use model::{to_canonical_json, Node, NodeKind};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Method, Url};
use serde_json::{json, Value};

use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use crate::render::RenderError;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, thiserror::Error)]
#[error("cannot build HTTP client: {0}")]
pub struct HttpClientError(String);

pub struct HttpExecutor {
    client: reqwest::Client,
    clock: Arc<dyn Clock>,
    default_timeout: Duration,
}

impl HttpExecutor {
    pub fn new(clock: Arc<dyn Clock>) -> Result<Self, HttpClientError> {
        // Fails only when a provider is already installed, which is equally fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| HttpClientError(e.to_string()))?;
        Ok(Self {
            client,
            clock,
            default_timeout: DEFAULT_TIMEOUT,
        })
    }

    /// Timeout for requests that do not set `timeout_ms`.
    #[must_use]
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Renders and sends `request`. `pointer` is the JSON Pointer of the request inside its
    /// node (`""` for http nodes, `/request` for poll), used to locate template errors.
    pub async fn send(
        &self,
        request: &HttpRequest,
        ctx: &ExecContext,
        pointer: &str,
    ) -> Result<Value, ExecError> {
        let at = |mut e: RenderError| {
            e.pointer = format!("{pointer}{}", e.pointer);
            ExecError::from(e)
        };
        let vars = ctx.vars();
        let path = text(
            &vars
                .render_str(&request.path)
                .map_err(|e| at(on(e, "/path")))?,
        );
        let mut url = self.url(&path, ctx)?;
        if !request.query.is_empty() {
            let mut rendered = Vec::with_capacity(request.query.len());
            for (key, value) in &request.query {
                let value = vars
                    .render_str(value)
                    .map_err(|e| at(on(e, &format!("/query/{}", escape(key)))))?;
                rendered.push((key, text(&value)));
            }
            url.query_pairs_mut().extend_pairs(rendered);
        }

        let mut headers = HeaderMap::new();
        for (name, value) in &request.headers {
            let rendered = vars
                .render_str(value)
                .map_err(|e| at(on(e, &format!("/headers/{}", escape(name)))))?;
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| invalid(format!("header name '{name}': {e}")))?;
            let value = HeaderValue::from_str(&text(&rendered))
                .map_err(|e| invalid(format!("header '{name}' value: {e}")))?;
            headers.insert(header, value);
        }

        let method = Method::from_bytes(method_name(request).as_bytes())
            .map_err(|e| invalid(e.to_string()))?;
        let mut builder = self.client.request(method.clone(), url.clone());
        if let Some(body) = &request.body {
            let body = vars.render_value(body).map_err(|e| at(on(e, "/body")))?;
            builder = match body {
                Value::String(raw) if headers.contains_key(CONTENT_TYPE) => builder.body(raw),
                other => {
                    if !headers.contains_key(CONTENT_TYPE) {
                        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    }
                    builder.body(to_canonical_json(&other).unwrap_or_default())
                }
            };
        }
        let timeout = request
            .timeout_ms
            .map_or(self.default_timeout, Duration::from_millis);

        let started = self.clock.now();
        let mut location = url.clone();
        location.set_query(None);
        let target = json!({"method": method.as_str(), "url": location.as_str()});
        let failed = |e: reqwest::Error| transport_error(&e, timeout, target.clone());

        let response = builder
            .headers(headers)
            .timeout(timeout)
            .send()
            .await
            .map_err(failed)?;
        let status = response.status().as_u16();
        let headers = collect_headers(response.headers());
        let bytes = response.bytes().await.map_err(failed)?;
        let duration = self.clock.now().duration_since(started);

        Ok(json!({
            "status": status,
            "headers": headers,
            "body": parse_body(&headers, &bytes),
            "timing": {"duration_ms": u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)},
        }))
    }

    fn url(&self, path: &str, ctx: &ExecContext) -> Result<Url, ExecError> {
        let full = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            let base = ctx.env.base_url();
            if base.is_empty() {
                return Err(ExecError::new(
                    ExecErrorKind::Template,
                    format!(
                        "relative path '{path}' but environment '{}' has no base_url",
                        ctx.env.name()
                    ),
                ));
            }
            format!(
                "{}/{}",
                base.trim_end_matches('/'),
                path.trim_start_matches('/')
            )
        };
        Url::parse(&full).map_err(|e| invalid(format!("URL '{full}': {e}")))
    }
}

impl NodeExecutor for HttpExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            match &node.kind {
                NodeKind::Http(cfg) => self.send(&cfg.request(), &ctx, "").await,
                other => Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!("HTTP executor cannot run '{}' nodes", other.type_name()),
                )),
            }
        })
    }
}

fn on(mut e: RenderError, field: &str) -> RenderError {
    e.pointer = format!("{field}{}", e.pointer);
    e
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn invalid(message: String) -> ExecError {
    ExecError::new(ExecErrorKind::Template, message)
}

/// Text form of a rendered value used in URLs and headers.
fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => to_canonical_json(other).unwrap_or_default(),
    }
}

fn method_name(request: &HttpRequest) -> String {
    serde_json::to_value(request.method)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn transport_error(e: &reqwest::Error, timeout: Duration, target: Value) -> ExecError {
    if e.is_timeout() {
        let millis = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        let mut detail = target;
        detail["timeout_ms"] = json!(millis);
        ExecError::new(
            ExecErrorKind::Timeout,
            format!("request timed out after {millis} ms"),
        )
        .with_detail(detail)
    } else {
        let mut message = e.to_string();
        let mut source = std::error::Error::source(e);
        while let Some(inner) = source {
            message.push_str(": ");
            message.push_str(&inner.to_string());
            source = inner.source();
        }
        let mut detail = target;
        detail["connect"] = json!(e.is_connect());
        ExecError::new(ExecErrorKind::Transport, message).with_detail(detail)
    }
}

fn collect_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in headers {
        let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
        out.entry(name.as_str().to_owned())
            .and_modify(|v| {
                v.push_str(", ");
                v.push_str(&value);
            })
            .or_insert(value);
    }
    out
}

fn parse_body(headers: &BTreeMap<String, String>, bytes: &[u8]) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    let is_json = headers
        .get("content-type")
        .is_some_and(|ct| ct.to_ascii_lowercase().contains("json"));
    if is_json {
        if let Ok(value) = serde_json::from_slice(bytes) {
            return value;
        }
    }
    Value::String(String::from_utf8_lossy(bytes).into_owned())
}
