//! Process exit codes. Scripts and CI rely on them, so they never change meaning.

/// The command did what was asked; for `run`, every node completed.
pub const SUCCESS: u8 = 0;
/// The run finished with failed (or skipped) nodes.
pub const RUN_FAILED: u8 = 1;
/// Usage error (clap also exits with 2), I/O or store failure, or an internal error.
pub const ERROR: u8 = 2;
/// The input was rejected by the gate or is not a DAG.
pub const INVALID: u8 = 3;
/// The run was interrupted (Ctrl-C); 128 + SIGINT by shell convention.
pub const CANCELLED: u8 = 130;
