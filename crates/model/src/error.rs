use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Format::Json => "JSON",
            Format::Yaml => "YAML",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    /// `path` is the location inside the document, e.g. `nodes[2]`.
    #[error("invalid {format} DAG at '{path}': {message}")]
    Parse {
        format: Format,
        path: String,
        message: String,
    },
    #[error("failed to serialize DAG: {0}")]
    Serialize(String),
}

impl ModelError {
    pub(crate) fn parse<E: fmt::Display>(
        format: Format,
        err: serde_path_to_error::Error<E>,
    ) -> Self {
        ModelError::Parse {
            format,
            path: err.path().to_string(),
            message: err.inner().to_string(),
        }
    }
}
