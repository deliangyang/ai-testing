//! `mytool`: the deterministic command-line entry point.

mod args;
mod dag;
mod exit;
mod input;
mod report;
mod run;
mod store;

use std::process::ExitCode;

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
    match &cli.command {
        Command::Dag(DagCommand::Validate { file, format }) => dag::validate(file, *format),
        Command::Dag(DagCommand::Save { file }) => dag::save(&cli.db, file),
        Command::Dag(DagCommand::Activate { id, version }) => dag::activate(&cli.db, id, *version),
        Command::Dag(DagCommand::List { format }) => dag::list(&cli.db, *format),
        Command::Run(args) => match &args.action {
            Some(args::RunAction::Show {
                run_id,
                node,
                format,
            }) => run::show(&cli.db, run_id, node.as_deref(), *format),
            None => run::run(&cli.db, args),
        },
    }
}
