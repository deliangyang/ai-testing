//! SQLite persistence: migrations and repositories.

pub mod api_specs;
pub mod dags;
pub mod db;
pub mod error;
pub mod failures;
pub mod llm_call_logs;
pub mod migrate;
pub mod nodes;
pub mod runs;
pub mod ui_layout;

pub use api_specs::{ApiSpecRepository, NewApiSpec};
pub use dags::{DagRecord, DagRepository, DagStatus, DagVersionInfo};
pub use db::Db;
pub use error::{Result, StoreError};
pub use failures::{FailureRecord, FailureRecordRepository, NewFailure};
pub use llm_call_logs::{LlmCallLogRecord, LlmCallLogRepository, LlmCallOutcome, NewLlmCallLog};
pub use migrate::{Migration, MIGRATIONS};
pub use nodes::{
    ExecutionKey, Failure, NodeExecution, NodeExecutionRepository, NodeStatus, NodeUpdate,
};
pub use runs::{NewRun, RunMode, RunQuery, RunRecord, RunRepository, RunStatus};
pub use ui_layout::{UiLayoutRecord, UiLayoutRepository, WorkspacePresetRecord};
