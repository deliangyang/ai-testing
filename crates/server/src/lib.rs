//! Read-only HTTP API over the local SQLite store (phase 1).

mod dto;
mod error;
mod event_hub;
mod routes;
mod sse;
mod state;

pub use error::ApiError;
pub use event_hub::{forward_engine_events, RunEventHub};
pub use routes::router;
pub use state::AppState;
