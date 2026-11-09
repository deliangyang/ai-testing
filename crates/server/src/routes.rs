use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use store::{DagRepository, DagStatus, NodeExecutionRepository, RunQuery, RunRepository};

use crate::api_import::{commit_api_import, preview_api_import};
use crate::dag_repair::repair_dag_with_ai;
use crate::dto::{DagDetailDto, DagVersionDto, NodeExecutionDto, RunDto};
use crate::error::{ApiError, ApiResult};
use crate::sse::run_event_stream;
use crate::state::AppState;
use crate::ui::{get_ui_layout, get_workspace_preset, list_workspace_presets, put_ui_layout};
use crate::write::{
    activate_dag, cancel_run, save_ai_candidate, save_dag, start_run, validate_dag,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/dags", get(list_dags).post(save_dag))
        .route("/api/dags/pending", get(list_pending_ai_dags))
        .route("/api/dags/candidates", post(save_ai_candidate))
        .route("/api/dags/repair-ai", post(repair_dag_with_ai))
        .route("/api/dags/validate", post(validate_dag))
        .route("/api/dags/{dag_id}/active", get(get_active_dag))
        .route(
            "/api/dags/{dag_id}/versions/{version}",
            get(get_dag_version),
        )
        .route(
            "/api/dags/{dag_id}/versions/{version}/activate",
            post(activate_dag),
        )
        .route("/api/runs", get(list_runs).post(start_run))
        .route("/api/runs/{run_id}", get(get_run))
        .route("/api/runs/{run_id}/cancel", post(cancel_run))
        .route("/api/runs/{run_id}/executions", get(list_executions))
        .route("/api/runs/{run_id}/events", get(run_event_stream))
        .route(
            "/api/ui/layouts/{scope_key}",
            get(get_ui_layout).put(put_ui_layout),
        )
        .route("/api/ui/presets", get(list_workspace_presets))
        .route("/api/ui/presets/{id}", get(get_workspace_preset))
        .route("/api/api-specs/import/preview", post(preview_api_import))
        .route("/api/api-specs/import/commit", post(commit_api_import))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct ListDagsQuery {
    status: Option<String>,
    created_by: Option<String>,
}

async fn list_dags(
    State(state): State<AppState>,
    Query(q): Query<ListDagsQuery>,
) -> ApiResult<Json<Vec<DagVersionDto>>> {
    let state = state.clone();
    let status = q.status.as_deref().and_then(parse_dag_status);
    let created_by = q.created_by;
    let rows = tokio::task::spawn_blocking(move || {
        let created_by = created_by.as_deref();
        state.with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref()).list_filtered(status, created_by)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn list_pending_ai_dags(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<DagVersionDto>>> {
    let state = state.clone();
    let rows = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref()).list_pending_ai()
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

fn parse_dag_status(s: &str) -> Option<DagStatus> {
    s.parse().ok()
}

async fn get_active_dag(
    State(state): State<AppState>,
    Path(dag_id): Path<String>,
) -> ApiResult<Json<DagDetailDto>> {
    let state = state.clone();
    let id = dag_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| DagRepository::new(db.conn(), state.clock.as_ref()).active(&id))
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("DAG '{dag_id}' has no active version")))?;
    Ok(Json(
        DagDetailDto::from_record(record).map_err(|e| ApiError::Internal(e.to_string()))?,
    ))
}

async fn get_dag_version(
    State(state): State<AppState>,
    Path((dag_id, version)): Path<(String, u32)>,
) -> ApiResult<Json<DagDetailDto>> {
    let state = state.clone();
    let id = dag_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref()).get(&id, version)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("DAG '{dag_id}' version {version}")))?;
    Ok(Json(
        DagDetailDto::from_record(record).map_err(|e| ApiError::Internal(e.to_string()))?,
    ))
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
        state.with_locked_db(|db| {
            RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref()).list(&query)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

fn parse_run_status(s: &str) -> Option<store::RunStatus> {
    use store::RunStatus;
    RunStatus::ALL.into_iter().find(|st| st.as_str() == s)
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<RunDto>> {
    let state = state.clone();
    let id = run_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            RunRepository::new(db.conn(), state.clock.as_ref(), state.ids.as_ref()).get(&id)
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
        state.with_locked_db(|db| {
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
