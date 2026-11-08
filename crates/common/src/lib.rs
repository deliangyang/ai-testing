//! Shared foundations: error conventions, tracing setup, and injectable Clock/Sleeper/IdGen.
//!
//! Error convention: library crates define concrete error enums with `thiserror`; `anyhow` is
//! reserved for binary entry points (`cli`, `server`).

pub mod clock;
pub mod id;
pub mod telemetry;

pub use clock::{
    Clock, FixedClock, ManualSleeper, SleepFuture, Sleeper, SystemClock, Timestamp,
    TimestampParseError, TokioSleeper,
};
pub use id::{IdGen, SeqIdGen, UuidV7Gen};
pub use telemetry::{init_tracing, TelemetryError};
