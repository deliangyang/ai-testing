//! Run evidence persistence (ADR-004).
//!
//! The scheduler reports every node through a [`RunRecorder`] when it starts and when it
//! reaches a terminal status; values from the process environment are already masked. Nodes
//! inside subworkflows are reported under their frame path (`<subworkflow node>/<node>`).
//! [`StoreRecorder`] writes the reports to `node_executions`, and [`load_results`] rebuilds
//! the per-node results of a run from it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use common::{Clock, IdGen};
use serde_json::Value;
use store::{
    Db, ExecutionKey, Failure, FailureRecordRepository, NewFailure, NewRun,
    NodeExecutionRepository, NodeUpdate, RunRecord, RunRepository, RunStatus, StoreError,
};

use crate::executor::{ExecError, ExecErrorKind};
use crate::scheduler::{NodeResult, RunOutcome};
use crate::state::NodeStatus;

/// Retries are not implemented yet; every node runs once.
pub const FIRST_ATTEMPT: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeRecord {
    /// The node moved to `RUNNING`. `input` is its rendered configuration, `None` when a
    /// template could not be rendered (the node then fails with a template error).
    Started { node: String, input: Option<Value> },
    /// The node reached a terminal status, whether or not it was started.
    Finished { node: String, result: NodeResult },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot record run evidence: {0}")]
pub struct RecordError(pub String);

impl From<StoreError> for RecordError {
    fn from(e: StoreError) -> Self {
        Self(e.to_string())
    }
}

/// Called from a blocking thread, one record at a time per scheduler, in the order the
/// scheduler observed the transitions.
pub trait RunRecorder: Send + Sync {
    fn record(&self, run_id: &str, record: &NodeRecord) -> Result<(), RecordError>;
}

/// Maps a node failure to the classification every `FAILED` row needs.
pub trait FailureClassifier: Send + Sync {
    fn classify(&self, error: &ExecError) -> Failure;
}

/// Writes run evidence to SQLite.
///
/// `output_json` holds the node output for `COMPLETED` rows and the serialized [`ExecError`]
/// for rows that ended with an error (`FAILED`, or `SKIPPED` after an interruption). Every
/// `FAILED` row is classified and gets a matching `failure_records` row in the same
/// transaction.
pub struct StoreRecorder {
    db: Arc<Mutex<Db>>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGen>,
    classifier: Arc<dyn FailureClassifier>,
}

impl StoreRecorder {
    pub fn new(
        db: Arc<Mutex<Db>>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGen>,
        classifier: Arc<dyn FailureClassifier>,
    ) -> Self {
        Self {
            db,
            clock,
            ids,
            classifier,
        }
    }

    /// Creates the `dag_runs` row and moves it to `RUNNING`.
    pub fn begin_run(&self, run: &NewRun) -> Result<RunRecord, RecordError> {
        let db = self.lock()?;
        let runs = RunRepository::new(db.conn(), self.clock.as_ref(), self.ids.as_ref());
        let created = runs.create(run)?;
        Ok(runs.transition(&created.id, RunStatus::Running)?)
    }

    /// Moves the run to its final status.
    pub fn finish_run(&self, run_id: &str, outcome: &RunOutcome) -> Result<RunRecord, RecordError> {
        let db = self.lock()?;
        let runs = RunRepository::new(db.conn(), self.clock.as_ref(), self.ids.as_ref());
        Ok(runs.transition(run_id, outcome.status())?)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Db>, RecordError> {
        self.db
            .lock()
            .map_err(|_| RecordError("store lock poisoned".to_owned()))
    }
}

impl RunRecorder for StoreRecorder {
    fn record(&self, run_id: &str, record: &NodeRecord) -> Result<(), RecordError> {
        let db = self.lock()?;
        let repo = NodeExecutionRepository::new(db.conn(), self.clock.as_ref());
        match record {
            NodeRecord::Started { node, input } => {
                let key = ExecutionKey::new(run_id, node.as_str(), FIRST_ATTEMPT);
                repo.insert(&key, NodeStatus::Running, input.as_ref())?;
            }
            NodeRecord::Finished { node, result } => {
                let key = ExecutionKey::new(run_id, node.as_str(), FIRST_ATTEMPT);
                let output = match (&result.error, result.status) {
                    (_, NodeStatus::Completed) => result.output.clone(),
                    (Some(error), _) => Some(error_json(error)?),
                    (None, _) => None,
                };
                let missing = ExecError::new(ExecErrorKind::Internal, "failed without an error");
                let error = result.error.as_ref().unwrap_or(&missing);
                let failure =
                    (result.status == NodeStatus::Failed).then(|| self.classifier.classify(error));

                let tx = db
                    .conn()
                    .unchecked_transaction()
                    .map_err(StoreError::from)?;
                if repo.get(&key)?.is_none() {
                    repo.insert(&key, NodeStatus::Pending, None)?;
                }
                repo.update(
                    &key,
                    &NodeUpdate {
                        status: result.status,
                        output,
                        poll_trace: result.trace.clone(),
                        failure: failure.clone(),
                    },
                )?;
                if let Some(failure) = failure {
                    FailureRecordRepository::new(db.conn(), self.clock.as_ref(), self.ids.as_ref())
                        .insert(&NewFailure {
                            run_id: run_id.to_owned(),
                            node_id: Some(node.clone()),
                            failure_category: failure.category,
                            error_code: Some(failure.error_code),
                            raw_message: error.message.clone(),
                        })?;
                }
                tx.commit().map_err(StoreError::from)?;
            }
        }
        Ok(())
    }
}

fn error_json(error: &ExecError) -> Result<Value, RecordError> {
    serde_json::to_value(error).map_err(|e| RecordError(e.to_string()))
}

/// Rebuilds the latest attempt of every node of a run, keyed by frame path, as recorded by
/// [`StoreRecorder`].
pub fn load_results(
    db: &Db,
    clock: &dyn Clock,
    run_id: &str,
) -> Result<BTreeMap<String, NodeResult>, RecordError> {
    let rows = NodeExecutionRepository::new(db.conn(), clock).list_for_run(run_id)?;
    let mut results = BTreeMap::new();
    // Rows come ordered by node then attempt, so later attempts overwrite earlier ones.
    for row in rows {
        let (output, error) = match (row.status, row.output) {
            (NodeStatus::Completed, output) => (output, None),
            (_, Some(json)) => {
                let error: ExecError = serde_json::from_value(json).map_err(|e| {
                    RecordError(format!("{}: stored error is malformed: {e}", row.key))
                })?;
                (None, Some(error))
            }
            (_, None) => (None, None),
        };
        results.insert(
            row.key.node_id,
            NodeResult {
                status: row.status,
                output,
                error,
                trace: row.poll_trace,
            },
        );
    }
    Ok(results)
}
