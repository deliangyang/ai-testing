//! JSON-friendly store / server access for the NSH `native_module` boundary.
//!
//! TSX shells call into `host` after `ensure_api()` to obtain an HTTP base URL for
//! [`web` `HttpDataSource`](../../web/src/data/http.ts), or use the synchronous read
//! helpers for headless checks.

mod dto;
mod embed;
mod host;
mod read;

use std::path::Path;

use common::clock::Clock;
use serde::{Deserialize, Serialize};
use store::{DagRepository, Db, StoreError};

pub use dto::{DagDetailDto, DagVersionDto, FailureDto, GateResponse, NodeExecutionDto, RunDto};
pub use embed::EmbeddedApi;
pub use host::{ensure_api, ensure_default_api, reset_host_for_tests, HostConfig};
pub use read::{
    get_active_dag, get_dag_version, get_run, list_dags as list_dags_for_state, list_executions,
    list_runs as list_runs_for_state, validate_dag,
};

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagListEntry {
    pub id: String,
    pub version: u32,
    pub status: String,
}

/// Lists active DAG rows for a TSX shell (mirrors server read API shape).
pub fn list_dags(db_path: &Path, clock: &dyn Clock) -> Result<Vec<DagListEntry>, BridgeError> {
    let db = Db::open(db_path, clock)?;
    let repo = DagRepository::new(db.conn(), clock);
    let rows = repo.list()?;
    Ok(rows
        .into_iter()
        .map(|row| DagListEntry {
            id: row.id,
            version: row.version,
            status: row.status.as_str().to_string(),
        })
        .collect())
}

pub mod datasource {
    pub use crate::host::{
        ensure_api, ensure_default_api, get_active_dag, get_dag_version, get_run, list_dags,
        list_executions, list_runs, validate_dag_json, HostConfig,
    };
}
