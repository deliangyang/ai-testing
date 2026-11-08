use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use store::{DagRepository, NodeExecutionRepository, RunQuery, RunRepository};

use crate::dto::{DagDetailDto, DagVersionDto, NodeExecutionDto, RunDto};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/dags", get(list_dags))
        .route("/api/dags/{dag_id}/active", get(get_active_dag))
        .route("/api/dags/{dag_id}/versions/{version}", get(get_dag_version))
        .route("/api/runs", get(list_runs))
        .route("/api/runs/{run_id}", get(get_run))
        .route("/api/runs/{run_id}/executions", get(list_executions))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn list_dags(State(state): State<AppState>) -> ApiResult<Json<Vec<DagVersionDto>>> {
    let state = state.clone();
    let rows = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let repo = DagRepository::new(db.conn(), state.clock.as_ref());
            repo.list()
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn get_active_dag(
    State(state): State<AppState>,
    Path(dag_id): Path<String>,
) -> ApiResult<Json<DagDetailDto>> {
    let state = state.clone();
    let id = dag_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let repo = DagRepository::new(db.conn(), state.clock.as_ref());
            repo.active(&id)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("DAG '{dag_id}' has no active version")))?;
    Ok(Json(DagDetailDto::from_record(record).map_err(|e| ApiError::Internal(e.to_string()))?))
}

async fn get_dag_version(
    State(state): State<AppState>,
    Path((dag_id, version)): Path<(String, u32)>,
) -> ApiResult<Json<DagDetailDto>> {
    let state = state.clone();
    let id = dag_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let repo = DagRepository::new(db.conn(), state.clock.as_ref());
            repo.get(&id, version)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("DAG '{dag_id}' version {version}")))?;
    Ok(Json(DagDetailDto::from_record(record).map_err(|e| ApiError::Internal(e.to_string()))?))
}

#[derive(Debug, Deserialize)]
struct ListRunsQuery {
    dag_id: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

async fn list_runs(
    State(state): State<AppState>,
    Query(q): Query<ListRunsQuery>,
) -> ApiResult<Json<Vec<RunDto>>> {
    let state = state.clone();
    let query = RunQuery {
        dag_id: q.dag_id,
        status: q.status.as_deref().and_then(parse_run_status),
        limit: q.limit,
    };
    let rows = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let repo = RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref());
            repo.list(&query)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

fn parse_run_status(s: &str) -> Option<store::RunStatus> {
    use store::RunStatus;
    RunStatus::ALL
        .into_iter()
        .find(|st| st.as_str() == s)
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<RunDto>> {
    let state = state.clone();
    let id = run_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let repo = RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref());
            repo.get(&id)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("run '{run_id}'")))?;
    Ok(Json(record.into()))
}

async fn list_executions(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<Vec<NodeExecutionDto>>> {
    let state = state.clone();
    let rows = tokio::task::spawn_blocking(move || {
        state.with_db(|db| {
            let runs = RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref());
            if runs.get(&run_id)?.is_none() {
                return Err(store::StoreError::NotFound(format!("run '{run_id}'")));
            }
            let nodes = NodeExecutionRepository::new(db.conn(), state.clock.as_ref());
            nodes.list_for_run(&run_id)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}
