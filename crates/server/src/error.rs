use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use gate::Report;
use serde_json::json;
use store::StoreError;

use crate::dto::GateResponse;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("gate rejected")]
    Gate(Report),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let ApiError::Gate(report) = self {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(GateResponse::from(report)),
            )
                .into_response();
        }
        let (status, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Store(StoreError::NotFound(msg)) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Store(StoreError::Conflict(msg)) => (StatusCode::CONFLICT, msg.clone()),
            ApiError::Store(StoreError::Invalid(msg)) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Gate(_) => unreachable!(),
            ApiError::Store(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
