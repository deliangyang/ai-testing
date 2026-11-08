//! Runtime fault injection for classifier and drift drills.

use serde::{Deserialize, Serialize};

/// Toggleable misbehaviour. All fields are deterministic from configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faults {
    /// Do not call [`crate::MockSutBuilder::spawn`]; use [`crate::connection_refused_base_url`].
    pub refuse_listen: bool,
    /// Every shop route responds with HTTP 500.
    pub internal_error: bool,
    /// `POST /orders` returns `orderId` instead of `id`.
    pub corrupt_create_field: bool,
    /// `GET /orders/:id` returns status `READY` instead of `PENDING`/`PAID`.
    pub corrupt_status_field: bool,
}

impl Faults {
    #[must_use]
    pub fn internal_error() -> Self {
        Self {
            internal_error: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn corrupt_create_field() -> Self {
        Self {
            corrupt_create_field: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn corrupt_status_field() -> Self {
        Self {
            corrupt_status_field: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn refuse_listen() -> Self {
        Self {
            refuse_listen: true,
            ..Self::default()
        }
    }
}
