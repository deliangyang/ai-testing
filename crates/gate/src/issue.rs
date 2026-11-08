use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
}

/// One finding. `code` is a stable dotted identifier such as `structure.duplicate_node_id`;
/// `json_path` is a JSON Pointer into the DAG document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub code: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
    pub message: String,
}

impl Issue {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            node_id: None,
            json_path: None,
            message: message.into(),
        }
    }

    pub fn warn(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warn,
            ..Self::error(code, message)
        }
    }

    #[must_use]
    pub fn at_node(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    #[must_use]
    pub fn at_path(mut self, json_path: impl Into<String>) -> Self {
        self.json_path = Some(json_path.into());
        self
    }

    fn sort_key(&self) -> (&Option<String>, &str, &Option<String>, Severity, &str) {
        (
            &self.node_id,
            &self.code,
            &self.json_path,
            self.severity,
            &self.message,
        )
    }
}

impl Ord for Issue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for Issue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warn => "warn",
        };
        write!(f, "{level}[{}]", self.code)?;
        if let Some(node) = &self.node_id {
            write!(f, " node '{node}'")?;
        }
        if let Some(path) = &self.json_path {
            write!(f, " at {path}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// Issues sorted by (node, code, path, severity, message) with exact duplicates removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn new(mut issues: Vec<Issue>) -> Self {
        issues.sort();
        issues.dedup();
        Self { issues }
    }

    /// True when there are no errors; warnings do not block.
    pub fn passed(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Warn)
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.issues.iter().any(|i| i.code == code)
    }
}
