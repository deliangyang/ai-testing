//! Per-invocation LLM audit records (metadata only).

use std::fmt;
use std::str::FromStr;

use common::id::IdGen;
use rusqlite::{params, Connection, Row};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LlmCallOutcome {
    Success,
    Error,
}

impl LlmCallOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmCallOutcome::Success => "success",
            LlmCallOutcome::Error => "error",
        }
    }
}

impl fmt::Display for LlmCallOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LlmCallOutcome {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "success" => Ok(LlmCallOutcome::Success),
            "error" => Ok(LlmCallOutcome::Error),
            other => Err(StoreError::Corrupt(format!(
                "unknown llm call outcome '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewLlmCallLog {
    pub run_id: Option<String>,
    pub node_id: Option<String>,
    pub purpose: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost: Option<f64>,
    pub latency_ms: u32,
    pub outcome: LlmCallOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCallLogRecord {
    pub id: String,
    pub run_id: Option<String>,
    pub node_id: Option<String>,
    pub purpose: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost: Option<f64>,
    pub latency_ms: u32,
    pub outcome: LlmCallOutcome,
}

pub struct LlmCallLogRepository<'a> {
    conn: &'a Connection,
    ids: &'a dyn IdGen,
}

const COLUMNS: &str =
    "id, run_id, node_id, purpose, tokens_in, tokens_out, cost, latency_ms, outcome";

impl<'a> LlmCallLogRepository<'a> {
    pub fn new(conn: &'a Connection, ids: &'a dyn IdGen) -> Self {
        Self { conn, ids }
    }

    pub fn insert(&self, entry: &NewLlmCallLog) -> Result<LlmCallLogRecord> {
        if entry.purpose.trim().is_empty() {
            return Err(StoreError::Invalid("purpose must not be empty".into()));
        }
        let record = LlmCallLogRecord {
            id: self.ids.next_id(),
            run_id: entry.run_id.clone(),
            node_id: entry.node_id.clone(),
            purpose: entry.purpose.clone(),
            tokens_in: entry.tokens_in,
            tokens_out: entry.tokens_out,
            cost: entry.cost,
            latency_ms: entry.latency_ms,
            outcome: entry.outcome,
        };
        self.conn.execute(
            "INSERT INTO llm_call_logs (id, run_id, node_id, purpose, tokens_in, tokens_out,
                                        cost, latency_ms, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.run_id,
                record.node_id,
                record.purpose,
                i64::from(record.tokens_in),
                i64::from(record.tokens_out),
                record.cost,
                i64::from(record.latency_ms),
                record.outcome.as_str(),
            ],
        )?;
        Ok(record)
    }

    /// Logs tied to a run in insertion order.
    pub fn list_for_run(&self, run_id: &str) -> Result<Vec<LlmCallLogRecord>> {
        self.query(
            &format!("SELECT {COLUMNS} FROM llm_call_logs WHERE run_id = ?1 ORDER BY rowid, id"),
            params![run_id],
        )
    }

    /// Most recent matching purpose first (by rowid).
    pub fn list_by_purpose(&self, purpose: &str, limit: u32) -> Result<Vec<LlmCallLogRecord>> {
        self.query(
            &format!(
                "SELECT {COLUMNS} FROM llm_call_logs WHERE purpose = ?1
                 ORDER BY rowid DESC LIMIT ?2"
            ),
            params![purpose, limit],
        )
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<LlmCallLogRecord>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, raw)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(Raw::decode))
            .collect()
    }
}

struct Raw {
    id: String,
    run_id: Option<String>,
    node_id: Option<String>,
    purpose: String,
    tokens_in: i64,
    tokens_out: i64,
    cost: Option<f64>,
    latency_ms: i64,
    outcome: String,
}

fn raw(r: &Row<'_>) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: r.get(0)?,
        run_id: r.get(1)?,
        node_id: r.get(2)?,
        purpose: r.get(3)?,
        tokens_in: r.get(4)?,
        tokens_out: r.get(5)?,
        cost: r.get(6)?,
        latency_ms: r.get(7)?,
        outcome: r.get(8)?,
    })
}

impl Raw {
    fn decode(self) -> Result<LlmCallLogRecord> {
        let tokens_in = u32::try_from(self.tokens_in)
            .map_err(|_| StoreError::Corrupt("tokens_in out of range".into()))?;
        let tokens_out = u32::try_from(self.tokens_out)
            .map_err(|_| StoreError::Corrupt("tokens_out out of range".into()))?;
        let latency_ms = u32::try_from(self.latency_ms)
            .map_err(|_| StoreError::Corrupt("latency_ms out of range".into()))?;
        Ok(LlmCallLogRecord {
            id: self.id,
            run_id: self.run_id,
            node_id: self.node_id,
            purpose: self.purpose,
            tokens_in,
            tokens_out,
            cost: self.cost,
            latency_ms,
            outcome: self.outcome.parse()?,
        })
    }
}
