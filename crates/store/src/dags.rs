//! Versioned DAG definitions: `pending` on save, one `active` per id, the rest `archived`.

use std::fmt;
use std::str::FromStr;

use common::clock::{Clock, Timestamp};
use model::{to_canonical_json, Dag};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DagStatus {
    Pending,
    Active,
    Archived,
}

impl DagStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DagStatus::Pending => "pending",
            DagStatus::Active => "active",
            DagStatus::Archived => "archived",
        }
    }
}

impl fmt::Display for DagStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DagStatus {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(DagStatus::Pending),
            "active" => Ok(DagStatus::Active),
            "archived" => Ok(DagStatus::Archived),
            other => Err(StoreError::Corrupt(format!("unknown DAG status '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagVersionInfo {
    pub id: String,
    pub version: u32,
    pub status: DagStatus,
    pub created_by: String,
    pub created_at: Timestamp,
}

/// A stored version; `dag.version` is always set to `info.version`.
#[derive(Debug, Clone, PartialEq)]
pub struct DagRecord {
    pub info: DagVersionInfo,
    pub dag: Dag,
}

pub struct DagRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
}

const INFO_COLUMNS: &str = "id, version, status, created_by, created_at";

impl<'a> DagRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock) -> Self {
        Self { conn, clock }
    }

    /// Stores `dag` as the next version of `dag.id`, in `pending` state. Any `version` already
    /// on `dag` is ignored; the content is stored as canonical JSON.
    pub fn save(&self, dag: &Dag, created_by: &str) -> Result<DagRecord> {
        let mut content = dag.clone();
        content.version = None;
        let json = to_canonical_json(&content).map_err(|e| StoreError::Corrupt(e.to_string()))?;

        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let latest: Option<u32> = tx.query_row(
            "SELECT max(version) FROM dag_definitions WHERE id = ?1",
            [&dag.id],
            |r| r.get(0),
        )?;
        let version = latest.unwrap_or(0) + 1;
        let created_at = self.clock.now();
        tx.execute(
            "INSERT INTO dag_definitions (id, version, content_json, status, created_by, created_at)
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
            params![dag.id, version, json, created_by, created_at.to_string()],
        )?;
        tx.commit()?;

        content.version = Some(version);
        Ok(DagRecord {
            info: DagVersionInfo {
                id: dag.id.clone(),
                version,
                status: DagStatus::Pending,
                created_by: created_by.to_owned(),
                created_at,
            },
            dag: content,
        })
    }

    /// Makes `version` the active one and archives the previous active version.
    ///
    /// `expected_active` is the active version the caller based its decision on; if another
    /// activation got there first the call fails with [`StoreError::Conflict`] and changes
    /// nothing. Activating an archived version is a rollback and is allowed.
    pub fn activate(&self, id: &str, version: u32, expected_active: Option<u32>) -> Result<()> {
        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT status FROM dag_definitions WHERE id = ?1 AND version = ?2",
                params![id, version],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!(
                "DAG '{id}' version {version}"
            )));
        }
        let current = active_version(&tx, id)?;
        if current != expected_active {
            return Err(StoreError::Conflict(format!(
                "DAG '{id}' active version is {}, expected {}",
                show(current),
                show(expected_active)
            )));
        }
        if current == Some(version) {
            return Ok(());
        }
        tx.execute(
            "UPDATE dag_definitions SET status = 'archived' WHERE id = ?1 AND status = 'active'",
            [id],
        )?;
        tx.execute(
            "UPDATE dag_definitions SET status = 'active' WHERE id = ?1 AND version = ?2",
            params![id, version],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &str, version: u32) -> Result<Option<DagRecord>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {INFO_COLUMNS}, content_json FROM dag_definitions
                     WHERE id = ?1 AND version = ?2"
                ),
                params![id, version],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    pub fn active(&self, id: &str) -> Result<Option<DagRecord>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {INFO_COLUMNS}, content_json FROM dag_definitions
                     WHERE id = ?1 AND status = 'active'"
                ),
                [id],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    /// All versions of `id`, oldest first.
    pub fn versions(&self, id: &str) -> Result<Vec<DagVersionInfo>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {INFO_COLUMNS} FROM dag_definitions WHERE id = ?1 ORDER BY version"
        ))?;
        let rows = stmt.query_map([id], raw_info)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(RawInfo::decode))
            .collect()
    }
}

fn active_version(conn: &Connection, id: &str) -> Result<Option<u32>> {
    Ok(conn
        .query_row(
            "SELECT version FROM dag_definitions WHERE id = ?1 AND status = 'active'",
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
    fn decode(self) -> Result<DagVersionInfo> {
        Ok(DagVersionInfo {
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
    content: String,
}

impl RawRecord {
    fn decode(self) -> Result<DagRecord> {
        let info = self.info.decode()?;
        let mut dag = Dag::from_json_str(&self.content).map_err(|e| {
            StoreError::Corrupt(format!("DAG '{}' version {}: {e}", info.id, info.version))
        })?;
        dag.version = Some(info.version);
        Ok(DagRecord { info, dag })
    }
}

fn raw_record(r: &Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        info: raw_info(r)?,
        content: r.get(5)?,
    })
}
