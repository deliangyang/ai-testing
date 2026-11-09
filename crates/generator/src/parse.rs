//! Tolerant extraction of DAG JSON from LLM text (markdown fences, preamble/postamble).

use model::Dag;
use serde_json::Value;

use crate::error::GenerateError;

/// Parse a [`Dag`] from raw LLM completion text.
pub fn parse_dag_from_llm_content(text: &str) -> Result<Dag, GenerateError> {
    let json = extract_dag_json(text)?;
    Dag::from_json_str(&json).map_err(GenerateError::from)
}

/// Extract a JSON object string suitable for [`Dag::from_json_str`].
pub fn extract_dag_json(text: &str) -> Result<String, GenerateError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(GenerateError::NoJsonFound);
    }

    if value_looks_like_dag(trimmed) {
        return Ok(trimmed.to_owned());
    }

    let blocks = fenced_code_blocks(trimmed);
    if blocks.is_empty() {
        return extract_balanced_object(trimmed)
            .map(ToOwned::to_owned)
            .ok_or(GenerateError::NoJsonFound);
    }

    let mut json_like: Vec<&FenceBlock> = blocks
        .iter()
        .filter(|b| block_may_hold_dag_json(b))
        .collect();
    let code_only: Vec<&FenceBlock> = blocks
        .iter()
        .filter(|b| block_is_rejected_code(b))
        .collect();

    if json_like.is_empty() {
        if !code_only.is_empty() {
            return Err(GenerateError::RejectedCodeOutput);
        }
        return extract_balanced_object(trimmed)
            .map(ToOwned::to_owned)
            .ok_or(GenerateError::NoJsonFound);
    }

    json_like.sort_by_key(|b| block_preference_rank(b));
    for block in json_like {
        let body = block.content.trim();
        if value_looks_like_dag(body) {
            return Ok(body.to_owned());
        }
        if let Some(slice) = extract_balanced_object(body) {
            return Ok(slice.to_owned());
        }
    }

    Err(GenerateError::NoJsonFound)
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

fn block_may_hold_dag_json(block: &FenceBlock) -> bool {
    if block_is_rejected_code(block) {
        return false;
    }
    let body = block.content.trim();
    value_looks_like_dag(body)
        || extract_balanced_object(body).is_some()
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
        return !value_looks_like_dag(body) && extract_balanced_object(body).is_none();
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
            | "tsx"
            | "go"
            | "golang"
            | "java"
            | "kotlin"
            | "swift"
            | "csharp"
            | "c"
            | "cpp"
            | "c++"
            | "ruby"
            | "rb"
            | "php"
            | "bash"
            | "sh"
            | "shell"
            | "zsh"
            | "powershell"
            | "ps1"
    )
}

fn value_looks_like_dag(s: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(s.trim()) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.contains_key("id") && obj.contains_key("nodes")
}

fn extract_balanced_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
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
            '{' => depth += 1,
            '}' => {
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
    use model::ModelError;

    const MINIMAL_DAG: &str = r#"{"id":"serial","nodes":[{"id":"create","type":"http","method":"POST","path":"/orders"}],"edges":[]}"#;

    #[test]
    fn accepts_raw_json() {
        let dag = parse_dag_from_llm_content(MINIMAL_DAG).unwrap();
        assert_eq!(dag.id, "serial");
        assert_eq!(dag.nodes.len(), 1);
    }

    #[test]
    fn accepts_json_in_markdown_fence_with_preamble() {
        let text = format!("下面是候选 DAG：\n\n```json\n{MINIMAL_DAG}\n```\n\n请审阅。");
        let dag = parse_dag_from_llm_content(&text).unwrap();
        assert_eq!(dag.id, "serial");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_dag_from_llm_content(r#"{"id":"x","nodes":"not-an-array"}"#).unwrap_err();
        assert!(matches!(
            err,
            GenerateError::InvalidDag(ModelError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_pure_code_fence() {
        let text = "```rust\nfn main() {\n    println!(\"hi\");\n}\n```";
        let err = parse_dag_from_llm_content(text).unwrap_err();
        assert!(matches!(err, GenerateError::RejectedCodeOutput));
    }

    #[test]
    fn prefers_json_fence_over_prose() {
        let text = format!("noise {{ broken\n```json\n{MINIMAL_DAG}\n```");
        let json = extract_dag_json(&text).unwrap();
        assert_eq!(json, MINIMAL_DAG);
    }
}
