//! Cassette replay for Markdown import without depending on `llm-gateway`.

use std::path::Path;

use model::to_canonical_json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::markdown_draft::{
    parse_api_spec_drafts_from_llm_content, ApiSpecDraft, MarkdownDraftParseError,
};
use crate::markdown_prompt::markdown_import_user_content;

/// LLM call purpose label aligned with store logging.
pub const MARKDOWN_IMPORT_PURPOSE: &str = "import_markdown";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CassetteFile {
    version: u32,
    request: ChatRequest,
    response: ChatResponse,
}

#[derive(Debug, Error)]
pub enum MarkdownCassetteError {
    #[error("read markdown file: {0}")]
    Io(#[from] std::io::Error),
    #[error("cassette directory not found: {0}")]
    DirMissing(String),
    #[error("cassette miss for request key {key}")]
    Miss { key: String },
    #[error("invalid cassette at {path}: {message}")]
    Invalid { path: String, message: String },
    #[error("canonical JSON: {0}")]
    Canonical(#[from] model::CanonicalError),
    #[error("parse drafts: {0}")]
    Parse(#[from] MarkdownDraftParseError),
}

/// Build the chat request used to key cassettes (`model` must match fixture metadata).
#[must_use]
pub fn build_markdown_cassette_request(markdown: &str, model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: markdown_import_user_content(markdown),
        }],
    }
}

/// Extract drafts by replaying a pre-recorded cassette from `dir` (no live LLM).
pub fn extract_api_spec_drafts_from_markdown_cassette(
    markdown: &str,
    cassette_dir: &Path,
    model: &str,
) -> Result<Vec<ApiSpecDraft>, MarkdownCassetteError> {
    if !cassette_dir.is_dir() {
        return Err(MarkdownCassetteError::DirMissing(
            cassette_dir.display().to_string(),
        ));
    }
    let request = build_markdown_cassette_request(markdown, model);
    let key = chat_request_key(&request)?;
    let path = cassette_dir.join(format!("{key}.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            MarkdownCassetteError::Miss { key: key.clone() }
        } else {
            MarkdownCassetteError::Io(e)
        }
    })?;
    let cassette: CassetteFile =
        serde_json::from_str(&text).map_err(|e| MarkdownCassetteError::Invalid {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
    if cassette.version != 1 {
        return Err(MarkdownCassetteError::Invalid {
            path: path.display().to_string(),
            message: format!("unsupported version {}", cassette.version),
        });
    }
    let file_key = chat_request_key(&cassette.request)?;
    if file_key != key {
        return Err(MarkdownCassetteError::Invalid {
            path: path.display().to_string(),
            message: format!("cassette request hash {file_key} does not match {key}"),
        });
    }
    parse_api_spec_drafts_from_llm_content(&cassette.response.content).map_err(Into::into)
}

fn chat_request_key(request: &ChatRequest) -> Result<String, MarkdownCassetteError> {
    let canonical = to_canonical_json(request)?;
    Ok(sha256_hex(canonical.as_bytes()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
