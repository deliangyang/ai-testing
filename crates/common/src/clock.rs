//! Injectable time sources.
//!
//! Every component that reads the wall clock or waits must go through [`Clock`] and
//! [`Sleeper`] so that runs can be replayed deterministically. This file is the only place
//! allowed to call the runtime's sleep (enforced by `scripts/check.ps1`).

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// UTC instant with millisecond precision, rendered as RFC 3339 (`2026-11-08T06:00:00.000Z`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub const fn unix_millis(self) -> i64 {
        self.0
    }

    /// Saturates instead of overflowing so a misconfigured timeout cannot panic.
    pub fn saturating_add(self, duration: Duration) -> Self {
        let millis = i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);
        Self(self.0.saturating_add(millis))
    }

    /// Elapsed time since `earlier`; zero if `earlier` is in the future.
    pub fn duration_since(self, earlier: Timestamp) -> Duration {
        let diff = self.0.saturating_sub(earlier.0);
        Duration::from_millis(u64::try_from(diff).unwrap_or(0))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid RFC 3339 timestamp '{input}': {reason}")]
pub struct TimestampParseError {
    input: String,
    reason: String,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nanos = i128::from(self.0) * 1_000_000;
        let dt = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| fmt::Error)?;
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            dt.year(),
            u8::from(dt.month()),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.millisecond()
        )
    }
}

impl FromStr for Timestamp {
    type Err = TimestampParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = |reason: String| TimestampParseError {
            input: s.to_owned(),
            reason,
        };
        let dt = OffsetDateTime::parse(s, &Rfc3339).map_err(|e| err(e.to_string()))?;
        let millis = dt.unix_timestamp_nanos() / 1_000_000;
        i64::try_from(millis)
            .map(Self)
            .map_err(|_| err("out of range".into()))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let nanos = OffsetDateTime::now_utc().unix_timestamp_nanos();
        Timestamp(i64::try_from(nanos / 1_000_000).unwrap_or(i64::MAX))
    }
}

/// Manually driven clock for tests. Clones share the same instant.
#[derive(Debug, Clone)]
pub struct FixedClock {
    millis: Arc<AtomicI64>,
}

impl FixedClock {
    pub fn new(start: Timestamp) -> Self {
        Self {
            millis: Arc::new(AtomicI64::new(start.0)),
        }
    }

    pub fn set(&self, at: Timestamp) {
        self.millis.store(at.0, Ordering::SeqCst);
    }

    pub fn advance(&self, by: Duration) {
        let millis = i64::try_from(by.as_millis()).unwrap_or(i64::MAX);
        self.millis.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.millis.load(Ordering::SeqCst))
    }
}

pub type SleepFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait Sleeper: Send + Sync {
    fn sleep(&self, duration: Duration) -> SleepFuture<'_>;
}

/// Real waiting on the tokio timer.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, duration: Duration) -> SleepFuture<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// Virtual-time sleeper: returns immediately, advances the paired [`FixedClock`] and records
/// every requested duration so tests can assert exact backoff sequences.
#[derive(Debug, Clone)]
pub struct ManualSleeper {
    clock: FixedClock,
    requests: Arc<Mutex<Vec<Duration>>>,
}

impl ManualSleeper {
    pub fn new(clock: FixedClock) -> Self {
        Self {
            clock,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn requests(&self) -> Vec<Duration> {
        self.requests
            .lock()
            .map(|r| r.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }
}

impl Sleeper for ManualSleeper {
    fn sleep(&self, duration: Duration) -> SleepFuture<'_> {
        match self.requests.lock() {
            Ok(mut r) => r.push(duration),
            Err(poisoned) => poisoned.into_inner().push(duration),
        }
        self.clock.advance(duration);
        Box::pin(std::future::ready(()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const START: Timestamp = Timestamp::from_unix_millis(1_794_117_600_000);

    #[test]
    fn timestamp_renders_rfc3339_with_millis() {
        assert_eq!(START.to_string(), "2026-11-08T06:00:00.000Z");
        assert_eq!(
            Timestamp::from_unix_millis(1_794_117_600_042).to_string(),
            "2026-11-08T06:00:00.042Z"
        );
    }

    #[test]
    fn timestamp_round_trips_through_string_and_serde() {
        let parsed: Timestamp = "2026-11-08T14:00:00.042+08:00".parse().unwrap();
        assert_eq!(parsed, START.saturating_add(Duration::from_millis(42)));

        let json = serde_json::to_string(&START).unwrap();
        assert_eq!(json, "\"2026-11-08T06:00:00.000Z\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), START);
    }

    #[test]
    fn timestamp_rejects_garbage() {
        assert!("yesterday".parse::<Timestamp>().is_err());
    }

    #[test]
    fn duration_since_is_never_negative() {
        let later = START.saturating_add(Duration::from_secs(3));
        assert_eq!(later.duration_since(START), Duration::from_secs(3));
        assert_eq!(START.duration_since(later), Duration::ZERO);
    }

    #[test]
    fn fixed_clock_advances_and_clones_share_state() {
        let clock = FixedClock::new(START);
        let other = clock.clone();
        clock.advance(Duration::from_millis(1500));
        assert_eq!(
            other.now(),
            START.saturating_add(Duration::from_millis(1500))
        );
        other.set(START);
        assert_eq!(clock.now(), START);
    }

    #[tokio::test]
    async fn manual_sleeper_advances_clock_and_records_requests() {
        let clock = FixedClock::new(START);
        let sleeper = ManualSleeper::new(clock.clone());
        for ms in [100, 200, 400] {
            sleeper.sleep(Duration::from_millis(ms)).await;
        }
        assert_eq!(
            clock.now(),
            START.saturating_add(Duration::from_millis(700))
        );
        assert_eq!(
            sleeper.requests(),
            vec![
                Duration::from_millis(100),
                Duration::from_millis(200),
                Duration::from_millis(400)
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn tokio_sleeper_waits_on_runtime_timer() {
        let before = tokio::time::Instant::now();
        TokioSleeper.sleep(Duration::from_secs(5)).await;
        assert!(before.elapsed() >= Duration::from_secs(5));
    }

    #[test]
    fn system_clock_is_after_2025() {
        assert!(SystemClock.now() > Timestamp::from_unix_millis(1_735_689_600_000));
    }
}
