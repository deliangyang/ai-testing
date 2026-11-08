//! Background `server` listener for WebView + `HttpDataSource`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use common::{SystemClock, UuidV7Gen};
use server::{router, AppState, RunEventHub};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tower_http::trace::TraceLayer;

use crate::BridgeError;

/// Running embedded API; keeps the Tokio runtime thread alive.
pub struct EmbeddedApi {
    pub state: AppState,
    pub base_url: String,
    #[allow(dead_code)]
    join: JoinHandle<()>,
}

impl EmbeddedApi {
    pub fn spawn(
        db_path: PathBuf,
        env_root: PathBuf,
        bind: SocketAddr,
    ) -> Result<Self, BridgeError> {
        if let Some(dir) = db_path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| store::StoreError::Invalid(e.to_string()))?;
        }
        let clock = Arc::new(SystemClock);
        let ids = Arc::new(UuidV7Gen);
        let hub = Arc::new(RunEventHub::new());
        let state = AppState::open(db_path, env_root, clock, ids, hub)?;
        let (ready_tx, ready_rx) = oneshot::channel();
        let app_state = state.clone();
        let join = thread::Builder::new()
            .name("nsh-bridge-embed".into())
            .spawn(move || {
                let Ok(rt) = Runtime::new() else {
                    return;
                };
                rt.block_on(async move {
                    let app = router(app_state).layer(TraceLayer::new_for_http());
                    let Ok(listener) = tokio::net::TcpListener::bind(bind).await else {
                        return;
                    };
                    let Ok(addr) = listener.local_addr() else {
                        return;
                    };
                    let _ = ready_tx.send(addr);
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("embedded server stopped: {e}");
                    }
                });
            })
            .map_err(|e| store::StoreError::Invalid(e.to_string()))?;
        let bound = ready_rx
            .blocking_recv()
            .map_err(|_| store::StoreError::Invalid("embedded server failed to start".into()))?;
        let base_url = format!("http://{bound}");
        Ok(Self {
            state,
            base_url,
            join,
        })
    }

    /// Listen on `127.0.0.1:0` (ephemeral port).
    pub fn spawn_ephemeral(db_path: PathBuf, env_root: PathBuf) -> Result<Self, BridgeError> {
        Self::spawn(db_path, env_root, SocketAddr::from(([127, 0, 0, 1], 0)))
    }
}
