//! HTTP API spec metadata (`api_specs` table).

use rusqlite::{params, Connection};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewApiSpec {
    pub id: String,
    pub name: String,
    pub method: String,
    pub path: String,
    pub auth_scheme_ref: Option<String>,
}

pub struct ApiSpecRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ApiSpecRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Inserts or replaces an API spec row. `auth_scheme_ref` must be a credential reference, not plaintext.
    pub fn upsert(&self, spec: &NewApiSpec) -> Result<()> {
        if spec.id.trim().is_empty() {
            return Err(StoreError::Invalid("api spec id must not be empty".into()));
        }
        secrets::validate_credential_ref(spec.auth_scheme_ref.as_deref())
            .map_err(|e| StoreError::Invalid(e.to_string()))?;

        self.conn
            .execute(
                "INSERT INTO api_specs (id, name, method, path, auth_scheme_ref)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    method = excluded.method,
                    path = excluded.path,
                    auth_scheme_ref = excluded.auth_scheme_ref",
                params![
                    spec.id,
                    spec.name,
                    spec.method,
                    spec.path,
                    spec.auth_scheme_ref
                ],
            )
            .map(|_| ())
            .map_err(Into::into)
    }
}
