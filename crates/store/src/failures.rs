//! Classified failures, queryable by run and by category.

use common::clock::{Clock, Timestamp};
use common::id::IdGen;
use rusqlite::{params, Connection, Row};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFailure {
    pub run_id: String,
    /// `None` for run-level failures (e.g. environment resolution).
    pub node_id: Option<String>,
    pub failure_category: String,
    pub error_code: Option<String>,
    /// Must already have credentials masked.
    pub raw_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecord {
    pub id: String,
    pub run_id: String,
    pub node_id: Option<String>,
    pub failure_category: String,
    pub error_code: Option<String>,
    pub raw_message: String,
    pub classified_at: Timestamp,
    pub resolution: Option<String>,
}

pub struct FailureRecordRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
    ids: &'a dyn IdGen,
}

const COLUMNS: &str =
    "id, run_id, node_id, failure_category, error_code, raw_message, classified_at, resolution";

impl<'a> FailureRecordRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock, ids: &'a dyn IdGen) -> Self {
        Self { conn, clock, ids }
    }

    pub fn insert(&self, failure: &NewFailure) -> Result<FailureRecord> {
        if failure.failure_category.trim().is_empty() {
            return Err(StoreError::Invalid(
                "failure_category must not be empty".into(),
            ));
        }
        let record = FailureRecord {
            id: self.ids.next_id(),
            run_id: failure.run_id.clone(),
            node_id: failure.node_id.clone(),
            failure_category: failure.failure_category.clone(),
            error_code: failure.error_code.clone(),
            raw_message: failure.raw_message.clone(),
            classified_at: self.clock.now(),
            resolution: None,
        };
        self.conn
            .execute(
                "INSERT INTO failure_records (id, run_id, node_id, failure_category, error_code,
                                              raw_message, classified_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.id,
                    record.run_id,
                    record.node_id,
                    record.failure_category,
                    record.error_code,
                    record.raw_message,
                    record.classified_at.to_string()
                ],
            )
            .map_err(|e| match e.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    StoreError::NotFound(format!("run '{}'", failure.run_id))
                }
                _ => e.into(),
            })?;
        Ok(record)
    }

    /// Failures of a run in the order they were recorded.
    pub fn list_for_run(&self, run_id: &str) -> Result<Vec<FailureRecord>> {
        self.query(
            &format!(
                "SELECT {COLUMNS} FROM failure_records WHERE run_id = ?1
                 ORDER BY classified_at, id"
            ),
            params![run_id],
        )
    }

    /// Most recent failures of one category across runs.
    pub fn list_by_category(&self, category: &str, limit: u32) -> Result<Vec<FailureRecord>> {
        self.query(
            &format!(
                "SELECT {COLUMNS} FROM failure_records WHERE failure_category = ?1
                 ORDER BY classified_at DESC, id DESC LIMIT ?2"
            ),
            params![category, limit],
        )
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<FailureRecord>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, raw)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(Raw::decode))
            .collect()
    }
}

struct Raw {
    id: String,
    run_id: String,
    node_id: Option<String>,
    failure_category: String,
    error_code: Option<String>,
    raw_message: String,
    classified_at: String,
    resolution: Option<String>,
}

fn raw(r: &Row<'_>) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: r.get(0)?,
        run_id: r.get(1)?,
        node_id: r.get(2)?,
        failure_category: r.get(3)?,
        error_code: r.get(4)?,
        raw_message: r.get(5)?,
        classified_at: r.get(6)?,
        resolution: r.get(7)?,
    })
}

impl Raw {
    fn decode(self) -> Result<FailureRecord> {
        Ok(FailureRecord {
            classified_at: self
                .classified_at
                .parse()
                .map_err(|e| StoreError::Corrupt(format!("{e}")))?,
            id: self.id,
            run_id: self.run_id,
            node_id: self.node_id,
            failure_category: self.failure_category,
            error_code: self.error_code,
            raw_message: self.raw_message,
            resolution: self.resolution,
        })
    }
}
