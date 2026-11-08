//! Node executions: one append-only evidence row per (run, node, attempt).

use std::fmt;

use common::clock::{Clock, Timestamp};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::{Result, StoreError};

pub use model::NodeStatus;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExecutionKey {
    pub run_id: String,
    pub node_id: String,
    pub attempt: u32,
}

impl ExecutionKey {
    pub fn new(run_id: impl Into<String>, node_id: impl Into<String>, attempt: u32) -> Self {
        Self {
            run_id: run_id.into(),
            node_id: node_id.into(),
            attempt,
        }
    }
}

impl fmt::Display for ExecutionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "node '{}' attempt {} of run '{}'",
            self.node_id, self.attempt, self.run_id
        )
    }
}

/// Classification required on every `FAILED` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub category: String,
    pub error_code: String,
}

/// The next state of an execution row. Fields left `None` keep their stored value.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeUpdate {
    pub status: NodeStatus,
    pub output: Option<Value>,
    pub poll_trace: Option<Value>,
    pub failure: Option<Failure>,
}

impl NodeUpdate {
    pub fn status(status: NodeStatus) -> Self {
        Self {
            status,
            output: None,
            poll_trace: None,
            failure: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeExecution {
    pub key: ExecutionKey,
    pub status: NodeStatus,
    /// Rendered input; credentials must already be masked by the caller.
    pub input_resolved: Option<Value>,
    pub output: Option<Value>,
    pub poll_trace: Option<Value>,
    pub failure: Option<Failure>,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

pub struct NodeExecutionRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
}

const COLUMNS: &str = "run_id, node_id, attempt, status, input_resolved_json, output_json, \
                       poll_trace_json, failure_category, error_code, started_at, finished_at";

impl<'a> NodeExecutionRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock) -> Self {
        Self { conn, clock }
    }

    /// Inserts a new attempt in a non-terminal status. Fails with [`StoreError::Conflict`] if
    /// the attempt already exists.
    pub fn insert(
        &self,
        key: &ExecutionKey,
        status: NodeStatus,
        input_resolved: Option<&Value>,
    ) -> Result<NodeExecution> {
        if status.is_terminal() || key.attempt == 0 {
            return Err(StoreError::Invalid(format!(
                "{key}: a new attempt must start at attempt >= 1 in a non-terminal status, got {status}"
            )));
        }
        if self.get(key)?.is_some() {
            return Err(StoreError::Conflict(format!("{key} already exists")));
        }
        let now = self.clock.now();
        let started_at = (status == NodeStatus::Running).then(|| now.to_string());
        self.conn.execute(
            "INSERT INTO node_executions (run_id, node_id, attempt, status, input_resolved_json,
                                          started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                key.run_id,
                key.node_id,
                key.attempt,
                status.as_str(),
                input_resolved.map(Value::to_string),
                started_at
            ],
        )?;
        self.require(key)
    }

    /// Applies `update`. Terminal rows are immutable ([`StoreError::TerminalRow`]); the status
    /// must follow the node state machine; `FAILED` needs a [`Failure`]. `started_at` is set on
    /// entering `RUNNING`, `finished_at` on entering a terminal status.
    pub fn update(&self, key: &ExecutionKey, update: &NodeUpdate) -> Result<NodeExecution> {
        let current = self.require(key)?;
        if current.status.is_terminal() {
            return Err(StoreError::TerminalRow(format!(
                "{key} is {} and cannot change",
                current.status
            )));
        }
        let to = update.status;
        if !current.status.can_transition_to(to) {
            return Err(StoreError::InvalidTransition {
                entity: key.to_string(),
                from: current.status.to_string(),
                to: to.to_string(),
            });
        }
        if to == NodeStatus::Failed && update.failure.is_none() {
            return Err(StoreError::Invalid(format!(
                "{key}: FAILED needs failure_category and error_code"
            )));
        }
        if to != NodeStatus::Failed && update.failure.is_some() {
            return Err(StoreError::Invalid(format!(
                "{key}: only FAILED carries a failure classification"
            )));
        }
        let now = self.clock.now().to_string();
        let started_at = match (to, &current.started_at) {
            (NodeStatus::Running, None) => Some(now.clone()),
            (_, started) => started.map(|t| t.to_string()),
        };
        let finished_at = to.is_terminal().then(|| now.clone());
        let changed = self.conn.execute(
            "UPDATE node_executions SET
                 status = ?1,
                 output_json = coalesce(?2, output_json),
                 poll_trace_json = coalesce(?3, poll_trace_json),
                 failure_category = ?4,
                 error_code = ?5,
                 started_at = ?6,
                 finished_at = ?7
             WHERE run_id = ?8 AND node_id = ?9 AND attempt = ?10 AND status = ?11",
            params![
                to.as_str(),
                update.output.as_ref().map(Value::to_string),
                update.poll_trace.as_ref().map(Value::to_string),
                update.failure.as_ref().map(|f| f.category.as_str()),
                update.failure.as_ref().map(|f| f.error_code.as_str()),
                started_at,
                finished_at,
                key.run_id,
                key.node_id,
                key.attempt,
                current.status.as_str()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::Conflict(format!(
                "{key} left {} before the update",
                current.status
            )));
        }
        self.require(key)
    }

