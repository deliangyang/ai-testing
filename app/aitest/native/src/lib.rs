//! `aitest/native` — NSH UI set backing `DataSource` for the desktop shell.
//!
//! Build with the NSH CLI (`nsh build` / `nsh test`), which enables the `nsh` feature.
//! Without the CLI, this crate is a stub; integration coverage lives in `nsh-bridge` tests.

pub use nsh_bridge::datasource;

#[cfg(feature = "nsh")]
mod nsh_host;

#[cfg(feature = "nsh")]
use nsh_core::prelude::*;

#[cfg(feature = "nsh")]
#[native_module("aitest/native")]
fn native(registry: &mut Registry) {
    nsh_host::register(registry);
}

#[cfg(not(feature = "nsh"))]
/// Documented stub when the NSH toolchain is not linked.
pub const STUB: &str = "Build app/aitest via `nsh` (see app/README.md).";
