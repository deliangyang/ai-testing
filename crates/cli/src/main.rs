//! `mytool`: the deterministic command-line entry point.

mod args;
mod exit;

use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;

use args::{Cli, Command, DagCommand};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = common::init_tracing(cli.log_level.filter()) {
        eprintln!("error: {e}");
        return ExitCode::from(exit::ERROR);
    }
    match dispatch(&cli) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(exit::ERROR)
        }
    }
}

fn dispatch(cli: &Cli) -> anyhow::Result<u8> {
    let name = match &cli.command {
        Command::Dag(DagCommand::Validate { .. }) => "dag validate",
        Command::Dag(DagCommand::Save { .. }) => "dag save",
        Command::Dag(DagCommand::Activate { .. }) => "dag activate",
        Command::Dag(DagCommand::List) => "dag list",
        Command::Run(_) => "run",
    };
    bail!("`{name}` is not implemented yet")
}
