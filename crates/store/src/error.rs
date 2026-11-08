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
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;
