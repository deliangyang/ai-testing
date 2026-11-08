//! DAG runs: the single source of truth for run state.

use std::fmt;
use std::str::FromStr;

use common::clock::{Clock, Timestamp};
use common::id::IdGen;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub const ALL: [RunStatus; 5] = [
        RunStatus::Pending,
        RunStatus::Running,
        RunStatus::Completed,
        RunStatus::Failed,
        RunStatus::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Pending => "PENDING",
            RunStatus::Running => "RUNNING",
            RunStatus::Completed => "COMPLETED",
            RunStatus::Failed => "FAILED",
            RunStatus::Cancelled => "CANCELLED",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }

    /// `PENDING -> RUNNING -> COMPLETED | FAILED | CANCELLED`. A pending run may also fail
    /// (e.g. its environment cannot be resolved) or be cancelled before it starts.
    pub fn can_transition_to(self, to: RunStatus) -> bool {
        use RunStatus::*;
        matches!(
            (self, to),
            (Pending, Running | Failed | Cancelled) | (Running, Completed | Failed | Cancelled)
        )
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RunStatus {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        RunStatus::ALL
            .into_iter()
            .find(|st| st.as_str() == s)
            .ok_or_else(|| StoreError::Corrupt(format!("unknown run status '{s}'")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunMode {
    Manual,
    Ci,
}

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Manual => "manual",
            RunMode::Ci => "ci",
        }
    }
}

impl FromStr for RunMode {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "manual" => Ok(RunMode::Manual),
            "ci" => Ok(RunMode::Ci),
            other => Err(StoreError::Corrupt(format!("unknown run mode '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRun {
    pub dag_id: String,
    pub dag_version: u32,
    pub env_name: String,
    pub mode: RunMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: String,
    pub dag_id: String,
    pub dag_version: u32,
    pub env_name: String,
    pub mode: RunMode,
    pub status: RunStatus,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

/// Filters for [`RunRepository::list`]; empty fields match everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunQuery {
    pub dag_id: Option<String>,
    pub status: Option<RunStatus>,
    pub limit: Option<u32>,
}

pub struct RunRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
    ids: &'a dyn IdGen,
}

const COLUMNS: &str = "id, dag_id, dag_version, env_name, mode, status, started_at, finished_at";

impl<'a> RunRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock, ids: &'a dyn IdGen) -> Self {
        Self { conn, clock, ids }
    }

    /// Creates a `PENDING` run of an existing DAG version.
    pub fn create(&self, run: &NewRun) -> Result<RunRecord> {
        let id = self.ids.next_id();
        let exists: Option<u32> = self
            .conn
            .query_row(
                "SELECT version FROM dag_definitions WHERE id = ?1 AND version = ?2",
                params![run.dag_id, run.dag_version],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!(
                "DAG '{}' version {}",
                run.dag_id, run.dag_version
            )));
        }
        self.conn.execute(
            "INSERT INTO dag_runs (id, dag_id, dag_version, env_name, mode, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'PENDING')",
            params![
                id,
                run.dag_id,
                run.dag_version,
                run.env_name,
                run.mode.as_str()
            ],
        )?;
        Ok(RunRecord {
            id,
            dag_id: run.dag_id.clone(),
            dag_version: run.dag_version,
            env_name: run.env_name.clone(),
            mode: run.mode,
            status: RunStatus::Pending,
            started_at: None,
            finished_at: None,
        })
    }

    /// Moves the run to `to`, stamping `started_at` on `RUNNING` and `finished_at` on a
    /// terminal status. The update is conditional on the status it was checked against, so a
    /// concurrent transition makes this one fail instead of being overwritten.
    pub fn transition(&self, id: &str, to: RunStatus) -> Result<RunRecord> {
        let current = self
            .get(id)?
            .ok_or_else(|| StoreError::NotFound(format!("run '{id}'")))?;
        if !current.status.can_transition_to(to) {
            return Err(StoreError::InvalidTransition {
                entity: format!("run '{id}'"),
                from: current.status.to_string(),
                to: to.to_string(),
            });
        }
        let now = self.clock.now().to_string();
        let changed = self.conn.execute(
            "UPDATE dag_runs SET status = ?1,
                 started_at  = CASE WHEN ?1 = 'RUNNING' THEN ?2 ELSE started_at END,
                 finished_at = CASE WHEN ?1 IN ('COMPLETED', 'FAILED', 'CANCELLED')
                               THEN ?2 ELSE finished_at END
             WHERE id = ?3 AND status = ?4",
            params![to.as_str(), now, id, current.status.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::Conflict(format!(
                "run '{id}' left {} before the transition to {to}",
                current.status
            )));
        }
        self.get(id)?
            .ok_or_else(|| StoreError::NotFound(format!("run '{id}'")))
    }

    pub fn get(&self, id: &str) -> Result<Option<RunRecord>> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM dag_runs WHERE id = ?1"),
                [id],
                raw_run,
            )
            .optional()?
            .map(RawRun::decode)
            .transpose()
    }

    /// Newest first (run ids are time-ordered).
    pub fn list(&self, query: &RunQuery) -> Result<Vec<RunRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM dag_runs
             WHERE (?1 IS NULL OR dag_id = ?1) AND (?2 IS NULL OR status = ?2)
             ORDER BY id DESC LIMIT ?3"
        ))?;
        let limit = query.limit.map_or(-1, i64::from);
        let rows = stmt.query_map(
            params![query.dag_id, query.status.map(RunStatus::as_str), limit],
            raw_run,
        )?;
        rows.map(|r| r.map_err(StoreError::from).and_then(RawRun::decode))
            .collect()
    }
}

struct RawRun {
    id: String,
    dag_id: String,
    dag_version: u32,
    env_name: String,
    mode: String,
    status: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

fn timestamp(s: Option<String>) -> Result<Option<Timestamp>> {
    s.map(|s| s.parse().map_err(|e| StoreError::Corrupt(format!("{e}"))))
        .transpose()
}

impl RawRun {
    fn decode(self) -> Result<RunRecord> {
        Ok(RunRecord {
            mode: self.mode.parse()?,
            status: self.status.parse()?,
            started_at: timestamp(self.started_at)?,
            finished_at: timestamp(self.finished_at)?,
            id: self.id,
            dag_id: self.dag_id,
            dag_version: self.dag_version,
            env_name: self.env_name,
        })
    }
}

fn raw_run(r: &Row<'_>) -> rusqlite::Result<RawRun> {
    Ok(RawRun {
        id: r.get(0)?,
        dag_id: r.get(1)?,
        dag_version: r.get(2)?,
        env_name: r.get(3)?,
        mode: r.get(4)?,
        status: r.get(5)?,
        started_at: r.get(6)?,
        finished_at: r.get(7)?,
    })
}
