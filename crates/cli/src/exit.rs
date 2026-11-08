//! Process exit codes. Scripts and CI rely on them, so they never change meaning.

/// Usage error (clap also exits with 2), I/O or store failure, or an internal error.
pub const ERROR: u8 = 2;
