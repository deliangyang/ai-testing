#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretsError {
    /// Value looks like a raw secret rather than an allowed reference form.
    #[error("credential must be keychain://<service>/<account> or ${{env:VAR}}, not plaintext")]
    PlaintextCredential,

    #[error("invalid credential reference: {0}")]
    InvalidReference(String),

    #[error("keychain: {0}")]
    Keychain(String),

    #[error("environment variable '{0}' is not set")]
    EnvNotSet(String),
}

pub type Result<T, E = SecretsError> = std::result::Result<T, E>;
