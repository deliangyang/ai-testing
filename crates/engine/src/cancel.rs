//! Run cancellation.

use std::fmt;
use std::sync::Arc;

use tokio::sync::watch;

/// Shared cancellation flag for one run, including its subworkflows. Clones observe the same
/// flag; cancelling is permanent.
#[derive(Clone)]
pub struct CancelToken {
    tx: Arc<watch::Sender<bool>>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(watch::Sender::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.tx.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.tx.borrow()
    }

    /// Resolves once the token is cancelled; immediately if it already is.
    pub async fn cancelled(&self) {
        let mut rx = self.tx.subscribe();
        // The sender lives in `self`, so the channel cannot close while we wait.
        let _ = rx.wait_for(|cancelled| *cancelled).await;
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}
