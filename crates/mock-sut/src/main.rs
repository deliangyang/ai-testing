use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use mock_sut::{connection_refused_base_url, Faults, MockSutBuilder};
use tracing_subscriber::EnvFilter;

/// Minimal shop API for local DAG runs and classifier drills.
#[derive(Parser, Debug)]
#[command(name = "mock-sut")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8787")]
    listen: SocketAddr,
    #[arg(long, default_value_t = 80)]
    pay_delay_ms: u64,
    #[arg(long, default_value_t = 40)]
    jitter_ms: u64,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 3)]
    stock_left: u64,
    #[arg(long, default_value_t = 10)]
    price_amount: u64,
    #[arg(long)]
    fault_internal_error: bool,
    #[arg(long)]
    fault_corrupt_create_field: bool,
    #[arg(long)]
    fault_corrupt_status_field: bool,
    #[arg(long)]
    fault_refuse_listen: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    if args.fault_refuse_listen {
        eprintln!(
            "refuse_listen: no server started; clients should use {}",
            connection_refused_base_url()
        );
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }

    let faults = Faults {
        refuse_listen: false,
        internal_error: args.fault_internal_error,
        corrupt_create_field: args.fault_corrupt_create_field,
        corrupt_status_field: args.fault_corrupt_status_field,
    };

    let handle = MockSutBuilder::new()
        .listen(args.listen)
        .pay_delay(Duration::from_millis(args.pay_delay_ms))
        .jitter_ms(args.jitter_ms)
        .seed(args.seed)
        .stock_left(args.stock_left)
        .price_amount(args.price_amount)
        .faults(faults)
        .spawn()
        .await
        .context("start mock-sut")?;

    tracing::info!(base_url = %handle.base_url(), "mock-sut listening");
    println!("{}", handle.base_url());

    tokio::signal::ctrl_c().await?;
    handle.shutdown().await?;
    Ok(())
}
