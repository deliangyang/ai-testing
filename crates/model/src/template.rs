//! Variable reference templates (§4.1).
//!
//! Grammar inside a string:
//! - `${<node>.output<path>}`: upstream node output, `<path>` = (`.key` | `[index]` | `["key"]`)*
//! - `${env:NAME}`: environment variable
//! - `${auth.NAME}`: auth scheme value injected at runtime
//! - `$${` escapes a literal `${`
//!
//! Error offsets count Unicode scalar values (chars) from the start of the template.

use std::fmt;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Literal(String),
    Ref(VarRef),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VarRef {
    Node { node: String, path: Vec<PathSeg> },
    Env { name: String },
    Auth { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathSeg {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid template at char {offset}: {message}")]
pub struct TemplateError {
    pub offset: usize,
    pub message: String,
}

/// A reference found while walking a JSON value, with the JSON pointer of the string holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedRef {
    pub pointer: String,
    pub var: VarRef,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{pointer}: {source}")]
pub struct LocatedTemplateError {
    pub pointer: String,
    pub source: TemplateError,
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, TemplateError> {
        Parser::new(input).parse()
    }

    pub fn refs(&self) -> impl Iterator<Item = &VarRef> {
        self.parts.iter().filter_map(|p| match p {
            Part::Ref(r) => Some(r),
            Part::Literal(_) => None,
        })
    }

    /// `Some` when the whole template is exactly one reference, so the renderer can keep the
    /// referenced value's JSON type instead of stringifying it.
    pub fn as_single_ref(&self) -> Option<&VarRef> {
        match self.parts.as_slice() {
            [Part::Ref(r)] => Some(r),
            _ => None,
        }
    }
}

impl VarRef {
    pub fn node_id(&self) -> Option<&str> {
        match self {
            VarRef::Node { node, .. } => Some(node),
            _ => None,
        }
    }
}

impl fmt::Display for VarRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarRef::Env { name } => write!(f, "${{env:{name}}}"),
            VarRef::Auth { name } => write!(f, "${{auth.{name}}}"),
            VarRef::Node { node, path } => {
                write!(f, "${{{node}.output")?;
                for seg in path {
                    match seg {
                        PathSeg::Index(i) => write!(f, "[{i}]")?,
                        PathSeg::Key(k) if is_plain_key(k) => write!(f, ".{k}")?,
                        PathSeg::Key(k) => write!(f, "[{}]", quote(k))?,
                    }
                }
                f.write_str("}")
            }
        }
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for part in &self.parts {
            match part {
                Part::Literal(s) => f.write_str(&s.replace("${", "$${"))?,
                Part::Ref(r) => write!(f, "{r}")?,
            }
        }
        Ok(())
    }
}

/// Collects every reference in all string leaves of `value`, in document order.
pub fn refs_in_value(value: &Value) -> Result<Vec<LocatedRef>, LocatedTemplateError> {
    let mut out = Vec::new();
    walk(value, &mut String::new(), &mut out)?;
    Ok(out)
}

