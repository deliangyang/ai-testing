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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// One line per issue plus a summary.
    Text,
    /// Canonical JSON.
    Json,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate and manage DAG definitions.
    #[command(subcommand)]
    Dag(DagCommand),
    /// Import API definitions into the catalog.
    #[command(subcommand)]
    Api(ApiCommand),
    /// Run a DAG against an environment, or inspect a recorded run.
    Run(RunArgs),
    /// Validate, store and activate Skill packages.
    #[command(subcommand)]
    Skill(SkillCommand),
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// Parse a file and store normalized API specs as drafts.
    Import {
        /// Input file for the selected `--format`.
        file: PathBuf,
        /// Source format (`md` uses cassette replay; set `AITEST_CASSETTE_DIR` or `--cassette-dir`).
        #[arg(long, value_enum)]
        format: crate::api::ImportFormat,
        /// How imported specs combine with existing catalog rows.
        #[arg(long, value_enum, default_value_t = crate::api::ImportStrategy::Merge)]
        strategy: crate::api::ImportStrategy,
        /// Directory of LLM cassettes (required for `--format md`).
        #[arg(long, env = "AITEST_CASSETTE_DIR")]
        cassette_dir: Option<PathBuf>,
        /// Cassette model id for markdown import.
        #[arg(long, default_value = "cassette-md-plain-json")]
        md_model: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum DagCommand {
    /// Check a DAG file with the deterministic gate; exits 3 when it is rejected.
    Validate {
        /// DAG file (`.json` is read as JSON, anything else as YAML).
        file: PathBuf,
        /// Report format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Store a DAG file as a new pending version; refused (exit 3) unless it passes the gate.
    Save {
        /// DAG file (`.json` is read as JSON, anything else as YAML).
        file: PathBuf,
    },
    /// Re-check a stored version with the gate and make it the active one.
    Activate {
        /// DAG id.
        id: String,
        /// Version to activate.
        #[arg(id = "dag_version", value_name = "VERSION")]
        version: u32,
    },
    /// List stored DAGs and their versions.
    List {
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// List AI-generated DAG versions awaiting review (`created_by=ai`, `pending`).
    Pending {
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Store a Skill file as a new pending revision; refused (exit 3) unless its internal DAG
    /// passes the standard gate.
    Save {
        /// Skill file (`.json` is read as JSON, anything else as YAML).
        file: PathBuf,
    },
    /// Re-check the internal DAG with the gate and make this revision active.
    Activate {
        /// Skill id (`manifest.id`).
        id: String,
        /// Activation revision (not the manifest semver).
        #[arg(id = "skill_revision", value_name = "REVISION")]
        version: u32,
    },
    /// List stored Skills and their revisions.
    List {
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
pub enum RunAction {
    /// Print recorded request, response, assertions and poll traces.
    Show {
        /// Run id printed by `mytool run`.
        run_id: String,
        /// Restrict output to this node (frame path, e.g. `login/session`).
        #[arg(long)]
        node: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(subcommand)]
    pub action: Option<RunAction>,

    /// Stored DAG id (its active version) or a DAG file. A file must pass the gate and is
    /// recorded as a pending version first (ADR-005). Required unless `show` is used.
    #[arg(long)]
    pub dag: Option<String>,

    /// Environment name, loaded from `<env-root>/environments/<name>.yaml`.
    #[arg(long)]
    pub env: Option<String>,

    /// Directory holding the `environments/` folder.
    #[arg(long, default_value = ".")]
    pub env_root: PathBuf,

    /// Maximum number of nodes running at once.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..))]
    pub parallel: u16,

    /// Never call a language model (the only mode so far).
    #[arg(long)]
    pub no_llm: bool,

    /// Write the canonical run report to this file.
    #[arg(long)]
    pub report_json: Option<PathBuf>,

    /// Reuse node outputs from this recorded run and only execute `--from-node` and its
    /// descendants. Implies the DAG and environment of that run unless overridden.
    #[arg(long, requires = "from_node")]
    pub from_run: Option<String>,

    /// Entry node for a partial rerun; requires `--from-run`.
    #[arg(long, requires = "from_run")]
    pub from_node: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn command_definition_is_consistent() {
        super::Cli::command().debug_assert();
    }
}