    pub fn get(&self, key: &ExecutionKey) -> Result<Option<NodeExecution>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM node_executions
                     WHERE run_id = ?1 AND node_id = ?2 AND attempt = ?3"
                ),
                params![key.run_id, key.node_id, key.attempt],
                raw,
            )
            .optional()?
            .map(Raw::decode)
            .transpose()
    }

    /// Highest attempt of a node in a run.
    pub fn latest(&self, run_id: &str, node_id: &str) -> Result<Option<NodeExecution>> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM node_executions
                     WHERE run_id = ?1 AND node_id = ?2 ORDER BY attempt DESC LIMIT 1"
                ),
                params![run_id, node_id],
                raw,
            )
            .optional()?
            .map(Raw::decode)
            .transpose()
    }

    /// Every attempt of every node in a run, ordered by node id then attempt so the result does
    /// not depend on parallel completion order.
    pub fn list_for_run(&self, run_id: &str) -> Result<Vec<NodeExecution>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM node_executions WHERE run_id = ?1 ORDER BY node_id, attempt"
        ))?;
        let rows = stmt.query_map([run_id], raw)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(Raw::decode))
            .collect()
    }

    fn require(&self, key: &ExecutionKey) -> Result<NodeExecution> {
        self.get(key)?
            .ok_or_else(|| StoreError::NotFound(key.to_string()))
    }
}

struct Raw {
    run_id: String,
    node_id: String,
    attempt: u32,
    status: String,
    input: Option<String>,
    output: Option<String>,
    poll_trace: Option<String>,
    category: Option<String>,
    error_code: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
}

fn raw(r: &Row<'_>) -> rusqlite::Result<Raw> {
    Ok(Raw {
        run_id: r.get(0)?,
        node_id: r.get(1)?,
        attempt: r.get(2)?,
        status: r.get(3)?,
        input: r.get(4)?,
        output: r.get(5)?,
        poll_trace: r.get(6)?,
        category: r.get(7)?,
        error_code: r.get(8)?,
        started_at: r.get(9)?,
        finished_at: r.get(10)?,
    })
}

fn json(s: Option<String>) -> Result<Option<Value>> {
    s.map(|s| serde_json::from_str(&s).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

fn timestamp(s: Option<String>) -> Result<Option<Timestamp>> {
    s.map(|s| s.parse().map_err(|e| StoreError::Corrupt(format!("{e}"))))
        .transpose()
}

impl Raw {
    fn decode(self) -> Result<NodeExecution> {
        let failure = match (self.category, self.error_code) {
            (Some(category), Some(error_code)) => Some(Failure {
                category,
                error_code,
            }),
            _ => None,
        };
        Ok(NodeExecution {
            key: ExecutionKey::new(self.run_id, self.node_id, self.attempt),
            status: self
                .status
                .parse()
                .map_err(|e| StoreError::Corrupt(format!("{e}")))?,
            input_resolved: json(self.input)?,
            output: json(self.output)?,
            poll_trace: json(self.poll_trace)?,
            failure,
            started_at: timestamp(self.started_at)?,
            finished_at: timestamp(self.finished_at)?,
        })
    }
}
