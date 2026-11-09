use model::CanonicalError;
use secrets::SecretsError;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("credential resolution failed: {0}")]
    Credential(#[from] SecretsError),
    #[error("cannot build HTTP client: {0}")]
    Client(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("unexpected response: {0}")]
    Response(String),
    #[error("no cassette for request hash {key}")]
    CassetteMiss { key: String },
    #[error("cassette {path}: {message}")]
    CassetteInvalid { path: String, message: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("canonical JSON error: {0}")]
    Canonical(#[from] CanonicalError),
    #[error("failed to persist llm call log: {0}")]
    LogPersist(String),
}
