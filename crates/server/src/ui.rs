use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use store::UiLayoutRepository;

use crate::dto::{UiLayoutDto, WorkspacePresetDto};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PutUiLayoutBody {
    pub layout: Value,
}

pub async fn get_ui_layout(
    State(state): State<AppState>,
    Path(scope_key): Path<String>,
) -> ApiResult<Json<UiLayoutDto>> {
    let state = state.clone();
    let key = scope_key.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| UiLayoutRepository::new(db.conn(), state.clock.as_ref()).get(&key))
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("layout '{scope_key}' not found")))?;
    Ok(Json(record.into()))
}

pub async fn put_ui_layout(
    State(state): State<AppState>,
    Path(scope_key): Path<String>,
    Json(body): Json<PutUiLayoutBody>,
) -> ApiResult<Json<UiLayoutDto>> {
    let state = state.clone();
    let key = scope_key.clone();
    let layout = body.layout.clone();
    let record = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| {
            UiLayoutRepository::new(db.conn(), state.clock.as_ref()).upsert(&key, &layout)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(record.into()))
}

pub async fn list_workspace_presets(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<WorkspacePresetDto>>> {
    let state = state.clone();
    let rows = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| UiLayoutRepository::new(db.conn(), state.clock.as_ref()).list_presets())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn get_workspace_preset(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<WorkspacePresetDto>> {
    let state = state.clone();
    let preset_id = id.clone();
    let row = tokio::task::spawn_blocking(move || {
        state.with_locked_db(|db| UiLayoutRepository::new(db.conn(), state.clock.as_ref()).get_preset(&preset_id))
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("workspace preset '{id}' not found")))?;
    Ok(Json(row.into()))
}
