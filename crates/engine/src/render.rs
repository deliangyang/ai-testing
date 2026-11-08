//! Rendering templates against upstream outputs and the environment.
//!
//! A string that is exactly one reference takes the referenced value with its JSON type. In a
//! mixed string, strings are inserted as-is and other values as canonical JSON.

use std::collections::BTreeMap;
use std::fmt;

use model::scope::template_view;
use model::template::Part;
use model::{to_canonical_json, Node, PathSeg, Template, TemplateError, VarRef};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::environment::Environment;
use crate::executor::{ExecError, ExecErrorKind};

/// Values visible to a node's templates.
#[derive(Debug, Clone, Copy)]
pub struct VarContext<'a> {
    pub env: &'a Environment,
    /// Outputs of the node's completed ancestors.
    pub outputs: &'a BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RenderFailure {
    InvalidTemplate {
        offset: usize,
        message: String,
    },
    /// Not an ancestor, or an ancestor without output.
    MissingNode {
        node: String,
    },
    /// `found` is the longest prefix of the reference that resolved.
    MissingPath {
        node: String,
        found: String,
    },
    MissingEnv {
        name: String,
    },
    /// `${auth.*}` is resolved by the auth scheme support (P3-11).
    AuthUnavailable {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
pub struct RenderError {
    /// JSON Pointer of the string holding the template, relative to the rendered value.
    pub pointer: String,
    /// The reference that failed; absent when the template itself is malformed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(flatten)]
    pub failure: RenderFailure,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = if self.pointer.is_empty() {
            "/"
        } else {
            &self.pointer
        };
        write!(f, "{at}: ")?;
        if let Some(reference) = &self.reference {
            write!(f, "{reference}: ")?;
        }
        match &self.failure {
            RenderFailure::InvalidTemplate { offset, message } => {
                write!(f, "invalid template at char {offset}: {message}")
            }
            RenderFailure::MissingNode { node } => {
                write!(f, "no output from node '{node}' in scope")
            }
            RenderFailure::MissingPath { found, .. } => {
                write!(f, "path not found in output (resolved up to {found})")
            }
            RenderFailure::MissingEnv { name } => {
                write!(f, "environment variable '{name}' is not defined")
            }
            RenderFailure::AuthUnavailable { name } => {
                write!(f, "auth value '{name}' is not available")
            }
        }
    }
}

impl From<RenderError> for ExecError {
    fn from(err: RenderError) -> Self {
        let detail = serde_json::to_value(&err).unwrap_or(Value::Null);
        ExecError::new(ExecErrorKind::Template, err.to_string()).with_detail(detail)
    }
}

impl VarContext<'_> {
    /// Renders every string leaf of `value`.
    pub fn render_value(&self, value: &Value) -> Result<Value, RenderError> {
        self.render_at(value, &mut String::new())
    }

    pub fn render_str(&self, input: &str) -> Result<Value, RenderError> {
        self.render_string(input, "")
    }

    /// The node as JSON with its own-scope templates rendered: the node's
    /// `input_resolved_json`. Strings outside template positions are kept verbatim.
    pub fn resolve_node(&self, node: &Node) -> Result<Value, RenderError> {
        let view = template_view(node);
        let rendered = self.render_value(&view)?;
        let mut doc = serde_json::to_value(node).map_err(|e| RenderError {
            pointer: String::new(),
            reference: None,
            failure: RenderFailure::InvalidTemplate {
                offset: 0,
                message: e.to_string(),
            },
        })?;
        substitute(&mut doc, &view, &rendered, &mut String::new());
        Ok(doc)
    }

    fn render_at(&self, value: &Value, pointer: &mut String) -> Result<Value, RenderError> {
        Ok(match value {
            Value::String(s) => self.render_string(s, pointer)?,
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (i, item) in items.iter().enumerate() {
                    let len = pointer.len();
                    pointer.push_str(&format!("/{i}"));
                    out.push(self.render_at(item, pointer)?);
                    pointer.truncate(len);
                }
                Value::Array(out)
            }
            Value::Object(map) => {
                let mut out = Map::new();
                for (key, item) in map {
                    let len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
                    out.insert(key.clone(), self.render_at(item, pointer)?);
                    pointer.truncate(len);
                }
                Value::Object(out)
            }
            other => other.clone(),
        })
    }

    fn render_string(&self, input: &str, pointer: &str) -> Result<Value, RenderError> {
        let template = Template::parse(input).map_err(|e: TemplateError| RenderError {
            pointer: pointer.to_owned(),
            reference: None,
            failure: RenderFailure::InvalidTemplate {
                offset: e.offset,
                message: e.message,
            },
        })?;
        let fail = |var: &VarRef, failure| RenderError {
            pointer: pointer.to_owned(),
            reference: Some(var.to_string()),
            failure,
        };
        if let Some(var) = template.as_single_ref() {
            return self.lookup(var).map_err(|f| fail(var, f));
        }
        let mut out = String::new();
        for part in &template.parts {
            match part {
                Part::Literal(text) => out.push_str(text),
                Part::Ref(var) => match self.lookup(var).map_err(|f| fail(var, f))? {
                    Value::String(s) => out.push_str(&s),
                    other => out.push_str(&to_canonical_json(&other).unwrap_or_default()),
                },
            }
        }
        Ok(Value::String(out))
    }

    fn lookup(&self, var: &VarRef) -> Result<Value, RenderFailure> {
        match var {
            VarRef::Env { name } => self
                .env
                .var(name)
                .map(|v| Value::String(v.to_owned()))
                .ok_or_else(|| RenderFailure::MissingEnv { name: name.clone() }),
            VarRef::Auth { name } => Err(RenderFailure::AuthUnavailable { name: name.clone() }),
            VarRef::Node { node, path } => {
                let mut current = self
                    .outputs
                    .get(node)
                    .ok_or_else(|| RenderFailure::MissingNode { node: node.clone() })?;
                for (i, seg) in path.iter().enumerate() {
                    let next = match seg {
                        PathSeg::Key(k) => current.get(k.as_str()),
                        PathSeg::Index(n) => current.get(*n),
                    };
                    current = next.ok_or_else(|| RenderFailure::MissingPath {
                        node: node.clone(),
                        found: VarRef::Node {
                            node: node.clone(),
                            path: path[..i].to_vec(),
                        }
                        .to_string(),
                    })?;
                }
                Ok(current.clone())
            }
        }
    }
}

/// Replaces string leaves of `doc` that sit at a template position (a string leaf of `view`)
/// with the rendered value at the same pointer.
fn substitute(doc: &mut Value, view: &Value, rendered: &Value, pointer: &mut String) {
    match doc {
        Value::String(_) => {
            if let (Some(Value::String(_)), Some(new)) =
                (view.pointer(pointer), rendered.pointer(pointer))
            {
                *doc = new.clone();
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                let len = pointer.len();
                pointer.push_str(&format!("/{i}"));
                substitute(item, view, rendered, pointer);
                pointer.truncate(len);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
                substitute(item, view, rendered, pointer);
                pointer.truncate(len);
            }
        }
        _ => {}
    }
}
