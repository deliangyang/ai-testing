use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use common::{SystemClock, UuidV7Gen};
use server::{AppState, RunEventHub};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// Local API for the Web UI.
#[derive(Parser, Debug)]
#[command(name = "aitest-server")]
struct Args {
    #[arg(long, env = "AITEST_DB", default_value = "aitest.db")]
    db: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8790")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let state = AppState::new(
        args.db,
        Arc::new(SystemClock),
        Arc::new(UuidV7Gen),
        Arc::new(RunEventHub::new()),
    );
    let app = server::router(state).layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .context("bind")?;
    tracing::info!(%args.listen, "aitest-server listening");
    axum::serve(listener, app).await.context("serve")?;
    Ok(())
}
