//! Synchronous store reads aligned with `server` HTTP handlers.

use gate::Gate;
use model::Dag;
use server::{AppState, DagDetailDto, DagVersionDto, GateResponse, NodeExecutionDto, RunDto};
use store::{DagRepository, NodeExecutionRepository, RunQuery, RunRepository};

use crate::BridgeError;

pub fn list_dags(state: &AppState) -> Result<Vec<DagVersionDto>, BridgeError> {
    let rows =
        state.with_locked_db(|db| DagRepository::new(db.conn(), state.clock.as_ref()).list())?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn get_active_dag(state: &AppState, dag_id: &str) -> Result<DagDetailDto, BridgeError> {
    let record = state
        .with_locked_db(|db| DagRepository::new(db.conn(), state.clock.as_ref()).active(dag_id))?
        .ok_or_else(|| {
            store::StoreError::NotFound(format!("DAG '{dag_id}' has no active version"))
        })?;
    Ok(DagDetailDto::from_record(record).map_err(|e| store::StoreError::Invalid(e.to_string()))?)
}

pub fn get_dag_version(
    state: &AppState,
    dag_id: &str,
    version: u32,
) -> Result<DagDetailDto, BridgeError> {
    let record = state
        .with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref()).get(dag_id, version)
        })?
        .ok_or_else(|| store::StoreError::NotFound(format!("DAG '{dag_id}' version {version}")))?;
    Ok(DagDetailDto::from_record(record).map_err(|e| store::StoreError::Invalid(e.to_string()))?)
}

pub fn list_runs(state: &AppState, query: &RunQuery) -> Result<Vec<RunDto>, BridgeError> {
    let rows = state.with_locked_db(|db| {
        RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref()).list(query)
    })?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn get_run(state: &AppState, run_id: &str) -> Result<RunDto, BridgeError> {
    let record = state
        .with_locked_db(|db| {
            RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref()).get(run_id)
        })?
        .ok_or_else(|| store::StoreError::NotFound(format!("run '{run_id}'")))?;
    Ok(record.into())
}

pub fn list_executions(
    state: &AppState,
    run_id: &str,
) -> Result<Vec<NodeExecutionDto>, BridgeError> {
    let rows = state.with_locked_db(|db| {
        let runs = RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref());
        if runs.get(run_id)?.is_none() {
            return Err(store::StoreError::NotFound(format!("run '{run_id}'")));
        }
        let nodes = NodeExecutionRepository::new(db.conn(), state.clock.as_ref());
        nodes.list_for_run(run_id)
    })?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn validate_dag(dag: &Dag) -> GateResponse {
    Gate::standard().validate(dag).into()
}
