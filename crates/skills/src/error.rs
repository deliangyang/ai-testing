//! Skill package parse and serialization errors.

use std::fmt;

use model::Format;

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("invalid {format} skill package at '{path}': {message}")]
    Parse {
        format: Format,
        path: String,
        message: String,
    },
    #[error("failed to serialize skill package: {0}")]
    Serialize(String),
}

impl SkillError {
    pub(crate) fn parse<E: fmt::Display>(
        format: Format,
        err: serde_path_to_error::Error<E>,
    ) -> Self {
        Self::Parse {
            format,
            path: err.path().to_string(),
            message: err.inner().to_string(),
        }
    }
}
