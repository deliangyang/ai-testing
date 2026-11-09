//! AI-assisted DAG repair when gate validation fails (develop.md §5.2 / P2-23).

use std::path::PathBuf;

use axum::extract::State;
use axum::Json;
use generator::{
    repair_dag_with_gate, DagGenerateRequestOptions, DagGenerateWithGateOptions,
    GenerateDagWithGateResult,
};
use llm_gateway::{CassetteProvider, MessageRole};
use model::Dag;
use serde::Serialize;
use store::DagRepository;

use crate::catalog::save_gate;
use crate::dto::{DagVersionDto, GateResponse};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DagAiRepairResponse {
    pub status: &'static str,
    pub dag: Dag,
    pub gate: GateResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<DagVersionDto>,
}

fn cassette_dir() -> Result<PathBuf, ApiError> {
    std::env::var("AITEST_CASSETTE_DIR")
        .map(PathBuf::from)
        .map_err(|_| {
            ApiError::BadRequest(
                "AI repair requires AITEST_CASSETTE_DIR or a configured LLM provider".into(),
            )
        })
}

fn repair_model() -> String {
    std::env::var("AITEST_LLM_MODEL").unwrap_or_else(|_| "cassette-dag-repair-pay".into())
}

pub async fn repair_dag_with_ai(
    State(state): State<AppState>,
    Json(dag): Json<Dag>,
) -> ApiResult<Json<DagAiRepairResponse>> {
    let dir = cassette_dir()?;
    let provider =
        CassetteProvider::from_dir(&dir).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let options = DagGenerateWithGateOptions {
        llm: DagGenerateRequestOptions {
            model: repair_model(),
            role: MessageRole::User,
        },
        max_rounds: 3,
    };
    let result = repair_dag_with_gate(&provider, &dag, &options)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    match result {
        GenerateDagWithGateResult::Accepted { dag: fixed, .. } => {
            let report = save_gate(&state.db, &state.clock, &fixed);
            if !report.passed() {
                return Err(ApiError::Gate(report));
            }
            let state = state.clone();
            let to_save = fixed.clone();
            let saved = tokio::task::spawn_blocking(move || {
                state.with_locked_db(|db| {
                    DagRepository::new(db.conn(), state.clock.as_ref()).save_ai_candidate(&to_save)
                })
            })
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??;
            Ok(Json(DagAiRepairResponse {
                status: "accepted",
                dag: fixed,
                gate: GateResponse::from(report),
                pending: Some(DagVersionDto::from(saved.info)),
            }))
        }
        GenerateDagWithGateResult::NeedsManualReview { dag, report, .. } => {
            Ok(Json(DagAiRepairResponse {
                status: "needs_manual_review",
                dag,
                gate: GateResponse::from(report),
                pending: None,
            }))
        }
    }
}
