//! Environment configuration: `environments/<name>.yaml` (ADR-001).
//!
//! ```yaml
//! base_url: http://localhost:8080
//! variables:
//!   TENANT: acme
//!   API_KEY: ${env:AITEST_API_KEY}
//! ```
//!
//! Values may embed `${env:VAR}`, read from the process environment at load time. In DAG
//! templates `${env:NAME}` refers to the loaded environment's variables; `base_url` is also
//! available as `BASE_URL`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use model::scope::node_refs;
use model::template::Part;
use model::{Dag, Template, TemplateError, VarRef};
use serde::Deserialize;

/// Name under which `base_url` is exposed to `${env:...}` references.
pub const BASE_URL_VAR: &str = "BASE_URL";

/// Source of `${env:VAR}` values inside environment files.
pub trait ProcessEnv {
    fn var(&self, name: &str) -> Option<String>;
}

/// The real process environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnv;

impl ProcessEnv for SystemEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

impl ProcessEnv for BTreeMap<String, String> {
    fn var(&self, name: &str) -> Option<String> {
        self.get(name).cloned()
    }
}

/// A `${env:VAR}` in the environment file that the process environment does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingProcessVar {
    /// `base_url` or `variables.<NAME>`.
    pub field: String,
    pub var: String,
}

/// A `${env:NAME}` in the DAG that the environment does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndefinedVar {
    pub node: String,
    /// JSON Pointer inside the node.
    pub pointer: String,
    pub var: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EnvError {
    #[error("invalid environment name '{0}': use letters, digits, '-' and '_'")]
    InvalidName(String),
    #[error("environment '{name}' not found at {}; available: {}", path.display(), list(available))]
    NotFound {
        name: String,
        path: PathBuf,
        available: Vec<String>,
    },
    #[error("cannot read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("environment '{name}': {path}: {message}")]
    Parse {
        name: String,
        path: String,
        message: String,
    },
    #[error("environment '{name}': {field}: {source}")]
    InvalidTemplate {
        name: String,
        field: String,
        source: TemplateError,
    },
    #[error("environment '{name}': {field}: only ${{env:...}} references are allowed, found {reference}")]
    UnsupportedRef {
        name: String,
        field: String,
        reference: String,
    },
    #[error("environment '{name}': variables.{BASE_URL_VAR} conflicts with base_url")]
    BaseUrlConflict { name: String },
    #[error(
        "environment '{name}': process environment variables not set: {}",
        missing_process(missing)
    )]
    MissingProcessVars {
        name: String,
        missing: Vec<MissingProcessVar>,
    },
    #[error(
        "DAG uses variables that environment '{name}' does not define: {}",
        undefined_vars(undefined)
    )]
    UndefinedVars {
        name: String,
        undefined: Vec<UndefinedVar>,
    },
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".into()
    } else {
        items.join(", ")
    }
}

fn missing_process(missing: &[MissingProcessVar]) -> String {
    missing
        .iter()
        .map(|m| format!("{} (used by {})", m.var, m.field))
        .collect::<Vec<_>>()
        .join(", ")
}

