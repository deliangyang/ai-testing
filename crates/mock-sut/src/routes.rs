use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use serde_json::{json, Value};

use crate::state::{OrderStatus, SharedState};

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/orders", post(create_order))
        .route("/orders/{id}/pay", post(pay_order))
        .route("/orders/{id}", get(get_order))
        .route("/stock/{sku}", get(stock))
        .route("/price/{sku}", get(price))
        .with_state(state)
}

async fn maybe_fault(state: &SharedState) -> Option<Response> {
    if state.faults().internal_error {
        return Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "injected internal_error"})),
            )
                .into_response(),
        );
    }
    None
}

async fn create_order(State(state): State<SharedState>) -> Response {
    if let Some(resp) = maybe_fault(&state).await {
        return resp;
    }
    let id = state.create_order().await;
    let body: Value = if state.faults().corrupt_create_field {
        json!({ "orderId": id })
    } else {
        json!({ "id": id })
    };
    (StatusCode::OK, Json(body)).into_response()
}

async fn pay_order(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = maybe_fault(&state).await {
        return resp;
    }
    if state.get_order(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "unknown order"})),
        )
            .into_response();
    }
    state.mark_paid_later(id).await;
    (StatusCode::OK, Json(json!({"paid": true}))).into_response()
}

async fn get_order(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = maybe_fault(&state).await {
        return resp;
    }
    let Some(order) = state.get_order(&id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "unknown order"})),
        )
            .into_response();
    };
    let status = if state.faults().corrupt_status_field {
        "READY".to_owned()
    } else {
        match order.status {
            OrderStatus::Pending => "PENDING".to_owned(),
            OrderStatus::Paid => "PAID".to_owned(),
        }
    };
    (StatusCode::OK, Json(json!({ "id": id, "status": status }))).into_response()
}

async fn stock(State(state): State<SharedState>, Path(sku): Path<String>) -> Response {
    if let Some(resp) = maybe_fault(&state).await {
        return resp;
    }
    (
        StatusCode::OK,
        Json(json!({"sku": sku, "left": state.config.stock_left})),
    )
        .into_response()
}

async fn price(State(state): State<SharedState>, Path(sku): Path<String>) -> Response {
    if let Some(resp) = maybe_fault(&state).await {
        return resp;
    }
    (
        StatusCode::OK,
        Json(json!({"sku": sku, "amount": state.config.price_amount})),
    )
        .into_response()
}