fn walk(
    value: &Value,
    pointer: &mut String,
    out: &mut Vec<LocatedRef>,
) -> Result<(), LocatedTemplateError> {
    match value {
        Value::String(s) => {
            let template = Template::parse(s).map_err(|source| LocatedTemplateError {
                pointer: pointer.clone(),
                source,
            })?;
            out.extend(template.refs().map(|var| LocatedRef {
                pointer: pointer.clone(),
                var: var.clone(),
            }));
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let len = pointer.len();
                pointer.push_str(&format!("/{i}"));
                walk(item, pointer, out)?;
                pointer.truncate(len);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
                walk(item, pointer, out)?;
                pointer.truncate(len);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_plain_key(k: &str) -> bool {
    !k.is_empty() && k.chars().all(is_ident_char)
}

fn quote(k: &str) -> String {
    format!("\"{}\"", k.replace('\\', "\\\\").replace('"', "\\\""))
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn err<T>(&self, offset: usize, message: impl Into<String>) -> Result<T, TemplateError> {
        Err(TemplateError {
            offset,
            message: message.into(),
        })
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        s.chars()
            .enumerate()
            .all(|(i, c)| self.chars.get(self.pos + i) == Some(&c))
    }

    fn parse(mut self) -> Result<Template, TemplateError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        while let Some(c) = self.peek() {
            if self.starts_with("$${") {
                literal.push_str("${");
                self.pos += 3;
            } else if self.starts_with("${") {
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                parts.push(Part::Ref(self.parse_ref()?));
            } else {
                literal.push(c);
                self.pos += 1;
            }
        }
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        Ok(Template { parts })
    }

    /// Parses `${ ... }` starting at `${`.
    fn parse_ref(&mut self) -> Result<VarRef, TemplateError> {
        let open = self.pos;
        self.pos += 2;
        let Some(close) = self.find_close() else {
            return self.err(open, "unterminated '${', missing '}'");
        };
        if close == self.pos {
            return self.err(open, "empty reference '${}'");
        }
        let var = if self.starts_with("env:") {
            self.pos += 4;
            let name = self.ident(close, "environment variable name", false)?;
            VarRef::Env { name }
        } else if self.starts_with("auth.") {
            self.pos += 5;
            let name = self.ident(close, "auth value name", true)?;
            VarRef::Auth { name }
        } else {
            let node = self.ident(close, "node id", true)?;
            if !self.starts_with(".output") {
                return self.err(
                    self.pos,
                    format!("expected '.output' after node id '{node}'"),
                );
            }
            self.pos += ".output".len();
            let path = self.path(close)?;
            VarRef::Node { node, path }
        };
        if self.pos != close {
            return self.err(self.pos, "unexpected characters before '}'");
        }
        self.pos = close + 1;
        Ok(var)
    }

    /// Index of the `}` closing the current reference, skipping braces inside quoted keys.
    fn find_close(&self) -> Option<usize> {
        let mut i = self.pos;
        let mut quote: Option<char> = None;
        while let Some(&c) = self.chars.get(i) {
            match (quote, c) {
                (Some(_), '\\') => i += 1,
                (Some(q), c) if c == q => quote = None,
                (None, '"' | '\'') => quote = Some(c),
                (None, '}') => return Some(i),
                (None, '$') if self.chars.get(i + 1) == Some(&'{') => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    fn ident(&mut self, end: usize, what: &str, allow_dash: bool) -> Result<String, TemplateError> {
        let start = self.pos;
        match self.peek() {
            Some(c) if self.pos < end && is_ident_start(c) => self.pos += 1,
            _ => return self.err(start, format!("expected {what}")),
        }
        while self.pos < end {
            match self.peek() {
                Some(c) if c.is_ascii_alphanumeric() || c == '_' || (allow_dash && c == '-') => {
                    self.pos += 1
                }
                _ => break,
            }
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }

    fn path(&mut self, end: usize) -> Result<Vec<PathSeg>, TemplateError> {
        let mut path = Vec::new();
        while self.pos < end {
            match self.peek() {
                Some('.') => {
                    self.pos += 1;
                    let start = self.pos;
                    while self.pos < end && self.peek().is_some_and(is_ident_char) {
                        self.pos += 1;
                    }
                    if start == self.pos {
                        return self.err(start, "expected field name after '.'");
                    }
                    path.push(PathSeg::Key(self.chars[start..self.pos].iter().collect()));
                }
                Some('[') => {
                    let open = self.pos;
                    self.pos += 1;
                    let seg = match self.peek() {
                        Some(q @ ('"' | '\'')) => {
                            self.pos += 1;
                            PathSeg::Key(self.quoted(q, open)?)
                        }
                        _ => PathSeg::Index(self.index(open)?),
                    };
                    if self.peek() != Some(']') {
                        return self.err(self.pos, "expected ']'");
                    }
                    self.pos += 1;
                    path.push(seg);
                }
                _ => return self.err(self.pos, "expected '.field' or '[index]' in output path"),
            }
        }
        Ok(path)
    }

    fn quoted(&mut self, q: char, open: usize) -> Result<String, TemplateError> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return self.err(open, "unterminated quoted key"),
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(c) => out.push(c),
                        None => return self.err(open, "unterminated quoted key"),
                    }
                }
                Some(c) if c == q => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(c) => out.push(c),
            }
            self.pos += 1;
        }
    }

    fn index(&mut self, open: usize) -> Result<usize, TemplateError> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let digits: String = self.chars[start..self.pos].iter().collect();
        if digits.is_empty() {
            return self.err(start, "expected array index or quoted key inside '[]'");
        }
        digits
            .parse()
            .or_else(|_| self.err(open, format!("array index '{digits}' is too large")))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(node: &str, path: &[PathSeg]) -> VarRef {
        VarRef::Node {
            node: node.into(),
            path: path.to_vec(),
        }
    }

    fn key(k: &str) -> PathSeg {
        PathSeg::Key(k.into())
    }

    #[test]
    fn parses_valid_templates() {
        use PathSeg::Index;
        let cases: Vec<(&str, Vec<Part>)> = vec![
            ("", vec![]),
            ("plain text", vec![Part::Literal("plain text".into())]),
            ("${a.output}", vec![Part::Ref(node("a", &[]))]),
            (
                "${login.output.body.token}",
                vec![Part::Ref(node("login", &[key("body"), key("token")]))],
            ),
            (
                "${create-order.output.body.items[0].sku}",
                vec![Part::Ref(node(
                    "create-order",
                    &[key("body"), key("items"), Index(0), key("sku")],
                ))],
            ),
            (
                "${a.output.list[12][3]}",
                vec![Part::Ref(node("a", &[key("list"), Index(12), Index(3)]))],
            ),
            (
                "${a.output[\"x.y\"]}",
                vec![Part::Ref(node("a", &[key("x.y")]))],
            ),
            ("${a.output['}']}", vec![Part::Ref(node("a", &[key("}")]))]),
            (
                "${a.output[\"q\\\"t\"]}",
                vec![Part::Ref(node("a", &[key("q\"t")]))],
            ),
            (
                "${env:BASE_URL}",
                vec![Part::Ref(VarRef::Env {
                    name: "BASE_URL".into(),
                })],
            ),
            (
                "${auth.bearerToken}",
                vec![Part::Ref(VarRef::Auth {
                    name: "bearerToken".into(),
                })],
            ),
            (
                "Bearer ${auth.token}",
                vec![
                    Part::Literal("Bearer ".into()),
                    Part::Ref(VarRef::Auth {
                        name: "token".into(),
                    }),
                ],
            ),
            (
                "/orders/${c.output.body.id}/pay",
                vec![
                    Part::Literal("/orders/".into()),
                    Part::Ref(node("c", &[key("body"), key("id")])),
                    Part::Literal("/pay".into()),
                ],
            ),
            (
                "${a.output}${b.output}",
                vec![Part::Ref(node("a", &[])), Part::Ref(node("b", &[]))],
            ),
            (
                "cost $${price}",
                vec![Part::Literal("cost ${price}".into())],
            ),
            (
                "$5 {not a ref}",
                vec![Part::Literal("$5 {not a ref}".into())],
            ),
            (
                "订单${a.output.body}",
                vec![
                    Part::Literal("订单".into()),
                    Part::Ref(node("a", &[key("body")])),
                ],
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                Template::parse(input).unwrap().parts,
                expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn rejects_invalid_templates_with_char_offset() {
        let cases = [
            ("${", 0, "unterminated"),
            ("ab${a.output", 2, "unterminated"),
            ("${}", 0, "empty reference"),
            ("${a}", 3, "expected '.output'"),
            ("${a.outputs}", 10, "expected '.field'"),
            ("${a.input.x}", 3, "expected '.output'"),
            ("${1a.output}", 2, "expected node id"),
            ("${a.output.}", 11, "expected field name"),
            ("${a.output[x]}", 11, "expected array index"),
            ("${a.output[0}", 12, "expected ']'"),
            ("${a.output['k}", 0, "unterminated"),
            ("${env:}", 6, "expected environment variable name"),
            ("${env:A-B}", 7, "unexpected characters"),
            ("${auth.}", 7, "expected auth value name"),
            ("订单${a}", 5, "expected '.output'"),
            ("${a.output ${b.output}}", 0, "unterminated"),
            ("${a.output[99999999999999999999999]}", 10, "too large"),
        ];
        for (input, offset, needle) in cases {
            let err = Template::parse(input).unwrap_err();
            assert_eq!(err.offset, offset, "input: {input}, err: {err}");
            assert!(err.message.contains(needle), "input: {input}, err: {err}");
        }
    }

    #[test]
    fn non_ascii_field_names_require_quoting() {
        assert!(Template::parse("${a.output.编号}").is_err());
        assert_eq!(
            Template::parse("${a.output[\"编号\"]}")
                .unwrap()
                .as_single_ref(),
            Some(&node("a", &[key("编号")]))
        );
    }

    #[test]
    fn single_ref_detection() {
        assert!(Template::parse("${a.output.x}")
            .unwrap()
            .as_single_ref()
            .is_some());
        assert!(Template::parse(" ${a.output.x}")
            .unwrap()
            .as_single_ref()
            .is_none());
        assert!(Template::parse("${a.output}${b.output}")
            .unwrap()
            .as_single_ref()
            .is_none());
    }

    #[test]
    fn display_round_trips() {
        for input in [
            "Bearer ${auth.token}",
            "/o/${c.output.body.items[0][\"a.b\"]}?x=${env:X}",
            "cost $${price} ${a.output}",
        ] {
            let template = Template::parse(input).unwrap();
            assert_eq!(template.to_string(), input);
            assert_eq!(Template::parse(&template.to_string()).unwrap(), template);
        }
    }

    #[test]
    fn collects_refs_from_json_value_with_pointers() {
        let body = json!({
            "user": "${login.output.body.id}",
            "items": [{"sku": "${cart.output.body.items[0].sku}"}, 3, null],
            "a/b": "x ${env:TOKEN} ${auth.k}",
            "static": "no refs"
        });
        let refs = refs_in_value(&body).unwrap();
        let summary: Vec<_> = refs
            .iter()
            .map(|r| (r.pointer.as_str(), r.var.to_string()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("/a~1b", "${env:TOKEN}".to_string()),
                ("/a~1b", "${auth.k}".to_string()),
                (
                    "/items/0/sku",
                    "${cart.output.body.items[0].sku}".to_string()
                ),
                ("/user", "${login.output.body.id}".to_string()),
            ]
        );
    }

    #[test]
    fn reports_pointer_of_invalid_template_in_value() {
        let err = refs_in_value(&json!({"h": ["ok", "${broken"]})).unwrap_err();
        assert_eq!(err.pointer, "/h/1");
        assert_eq!(err.source.offset, 0);
    }
}