fn undefined_vars(undefined: &[UndefinedVar]) -> String {
    undefined
        .iter()
        .map(|u| format!("{} (node '{}' at {})", u.var, u.node, u.pointer))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvironment {
    base_url: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
}

/// A loaded environment with every `${env:VAR}` substituted.
#[derive(Clone, PartialEq)]
pub struct Environment {
    name: String,
    variables: BTreeMap<String, String>,
    /// Variables whose value came from the process environment and may be secret.
    from_process: BTreeSet<String>,
}

impl Environment {
    /// Loads `<root>/environments/<name>.yaml`.
    pub fn load(root: &Path, name: &str, env: &dyn ProcessEnv) -> Result<Self, EnvError> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(EnvError::InvalidName(name.to_owned()));
        }
        let dir = root.join("environments");
        let path = dir.join(format!("{name}.yaml"));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(EnvError::NotFound {
                    name: name.to_owned(),
                    path,
                    available: available(&dir),
                })
            }
            Err(e) => {
                return Err(EnvError::Io {
                    path,
                    message: e.to_string(),
                })
            }
        };
        Self::parse(name, &text, env)
    }

    /// Parses an environment file's contents.
    pub fn parse(name: &str, yaml: &str, env: &dyn ProcessEnv) -> Result<Self, EnvError> {
        let de = yaml_serde::Deserializer::from_str(yaml);
        let raw: RawEnvironment =
            serde_path_to_error::deserialize(de).map_err(|e| EnvError::Parse {
                name: name.to_owned(),
                path: e.path().to_string(),
                message: e.inner().to_string(),
            })?;
        if raw.variables.contains_key(BASE_URL_VAR) {
            return Err(EnvError::BaseUrlConflict {
                name: name.to_owned(),
            });
        }

        let mut fields = vec![(BASE_URL_VAR.to_owned(), "base_url".to_owned(), raw.base_url)];
        fields.extend(
            raw.variables
                .into_iter()
                .map(|(k, v)| (k.clone(), format!("variables.{k}"), v)),
        );

        let mut variables = BTreeMap::new();
        let mut from_process = BTreeSet::new();
        let mut missing = Vec::new();
        for (key, field, value) in fields {
            let template = Template::parse(&value).map_err(|source| EnvError::InvalidTemplate {
                name: name.to_owned(),
                field: field.clone(),
                source,
            })?;
            let mut rendered = String::new();
            for part in &template.parts {
                match part {
                    Part::Literal(text) => rendered.push_str(text),
                    Part::Ref(VarRef::Env { name: var }) => {
                        from_process.insert(key.clone());
                        match env.var(var) {
                            Some(v) => rendered.push_str(&v),
                            None => missing.push(MissingProcessVar {
                                field: field.clone(),
                                var: var.clone(),
                            }),
                        }
                    }
                    Part::Ref(other) => {
                        return Err(EnvError::UnsupportedRef {
                            name: name.to_owned(),
                            field,
                            reference: other.to_string(),
                        })
                    }
                }
            }
            variables.insert(key, rendered);
        }
        if !missing.is_empty() {
            return Err(EnvError::MissingProcessVars {
                name: name.to_owned(),
                missing,
            });
        }
        Ok(Self {
            name: name.to_owned(),
            variables,
            from_process,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn base_url(&self) -> &str {
        self.var(BASE_URL_VAR).unwrap_or_default()
    }

    /// Value for `${env:NAME}`.
    pub fn var(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    /// All variables, `BASE_URL` included.
    pub fn variables(&self) -> &BTreeMap<String, String> {
        &self.variables
    }

    /// Whether the value was read from the process environment (so it must be masked in output).
    pub fn is_from_process(&self, name: &str) -> bool {
        self.from_process.contains(name)
    }

    /// Fails listing every `${env:NAME}` in the DAG's own-scope templates that this environment
    /// does not define. Malformed templates are the gate's concern and are ignored here.
    pub fn check_dag(&self, dag: &Dag) -> Result<(), EnvError> {
        let mut undefined = Vec::new();
        for node in &dag.nodes {
            let Ok(refs) = node_refs(node) else {
                continue;
            };
            for located in refs {
                if let VarRef::Env { name } = &located.var {
                    if !self.variables.contains_key(name) {
                        undefined.push(UndefinedVar {
                            node: node.id.clone(),
                            pointer: located.pointer,
                            var: name.clone(),
                        });
                    }
                }
            }
        }
        if undefined.is_empty() {
            Ok(())
        } else {
            Err(EnvError::UndefinedVars {
                name: self.name.clone(),
                undefined,
            })
        }
    }
}

impl fmt::Debug for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked: BTreeMap<&str, &str> = self
            .variables
            .iter()
            .map(|(k, v)| {
                let shown = if self.from_process.contains(k) {
                    "****"
                } else {
                    v.as_str()
                };
                (k.as_str(), shown)
            })
            .collect();
        f.debug_struct("Environment")
            .field("name", &self.name)
            .field("variables", &masked)
            .finish()
    }
}

fn available(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? != "yaml" {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_owned())
        })
        .collect();
    names.sort();
    names
}
