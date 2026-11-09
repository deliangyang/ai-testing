//! Markdown-derived API drafts (parse only; LLM extraction is optional).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api_spec::{normalize_api_spec, ApiSpec};

/// One AI-extracted API candidate before human review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiSpecDraft {
    pub spec: ApiSpec,
    /// Model-reported confidence in `0.0..=1.0`.
    pub confidence: f64,
    /// 1-based line numbers in the source Markdown that support this draft.
    pub source_lines: Vec<u32>,
}

/// Errors while parsing Markdown-derived drafts from model text.
#[derive(Debug, thiserror::Error)]
pub enum MarkdownDraftParseError {
    #[error("no JSON array found in model output")]
    NoJsonFound,
    #[error("model returned non-API code output")]
    RejectedCodeOutput,
    #[error("invalid draft JSON: {0}")]
    InvalidDraft(#[from] serde_json::Error),
    #[error("draft confidence {0} is outside 0..=1")]
    InvalidConfidence(f64),
}

/// Parse drafts from raw LLM text (plain JSON array or ```json fence).
pub fn parse_api_spec_drafts_from_llm_content(
    text: &str,
) -> Result<Vec<ApiSpecDraft>, MarkdownDraftParseError> {
    let json = extract_drafts_json(text)?;
    let raw: Vec<RawApiSpecDraft> = serde_json::from_str(&json)?;
    raw.into_iter().map(RawApiSpecDraft::into_draft).collect()
}

#[derive(Debug, Deserialize)]
struct RawApiSpecDraft {
    spec: ApiSpec,
    confidence: f64,
    source_lines: Vec<u32>,
}

impl RawApiSpecDraft {
    fn into_draft(self) -> Result<ApiSpecDraft, MarkdownDraftParseError> {
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(MarkdownDraftParseError::InvalidConfidence(self.confidence));
        }
        Ok(ApiSpecDraft {
            spec: normalize_api_spec(self.spec),
            confidence: self.confidence,
            source_lines: self.source_lines,
        })
    }
}

fn extract_drafts_json(text: &str) -> Result<String, MarkdownDraftParseError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(MarkdownDraftParseError::NoJsonFound);
    }

    if value_looks_like_drafts(trimmed) {
        return Ok(trimmed.to_owned());
    }

    let blocks = fenced_code_blocks(trimmed);
    if blocks.is_empty() {
        return extract_balanced_array(trimmed)
            .map(ToOwned::to_owned)
            .ok_or(MarkdownDraftParseError::NoJsonFound);
    }

    let mut json_like: Vec<&FenceBlock> = blocks
        .iter()
        .filter(|b| block_may_hold_drafts_json(b))
        .collect();
    let code_only: Vec<&FenceBlock> = blocks
        .iter()
        .filter(|b| block_is_rejected_code(b))
        .collect();

    if json_like.is_empty() {
        if !code_only.is_empty() {
            return Err(MarkdownDraftParseError::RejectedCodeOutput);
        }
        return extract_balanced_array(trimmed)
            .map(ToOwned::to_owned)
            .ok_or(MarkdownDraftParseError::NoJsonFound);
    }

    json_like.sort_by_key(|b| block_preference_rank(b));
    for block in json_like {
        let body = block.content.trim();
        if value_looks_like_drafts(body) {
            return Ok(body.to_owned());
        }
        if let Some(slice) = extract_balanced_array(body) {
            return Ok(slice.to_owned());
        }
    }

    Err(MarkdownDraftParseError::NoJsonFound)
}

#[derive(Debug, Clone)]
struct FenceBlock {
    lang: Option<String>,
    content: String,
}

fn fenced_code_blocks(text: &str) -> Vec<FenceBlock> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("```") {
        let after_ticks = &rest[open + 3..];
        let (lang_line, content_start) = if let Some(nl) = after_ticks.find('\n') {
            let lang = after_ticks[..nl].trim();
            let lang = if lang.is_empty() {
                None
            } else {
                Some(lang.to_ascii_lowercase())
            };
            (lang, nl + 1)
        } else {
            (None, 0)
        };
        let body = &after_ticks[content_start..];
        if let Some(close) = body.find("```") {
            let content = body[..close].trim_end();
            out.push(FenceBlock {
                lang: lang_line,
                content: content.to_owned(),
            });
            rest = &body[close + 3..];
        } else {
            break;
        }
    }
    out
}

fn block_preference_rank(block: &FenceBlock) -> u8 {
    match block.lang.as_deref() {
        Some("json") | Some("jsonc") => 0,
        None | Some("") => 1,
        Some(_) if serde_json::from_str::<Value>(block.content.trim()).is_ok() => 2,
        _ => 3,
    }
}

fn block_may_hold_drafts_json(block: &FenceBlock) -> bool {
    if block_is_rejected_code(block) {
        return false;
    }
    let body = block.content.trim();
    value_looks_like_drafts(body)
        || extract_balanced_array(body).is_some()
        || block
            .lang
            .as_deref()
            .is_some_and(|l| matches!(l, "json" | "jsonc"))
        || block.lang.is_none()
}

fn block_is_rejected_code(block: &FenceBlock) -> bool {
    let Some(lang) = block.lang.as_deref() else {
        return false;
    };
    if matches!(lang, "json" | "jsonc") {
        return false;
    }
    if is_rejected_code_lang(lang) {
        let body = block.content.trim();
        return !value_looks_like_drafts(body) && extract_balanced_array(body).is_none();
    }
    false
}

fn is_rejected_code_lang(lang: &str) -> bool {
    matches!(
        lang,
        "rust"
            | "rs"
            | "python"
            | "py"
            | "javascript"
            | "js"
            | "typescript"
            | "ts"
            | "go"
            | "java"
            | "bash"
            | "sh"
    )
}

fn value_looks_like_drafts(s: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(s.trim()) else {
        return false;
    };
    let Some(arr) = value.as_array() else {
        return false;
    };
    arr.first().is_some_and(|item| {
        let Some(obj) = item.as_object() else {
            return false;
        };
        obj.contains_key("spec")
            && obj.contains_key("confidence")
            && obj.contains_key("source_lines")
    })
}

fn extract_balanced_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let slice = &text[start..];
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, c) in slice.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let end = i + c.len_utf8();
                    let candidate = &slice[..end];
                    if serde_json::from_str::<Value>(candidate).is_ok() {
                        return Some(candidate);
                    }
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SAMPLE_DRAFTS_JSON: &str = r#"[
  {
    "spec": {
      "id": "ping.get",
      "name": "Health",
      "method": "GET",
      "path": "/ping",
      "params": []
    },
    "confidence": 0.9,
    "source_lines": [3, 4]
  }
]"#;

    #[test]
    fn parses_plain_json_array() {
        let drafts = parse_api_spec_drafts_from_llm_content(SAMPLE_DRAFTS_JSON).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].spec.id, "ping.get");
    }
}
