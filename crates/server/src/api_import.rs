//! HTTP API for interface import preview and commit (develop.md §6.1).

use std::path::PathBuf;

use axum::extract::State;
use axum::Json;
use importer::{
    build_import_preview, parse_import_text, persist_confirmed_import, ImportCommitItem,
    ImportFormat, ImportPipelineError, ImportPreviewResponse, MergeStrategy,
};
use serde::Deserialize;
use store::{ApiSpecRepository, StoreError};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const API_IMPORT_CREATED_BY: &str = "api-import-ui";

#[derive(Debug, Deserialize)]
pub struct ImportPreviewRequest {
    pub format: ImportFormatDto,
    pub content: String,
    pub strategy: Option<MergeStrategyDto>,
    #[serde(default)]
    pub markdown_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportCommitRequest {
    pub strategy: MergeStrategyDto,
    pub items: Vec<ImportCommitItem>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportFormatDto {
    Openapi,
    Postman,
    Curl,
    Har,
    Md,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategyDto {
    Merge,
    Overwrite,
    Skip,
}

impl From<ImportFormatDto> for ImportFormat {
    fn from(value: ImportFormatDto) -> Self {
        match value {
            ImportFormatDto::Openapi => ImportFormat::OpenApi,
            ImportFormatDto::Postman => ImportFormat::Postman,
            ImportFormatDto::Curl => ImportFormat::Curl,
            ImportFormatDto::Har => ImportFormat::Har,
            ImportFormatDto::Md => ImportFormat::Markdown,
        }
    }
}

impl From<MergeStrategyDto> for MergeStrategy {
    fn from(value: MergeStrategyDto) -> Self {
        match value {
            MergeStrategyDto::Merge => MergeStrategy::Merge,
            MergeStrategyDto::Overwrite => MergeStrategy::Overwrite,
            MergeStrategyDto::Skip => MergeStrategy::Skip,
        }
    }
}

fn cassette_dir() -> Result<Option<PathBuf>, ApiError> {
    match std::env::var("AITEST_CASSETTE_DIR") {
        Ok(dir) if !dir.trim().is_empty() => Ok(Some(PathBuf::from(dir))),
        Ok(_) => Err(ApiError::BadRequest(
            "AITEST_CASSETTE_DIR must not be empty for markdown import".into(),
        )),
        Err(_) => Ok(None),
    }
}

fn map_pipeline_error(err: ImportPipelineError) -> ApiError {
    ApiError::BadRequest(err.to_string())
}

pub async fn preview_api_import(
    State(state): State<AppState>,
    Json(body): Json<ImportPreviewRequest>,
) -> ApiResult<Json<ImportPreviewResponse>> {
    let format = body.format.into();
    let strategy = body.strategy.unwrap_or(MergeStrategyDto::Merge).into();
    let cassette = cassette_dir()?;
    let model = body
        .markdown_model
        .unwrap_or_else(|| "cassette-md-plain-json".into());
    let content = body.content;
    let state = state.clone();
    let preview = tokio::task::spawn_blocking(move || -> ApiResult<ImportPreviewResponse> {
        let drafts = parse_import_text(format, &content, cassette.as_deref(), &model)
            .map_err(map_pipeline_error)?;
        state
            .with_locked_db(|db| {
                let repo = ApiSpecRepository::new(db.conn(), state.clock.as_ref());
                build_import_preview(&repo, &drafts, strategy).map_err(pipeline_to_store)
            })
            .map_err(ApiError::from)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(preview))
}

pub async fn commit_api_import(
    State(state): State<AppState>,
    Json(body): Json<ImportCommitRequest>,
) -> ApiResult<Json<importer::ImportPersistSummary>> {
    let strategy = body.strategy.into();
    let items = body.items;
    let state = state.clone();
    let summary =
        tokio::task::spawn_blocking(move || -> ApiResult<importer::ImportPersistSummary> {
            state
                .with_locked_db(|db| {
                    let repo = ApiSpecRepository::new(db.conn(), state.clock.as_ref());
                    persist_confirmed_import(&repo, &items, strategy, API_IMPORT_CREATED_BY)
                        .map_err(pipeline_to_store)
                })
                .map_err(ApiError::from)
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(Json(summary))
}

fn pipeline_to_store(err: ImportPipelineError) -> StoreError {
    StoreError::Invalid(err.to_string())
}
