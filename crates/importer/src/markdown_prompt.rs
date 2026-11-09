//! Shared Markdown import prompt text (deterministic, no LLM).

pub(crate) const MARKDOWN_IMPORT_INSTRUCTIONS: &str = r#"你是 API 文档分析助手。从下面的 Markdown（带行号）中提取 HTTP 接口定义。

输出要求：
1. 只输出一个 JSON 数组，不要解释或其他文本。
2. 每个数组元素必须包含字段：spec、confidence、source_lines。
3. spec 与 ApiSpec 对齐：id、name、method、path；可选 params、response_schema、auth_scheme_ref。
4. confidence 为 0.0 到 1.0 的浮点数。
5. source_lines 为 1-based 行号数组，指向支持该接口的 Markdown 行。
6. params 元素含 name、location（query/path/header/body）、required、可选 schema。

Markdown："#;

/// Number lines for the model (1-based, fixed width).
pub fn number_markdown_lines(markdown: &str) -> String {
    markdown
        .lines()
        .enumerate()
        .map(|(idx, line)| format!("{:4}| {}", idx + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// User message body for markdown API extraction (before gateway redaction).
pub fn markdown_import_user_content(markdown: &str) -> String {
    let numbered = number_markdown_lines(markdown);
    format!("{MARKDOWN_IMPORT_INSTRUCTIONS}\n\n{numbered}")
}
