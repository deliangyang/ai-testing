//! HTTP API spec metadata (`api_specs` table): versioned rows with draft/active/deprecated.

use std::fmt;
use std::str::FromStr;

use common::clock::{Clock, Timestamp};
use model::to_canonical_json;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde_json::Value;

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ApiSpecStatus {
    Draft,
    Active,
    Deprecated,
}

impl ApiSpecStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ApiSpecStatus::Draft => "draft",
            ApiSpecStatus::Active => "active",
            ApiSpecStatus::Deprecated => "deprecated",
        }
    }
}

impl fmt::Display for ApiSpecStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ApiSpecStatus {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "draft" => Ok(ApiSpecStatus::Draft),
            "active" => Ok(ApiSpecStatus::Active),
            "deprecated" => Ok(ApiSpecStatus::Deprecated),
            other => Err(StoreError::Corrupt(format!(
                "unknown API spec status '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSpecVersionInfo {
    pub id: String,
    pub version: u32,
    pub status: ApiSpecStatus,
    pub created_by: String,
    pub created_at: Timestamp,
}

/// Payload for [`ApiSpecRepository::save`]; `version` is assigned on persist.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiSpecSaveInput {
    pub id: String,
    pub name: String,
    pub method: String,
    pub path: String,
    pub params: Option<Value>,
    pub response_schema: Option<Value>,
    pub auth_scheme_ref: Option<String>,
}

/// Back-compat name used in early integration tests.
pub type NewApiSpec = ApiSpecSaveInput;

#[derive(Debug, Clone, PartialEq)]
pub struct ApiSpecRecord {
    pub info: ApiSpecVersionInfo,
    pub spec: ApiSpecSaveInput,
}

pub struct ApiSpecRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
}

const INFO_COLUMNS: &str = "id, version, status, created_by, created_at";

impl<'a> ApiSpecRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock) -> Self {
        Self { conn, clock }
    }

    /// Stores the next version of `spec.id` in `draft` state. JSON columns are canonicalized.
    pub fn save(&self, spec: &ApiSpecSaveInput, created_by: &str) -> Result<ApiSpecRecord> {
        validate_save(spec)?;

        let params_json = json_column(spec.params.as_ref())?;
        let response_schema_json = json_column(spec.response_schema.as_ref())?;

        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let latest: Option<u32> = tx.query_row(
            "SELECT max(version) FROM api_specs WHERE id = ?1",
            [&spec.id],
            |r| r.get(0),
        )?;
        let version = latest.unwrap_or(0) + 1;
        let created_at = self.clock.now();
        tx.execute(
            "INSERT INTO api_specs (
                id, version, name, method, path, params_json, response_schema_json,
                auth_scheme_ref, status, created_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'draft', ?9, ?10)",
            params![
                spec.id,
                version,
                spec.name,
                spec.method,
                spec.path,
                params_json,
                response_schema_json,
                spec.auth_scheme_ref,
                created_by,
                created_at.to_string(),
            ],
        )?;
        tx.commit()?;

        Ok(ApiSpecRecord {
            info: ApiSpecVersionInfo {
                id: spec.id.clone(),
                version,
                status: ApiSpecStatus::Draft,
                created_by: created_by.to_owned(),
                created_at,
            },
            spec: spec.clone(),
        })
    }

    /// Makes `version` the active one and deprecates the previous active version.
    pub fn activate(&self, id: &str, version: u32, expected_active: Option<u32>) -> Result<()> {
        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT status FROM api_specs WHERE id = ?1 AND version = ?2",
                params![id, version],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!(
                "API spec '{id}' version {version}"
            )));
        }
        let current = active_version(&tx, id)?;
        if current != expected_active {
            return Err(StoreError::Conflict(format!(
                "API spec '{id}' active version is {}, expected {}",
                show(current),
                show(expected_active)
            )));
        }
        if current == Some(version) {
            return Ok(());
        }
        tx.execute(
            "UPDATE api_specs SET status = 'deprecated' WHERE id = ?1 AND status = 'active'",
            [id],
        )?;
        tx.execute(
            "UPDATE api_specs SET status = 'active' WHERE id = ?1 AND version = ?2",
            params![id, version],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &str, version: u32) -> Result<Option<ApiSpecRecord>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {INFO_COLUMNS}, name, method, path, params_json, response_schema_json, auth_scheme_ref
                     FROM api_specs WHERE id = ?1 AND version = ?2"
                ),
                params![id, version],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    pub fn get_active(&self, id: &str) -> Result<Option<ApiSpecRecord>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {INFO_COLUMNS}, name, method, path, params_json, response_schema_json, auth_scheme_ref
                     FROM api_specs WHERE id = ?1 AND status = 'active'"
                ),
                [id],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    /// All versions of `id`, oldest first.
    pub fn list_versions(&self, id: &str) -> Result<Vec<ApiSpecVersionInfo>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {INFO_COLUMNS} FROM api_specs WHERE id = ?1 ORDER BY version"
        ))?;
        let rows = stmt.query_map([id], raw_info)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(RawInfo::decode))
            .collect()
    }

    pub fn active_version(&self, id: &str) -> Result<Option<u32>> {
        active_version(self.conn, id)
    }

    /// One row per `id`: active version when present, otherwise the highest version.
    pub fn list_effective_specs(&self) -> Result<Vec<ApiSpecRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM api_specs GROUP BY id ORDER BY id COLLATE NOCASE")?;
        let ids: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(active) = self.get_active(&id)? {
                out.push(active);
            } else if let Some(latest) = self.get_latest_any_status(&id)? {
                out.push(latest);
            }
        }
        Ok(out)
    }

    fn get_latest_any_status(&self, id: &str) -> Result<Option<ApiSpecRecord>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {INFO_COLUMNS}, name, method, path, params_json, response_schema_json, auth_scheme_ref
                     FROM api_specs WHERE id = ?1 ORDER BY version DESC LIMIT 1"
                ),
                [id],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    /// Inserts or replaces a single draft row (legacy helper); prefer [`save`](Self::save).
    #[deprecated(note = "use save/activate versioning instead")]
    pub fn upsert(&self, spec: &ApiSpecSaveInput) -> Result<()> {
        validate_save(spec)?;
        self.conn.execute(
            "INSERT INTO api_specs (
                id, version, name, method, path, params_json, response_schema_json,
                auth_scheme_ref, status, created_by, created_at
             ) VALUES (?1, 1, ?2, ?3, ?4, NULL, NULL, ?5, 'draft', 'legacy-upsert', ?6)
             ON CONFLICT(id, version) DO UPDATE SET
                name = excluded.name,
                method = excluded.method,
                path = excluded.path,
                auth_scheme_ref = excluded.auth_scheme_ref",
            params![
                spec.id,
                spec.name,
                spec.method,
                spec.path,
                spec.auth_scheme_ref,
                self.clock.now().to_string(),
            ],
        )?;
        Ok(())
    }
}

