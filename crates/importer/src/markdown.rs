//! Markdown → LLM extraction of [`ApiSpecDraft`] candidates (develop.md §4.10).

#[cfg(feature = "markdown")]
use llm_gateway::{redact_text, ChatMessage, ChatRequest, LlmProvider, MessageRole};
use store::{ApiSpecRecord, ApiSpecRepository, ApiSpecStatus};

use crate::api_spec::normalize_api_spec;
use crate::markdown_draft::{
    parse_api_spec_drafts_from_llm_content, ApiSpecDraft, MarkdownDraftParseError,
};
use crate::markdown_prompt::markdown_import_user_content;

/// LLM call purpose label for store logging.
pub const MARKDOWN_IMPORT_PURPOSE: &str = "import_markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownImportRequestOptions {
    pub model: String,
    #[cfg(feature = "markdown")]
    pub role: MessageRole,
}

impl Default for MarkdownImportRequestOptions {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".into(),
            #[cfg(feature = "markdown")]
            role: MessageRole::User,
        }
    }
}

/// Errors while extracting or persisting Markdown-derived drafts.
#[derive(Debug, thiserror::Error)]
pub enum MarkdownImportError {
    #[cfg(feature = "markdown")]
    #[error("LLM request failed: {0}")]
    Llm(#[from] llm_gateway::LlmError),
    #[error("no JSON array found in model output")]
    NoJsonFound,
    #[error("model returned non-API code output")]
    RejectedCodeOutput,
    #[error("invalid draft JSON: {0}")]
    InvalidDraft(#[from] serde_json::Error),
    #[error("draft confidence {0} is outside 0..=1")]
    InvalidConfidence(f64),
    #[error("persist draft: {0}")]
    Store(#[from] store::StoreError),
    #[error("parse drafts: {0}")]
    Parse(#[from] MarkdownDraftParseError),
}

/// Call the provider and parse [`ApiSpecDraft`] list from the completion.
#[cfg(feature = "markdown")]
pub async fn extract_api_spec_drafts_from_markdown<P: LlmProvider + ?Sized>(
    provider: &P,
    markdown: &str,
    options: &MarkdownImportRequestOptions,
) -> Result<Vec<ApiSpecDraft>, MarkdownImportError> {
    let request = build_markdown_import_request(markdown, options);
    let response = provider.chat(request).await?;
    parse_api_spec_drafts_from_llm_content(&response.content).map_err(Into::into)
}

/// Build a redacted chat request (used by cassette tests and live calls).
#[cfg(feature = "markdown")]
pub fn build_markdown_import_request(
    markdown: &str,
    options: &MarkdownImportRequestOptions,
) -> ChatRequest {
    let body = redact_text(&markdown_import_user_content(markdown));
    ChatRequest {
        model: options.model.clone(),
        messages: vec![ChatMessage {
            role: options.role,
            content: body,
        }],
    }
}

/// Persist each draft via [`ApiSpecRepository::save`] only (always `status=draft`).
pub fn persist_api_spec_drafts(
    repo: &ApiSpecRepository<'_>,
    drafts: &[ApiSpecDraft],
    created_by: &str,
) -> Result<Vec<ApiSpecRecord>, MarkdownImportError> {
    let mut out = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let input = normalize_api_spec(draft.spec.clone()).into_save_input();
        let record = repo.save(&input, created_by)?;
        if record.info.status != ApiSpecStatus::Draft {
            return Err(MarkdownImportError::Store(store::StoreError::Corrupt(
                "save must create draft status only".into(),
            )));
        }
        out.push(record);
    }
    Ok(out)
}
