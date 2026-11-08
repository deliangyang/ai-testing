//! SQLite persistence: migrations and repositories.

pub mod dags;
pub mod db;
pub mod error;
pub mod failures;
pub mod migrate;
pub mod nodes;
pub mod runs;
pub mod ui_layout;

pub use dags::{DagRecord, DagRepository, DagStatus, DagVersionInfo};
pub use db::Db;
pub use error::{Result, StoreError};
pub use failures::{FailureRecord, FailureRecordRepository, NewFailure};
pub use migrate::{Migration, MIGRATIONS};
pub use nodes::{
    ExecutionKey, Failure, NodeExecution, NodeExecutionRepository, NodeStatus, NodeUpdate,
};
pub use runs::{NewRun, RunMode, RunQuery, RunRecord, RunRepository, RunStatus};
pub use ui_layout::{UiLayoutRecord, UiLayoutRepository, WorkspacePresetRecord};
