use axum::extract::{Path, State};
use axum::Json;
use model::Dag;
use serde::{Deserialize, Serialize};
use store::DagRepository;

use crate::catalog::{save_gate, validate_gate};
use crate::dto::{DagVersionDto, GateResponse};
use crate::error::{ApiError, ApiResult};
use crate::run_control::{spawn_run, CREATED_BY};
use crate::state::AppState;

pub async fn validate_dag(
    State(_state): State<AppState>,
    Json(dag): Json<Dag>,
) -> Json<GateResponse> {
    Json(validate_gate(&dag).into())
}

pub async fn save_dag(
    State(state): State<AppState>,
    Json(dag): Json<Dag>,
) -> ApiResult<Json<DagVersionDto>> {
    let report = save_gate(&state.db, &state.clock, &dag);
    if !report.passed() {
        return Err(ApiError::Gate(report));
    }
    let state = state.clone();
    let saved = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            DagRepository::new(db.conn(), state.clock.as_ref()).save(&dag, CREATED_BY)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(DagVersionDto::from(saved.info)))
}

pub async fn activate_dag(
    State(state): State<AppState>,
    Path((dag_id, version)): Path<(String, u32)>,
) -> ApiResult<Json<DagVersionDto>> {
    let dag = {
        let state = state.clone();
        let id = dag_id.clone();
        tokio::task::spawn_blocking(move || {
            state.with_locked_db(|db| {
                DagRepository::new(db.conn(), state.clock.as_ref())
                    .get(&id, version)?
                    .ok_or_else(|| {
                        store::StoreError::NotFound(format!("DAG '{id}' version {version}"))
                    })
                    .map(|r| r.dag)
            })
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))??
    };
    let report = save_gate(&state.db, &state.clock, &dag);
    if !report.passed() {
        return Err(ApiError::Gate(report));
    }
    let state = state.clone();
    let id = dag_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            let repo = DagRepository::new(db.conn(), state.clock.as_ref());
            let previous = repo.active_version(&id)?;
            repo.activate(&id, version, previous)?;
            repo.get(&id, version)?
                .ok_or_else(|| store::StoreError::NotFound(format!("DAG '{id}' version {version}")))
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(DagVersionDto::from(record.info)))
}

#[derive(Debug, Deserialize)]
pub struct CreateRunBody {
    pub dag_id: String,
    pub env: String,
    #[serde(default = "default_parallel")]
    pub parallel: u32,
}

fn default_parallel() -> u32 {
    4
}

#[derive(Debug, Serialize)]
pub struct CreateRunResponse {
    pub run_id: String,
}

pub async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<CreateRunBody>,
) -> ApiResult<Json<CreateRunResponse>> {
    let run_id = spawn_run(
        state,
        crate::run_control::StartRunRequest {
            dag_id: body.dag_id,
            env: body.env,
            parallel: body.parallel as usize,
        },
    )
    .map_err(ApiError::BadRequest)?;
    Ok(Json(CreateRunResponse { run_id }))
}

#[derive(Debug, Serialize)]
pub struct CancelRunResponse {
    pub cancelled: bool,
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<CancelRunResponse>> {
    let cancelled = state.runs.cancel(&run_id);
    if !cancelled {
        return Err(ApiError::NotFound(format!(
            "run '{run_id}' is not running on this server"
        )));
    }
    Ok(Json(CancelRunResponse { cancelled: true }))
}
