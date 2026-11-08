//! Process-wide tracing setup for binaries (`cli`, `server`). Libraries only emit events.

use tracing_subscriber::EnvFilter;

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("invalid log filter '{filter}': {reason}")]
    InvalidFilter { filter: String, reason: String },
    #[error("tracing subscriber already initialised")]
    AlreadyInitialised,
}

/// Installs a stderr subscriber. `RUST_LOG` overrides `default_filter` when set.
pub fn init_tracing(default_filter: &str) -> Result<(), TelemetryError> {
    let filter = match std::env::var("RUST_LOG") {
        Ok(env) if !env.trim().is_empty() => parse_filter(&env)?,
        _ => parse_filter(default_filter)?,
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init()
        .map_err(|_| TelemetryError::AlreadyInitialised)
}

fn parse_filter(filter: &str) -> Result<EnvFilter, TelemetryError> {
    EnvFilter::try_new(filter).map_err(|e| TelemetryError::InvalidFilter {
        filter: filter.to_owned(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_filter() {
        assert!(matches!(
            parse_filter("engine=notalevel"),
            Err(TelemetryError::InvalidFilter { .. })
        ));
    }

    #[test]
    fn second_init_reports_already_initialised() {
        let first = init_tracing("warn");
        let second = init_tracing("warn");
        assert!(first.is_ok() || matches!(first, Err(TelemetryError::AlreadyInitialised)));
        assert!(matches!(second, Err(TelemetryError::AlreadyInitialised)));
    }
}
