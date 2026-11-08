//! HTTP stand-in for a small shop API used by acceptance examples and drift drills.

mod faults;
mod routes;
mod state;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
pub use faults::Faults;
use state::SharedState;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Returned base URL points at nothing listening; TCP connect fails immediately.
pub fn connection_refused_base_url() -> String {
    "http://127.0.0.1:1".to_owned()
}

/// Configuration for payment latency and catalogue fixtures.
#[derive(Clone, Debug)]
pub struct MockSutConfig {
    pub listen: SocketAddr,
    pub pay_delay: Duration,
    pub jitter_ms: u64,
    pub seed: u64,
    pub stock_left: u64,
    pub price_amount: u64,
    pub faults: Faults,
}

impl Default for MockSutConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 0)),
            pay_delay: Duration::from_millis(80),
            jitter_ms: 40,
            seed: 42,
            stock_left: 3,
            price_amount: 10,
            faults: Faults::default(),
        }
    }
}

/// Builder for [`MockSutConfig`].
#[derive(Clone, Debug, Default)]
pub struct MockSutBuilder {
    config: MockSutConfig,
}

impl MockSutBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn listen(mut self, addr: SocketAddr) -> Self {
        self.config.listen = addr;
        self
    }

    #[must_use]
    pub fn pay_delay(mut self, delay: Duration) -> Self {
        self.config.pay_delay = delay;
        self
    }

    #[must_use]
    pub fn pay_delay_ms(self, millis: u64) -> Self {
        self.pay_delay(Duration::from_millis(millis))
    }

    #[must_use]
    pub fn jitter_ms(mut self, millis: u64) -> Self {
        self.config.jitter_ms = millis;
        self
    }

    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    #[must_use]
    pub fn stock_left(mut self, left: u64) -> Self {
        self.config.stock_left = left;
        self
    }

    #[must_use]
    pub fn price_amount(mut self, amount: u64) -> Self {
        self.config.price_amount = amount;
        self
    }

    #[must_use]
    pub fn faults(mut self, faults: Faults) -> Self {
        self.config.faults = faults;
        self
    }

    #[must_use]
    pub fn config(self) -> MockSutConfig {
        self.config
    }

    /// Binds an ephemeral port unless `Faults::refuse_listen` is set (see [`Faults`]).
    pub async fn spawn(self) -> anyhow::Result<MockSutHandle> {
        let config = self.config;
        if config.faults.refuse_listen {
            anyhow::bail!(
                "refuse_listen fault: call connection_refused_base_url() instead of spawning"
            );
        }
        MockSutHandle::spawn(config).await
    }
}

/// Running instance; safe to drop (best-effort shutdown).
pub struct MockSutHandle {
    base_url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_task: JoinHandle<()>,
}

impl MockSutHandle {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn spawn(config: MockSutConfig) -> anyhow::Result<Self> {
        let state = SharedState::new(config.clone());
        let app = routes::router(state);
        let listener = tokio::net::TcpListener::bind(config.listen).await?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server_task = tokio::spawn(async move {
            let shutdown = async move {
                let _ = shutdown_rx.await;
            };
            if axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
                .is_err()
            {
                tracing::warn!("mock-sut server exited with error");
            }
        });

        Ok(Self {
            base_url,
            shutdown_tx: Some(shutdown_tx),
            server_task,
        })
    }

    /// Stop the listener and wait for the accept loop to exit.
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.server_task.await?;
        Ok(())
    }
}

/// Build an axum router for embedding in custom servers (tests only).
pub fn router(config: MockSutConfig) -> Router {
    routes::router(SharedState::new(config))
}