fn validate_save(spec: &ApiSpecSaveInput) -> Result<()> {
    if spec.id.trim().is_empty() {
        return Err(StoreError::Invalid("api spec id must not be empty".into()));
    }
    secrets::validate_credential_ref(spec.auth_scheme_ref.as_deref())
        .map_err(|e| StoreError::Invalid(e.to_string()))?;
    Ok(())
}

fn json_column(value: Option<&Value>) -> Result<Option<String>> {
    value
        .map(to_canonical_json)
        .transpose()
        .map_err(|e| StoreError::Corrupt(e.to_string()))
}

fn active_version(conn: &Connection, id: &str) -> Result<Option<u32>> {
    Ok(conn
        .query_row(
            "SELECT version FROM api_specs WHERE id = ?1 AND status = 'active'",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}

fn show(version: Option<u32>) -> String {
    version.map_or_else(|| "none".to_owned(), |v| v.to_string())
}

struct RawInfo {
    id: String,
    version: u32,
    status: String,
    created_by: String,
    created_at: String,
}

impl RawInfo {
    fn decode(self) -> Result<ApiSpecVersionInfo> {
        Ok(ApiSpecVersionInfo {
            status: self.status.parse()?,
            created_at: self
                .created_at
                .parse()
                .map_err(|e| StoreError::Corrupt(format!("{e}")))?,
            id: self.id,
            version: self.version,
            created_by: self.created_by,
        })
    }
}

fn raw_info(r: &Row<'_>) -> rusqlite::Result<RawInfo> {
    Ok(RawInfo {
        id: r.get(0)?,
        version: r.get(1)?,
        status: r.get(2)?,
        created_by: r.get(3)?,
        created_at: r.get(4)?,
    })
}

struct RawRecord {
    info: RawInfo,
    name: String,
    method: String,
    path: String,
    params_json: Option<String>,
    response_schema_json: Option<String>,
    auth_scheme_ref: Option<String>,
}

impl RawRecord {
    fn decode(self) -> Result<ApiSpecRecord> {
        let info = self.info.decode()?;
        let params = parse_json_column(&self.params_json)?;
        let response_schema = parse_json_column(&self.response_schema_json)?;
        Ok(ApiSpecRecord {
            spec: ApiSpecSaveInput {
                id: info.id.clone(),
                name: self.name,
                method: self.method,
                path: self.path,
                params,
                response_schema,
                auth_scheme_ref: self.auth_scheme_ref,
            },
            info,
        })
    }
}

fn parse_json_column(raw: &Option<String>) -> Result<Option<Value>> {
    raw.as_deref()
        .map(|s| {
            serde_json::from_str(s)
                .map_err(|e| StoreError::Corrupt(format!("invalid stored JSON: {e}")))
        })
        .transpose()
}

fn raw_record(r: &Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        info: raw_info(r)?,
        name: r.get(5)?,
        method: r.get(6)?,
        path: r.get(7)?,
        params_json: r.get(8)?,
        response_schema_json: r.get(9)?,
        auth_scheme_ref: r.get(10)?,
    })
}
