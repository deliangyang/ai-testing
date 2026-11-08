//! Read-only HTTP API over the local SQLite store (phase 1).

mod dto;
mod error;
mod routes;
mod state;

pub use error::ApiError;
pub use routes::router;
pub use state::AppState;
