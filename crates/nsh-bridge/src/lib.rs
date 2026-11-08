//! JSON-friendly store access for a desktop `native_module` boundary.
//!
//! NSH TSX imports typed functions generated from the Rust signatures that
//! will live on `#[native_module("aitest/native")]`. This crate is the spike
//! implementation of that Rust side without taking a dependency on `nsh-core`.

use std::path::Path;

use common::clock::Clock;
use serde::{Deserialize, Serialize};
use store::{DagRepository, Db, StoreError};

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
    Ok(
        rows
            .into_iter()
            .map(|row| DagListEntry {
                id: row.id,
                version: row.version,
                status: row.status.as_str().to_string(),
            })
            .collect(),
    )
}
