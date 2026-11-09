use std::sync::Arc;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::error::LlmError;
use crate::provider::{ChatFuture, LlmProvider};
use crate::redact::redact_chat_request;
use crate::types::{ChatRequest, ChatResponse, MessageRole, TokenUsage};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// Configuration for an OpenAI-compatible chat-completions client.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub base_url: String,
    pub model: String,
    /// Credential reference (`${env:...}` or `keychain://...`), never plaintext.
    pub api_key_ref: String,
    /// Pre-resolved key for integration tests (never log). Production callers leave this `None`.
    resolved_api_key: Option<String>,
}

impl OpenAiConfig {
    #[must_use]
    pub fn new(model: impl Into<String>, api_key_ref: impl Into<String>) -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            model: model.into(),
            api_key_ref: api_key_ref.into(),
            resolved_api_key: None,
        }
    }

    /// Wiremock/integration tests only: avoids mutating process environment under parallel `cargo test`.
    #[doc(hidden)]
    #[must_use]
    pub fn with_resolved_api_key_for_test(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key_ref: String::new(),
            resolved_api_key: Some(api_key.into()),
        }
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

pub struct OpenAiClient {
    client: reqwest::Client,
    config: OpenAiConfig,
}

impl OpenAiClient {
    pub fn new(config: OpenAiConfig) -> Result<Self, LlmError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| LlmError::Client(e.to_string()))?;
        Ok(Self { client, config })
    }

    #[must_use]
    pub fn config(&self) -> &OpenAiConfig {
        &self.config
    }

    async fn chat_impl(&self, request: ChatRequest) -> Result<ChatResponse, LlmError> {
        let request = redact_chat_request(request);
        let api_key = match &self.config.resolved_api_key {
            Some(key) => key.clone(),
            None => secrets::resolve_credential_ref(&self.config.api_key_ref)?,
        };
        let model = if request.model.is_empty() {
            self.config.model.clone()
        } else {
            request.model.clone()
        };
        let url = chat_completions_url(&self.config.base_url)?;
        let body = json!({
            "model": model,
            "messages": request
                .messages
                .iter()
                .map(|m| json!({
                    "role": role_str(m.role),
                    "content": m.content,
                }))
                .collect::<Vec<_>>(),
        });

        let response = self
            .client
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Transport(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| LlmError::Transport(e.to_string()))?;

        if !status.is_success() {
            return Err(api_error(status, &text));
        }

        parse_chat_response(&text, &model)
    }
}

impl LlmProvider for OpenAiClient {
    fn chat(&self, request: ChatRequest) -> ChatFuture {
        let this = Arc::new(self.clone_inner());
        Box::pin(async move { this.chat_impl(request).await })
    }
}

impl OpenAiClient {
    fn clone_inner(&self) -> Self {
        Self {
            client: self.client.clone(),
            config: self.config.clone(),
        }
    }
}

fn chat_completions_url(base_url: &str) -> Result<String, LlmError> {
    let base = base_url.trim_end_matches('/');
    Ok(format!("{base}/v1/chat/completions"))
}

fn role_str(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

fn api_error(status: StatusCode, body: &str) -> LlmError {
    let message = serde_json::from_str::<ApiErrorBody>(body)
        .ok()
        .and_then(|e| e.error.map(|inner| inner.message))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| truncate_body(body));
    LlmError::Api {
        status: status.as_u16(),
        message,
    }
}

fn truncate_body(body: &str) -> String {
    const MAX: usize = 512;
    if body.len() <= MAX {
        return body.to_owned();
    }
    format!("{}…", &body[..MAX])
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: Option<ApiErrorInner>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorInner {
    message: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    model: Option<String>,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

fn parse_chat_response(body: &str, request_model: &str) -> Result<ChatResponse, LlmError> {
    let parsed: OpenAiChatResponse =
        serde_json::from_str(body).map_err(|e| LlmError::Response(e.to_string()))?;
    let choice = parsed
        .choices
        .first()
        .ok_or_else(|| LlmError::Response("missing choices".into()))?;
    let content = choice
        .message
        .as_ref()
        .and_then(|m| m.content.clone())
        .unwrap_or_default();
    let usage = parsed.usage.unwrap_or(OpenAiUsage {
        prompt_tokens: Some(0),
        completion_tokens: Some(0),
        total_tokens: Some(0),
    });
    Ok(ChatResponse {
        content,
        model: parsed.model.unwrap_or_else(|| request_model.to_owned()),
        usage: TokenUsage {
            prompt_tokens: usage.prompt_tokens.unwrap_or(0),
            completion_tokens: usage.completion_tokens.unwrap_or(0),
            total_tokens: usage.total_tokens.unwrap_or(0),
        },
        finish_reason: choice.finish_reason.clone(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_success_body() {
        let body = r#"{
            "model": "gpt-4o-mini",
            "choices": [{"message": {"content": "hi"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 1, "total_tokens": 4}
        }"#;
        let resp = parse_chat_response(body, "fallback").unwrap();
        assert_eq!(resp.content, "hi");
        assert_eq!(resp.model, "gpt-4o-mini");
        assert_eq!(resp.usage.total_tokens, 4);
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    }
}
