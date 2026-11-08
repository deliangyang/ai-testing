#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error("invalid migration '{file}': {reason}")]
    InvalidMigration { file: String, reason: String },

    /// The database was migrated by a newer build that knows migrations this one does not.
    #[error("database has migration V{version:03} applied, which this build does not know")]
    UnknownMigration { version: u32 },

    #[error("migration V{version:03} '{name}' was modified after it was applied")]
    ChecksumMismatch { version: u32, name: String },

    #[error("migration V{version:03} is pending but V{applied:03} is already applied")]
    OutOfOrder { version: u32, applied: u32 },

    #[error("{entity} cannot go from {from} to {to}")]
    InvalidTransition {
        entity: String,
        from: String,
        to: String,
    },

    /// Terminal evidence rows are immutable.
    #[error("terminal row: {0}")]
    TerminalRow(String),

    /// The caller asked for something the data model does not allow.
    #[error("invalid: {0}")]
    Invalid(String),

    #[error("not found: {0}")]
    NotFound(String),

    /// A concurrent writer changed the state this operation was based on.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Stored data that this build cannot interpret.
    #[error("corrupt data: {0}")]
    Corrupt(String),
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;
