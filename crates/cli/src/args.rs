//! Command-line grammar.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "mytool",
    version,
    about = "Validate, store and deterministically run API test DAGs.",
    propagate_version = true
)]
pub struct Cli {
    /// SQLite database holding DAG versions and run evidence.
    #[arg(
        long,
        global = true,
        env = "AITEST_DB",
        default_value = ".aitest/aitest.db"
    )]
    pub db: PathBuf,

    /// Diagnostics on stderr; `RUST_LOG` overrides it.
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Warn)]
    pub log_level: LogLevel,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn filter(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate and manage DAG definitions.
    #[command(subcommand)]
    Dag(DagCommand),
    /// Run a DAG against an environment.
    Run(RunArgs),
}

#[derive(Debug, Subcommand)]
pub enum DagCommand {
    /// Check a DAG file with the deterministic gate.
    Validate {
        /// DAG file (YAML or JSON).
        file: PathBuf,
    },
    /// Validate a DAG file and store it as a new pending version.
    Save {
        /// DAG file (YAML or JSON).
        file: PathBuf,
    },
    /// Make a stored version the active one.
    Activate {
        /// DAG id.
        id: String,
        /// Version to activate.
        version: u32,
    },
    /// List stored DAGs and their versions.
    List,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Stored DAG id (its active version) or a DAG file.
    #[arg(long)]
    pub dag: String,

    /// Environment name, loaded from `environments/<name>.yaml`.
    #[arg(long)]
    pub env: String,

    /// Never call a language model (the only mode so far).
    #[arg(long)]
    pub no_llm: bool,

    /// Write the canonical run report to this file.
    #[arg(long)]
    pub report_json: Option<PathBuf>,
}
