//! Local HTTP API over the SQLite store (phase 1).

mod catalog;
mod dto;
mod error;
mod event_hub;
mod routes;
mod run_control;
mod sse;
mod state;
mod ui;
mod write;

pub use error::ApiError;
pub use event_hub::{forward_engine_events, RunEventHub};
pub use routes::router;
pub use run_control::RunRegistry;
pub use state::AppState;
