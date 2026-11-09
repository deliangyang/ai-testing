use std::path::PathBuf;

use model::ModelError;

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("prompt assembly failed: {0}")]
    Prompt(#[from] PromptError),
    #[error("LLM call failed: {0}")]
    Llm(#[from] llm_gateway::LlmError),
    #[error("model returned executable code instead of DAG JSON")]
    RejectedCodeOutput,
    #[error("no DAG JSON found in model output")]
    NoJsonFound,
    #[error("invalid DAG JSON: {0}")]
    InvalidDag(#[from] ModelError),
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("prompt manifest not found: {0}")]
    ManifestNotFound(PathBuf),
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid prompt manifest: {0}")]
    InvalidManifest(String),
    #[error("template {id} not listed in manifest")]
    UnknownTemplate { id: String },
    #[error("template file missing for {id}: {path}")]
    TemplateFileMissing { id: String, path: PathBuf },
    #[error("invalid template front matter in {path}: {reason}")]
    InvalidFrontMatter { path: PathBuf, reason: String },
    #[error("canonical hash failed: {0}")]
    Canonical(#[from] model::CanonicalError),
    #[error("invalid api schema: {0}")]
    InvalidApiSchema(String),
}
