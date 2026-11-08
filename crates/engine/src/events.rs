//! Live engine events for the CLI and the UI.
//!
//! Events describe progress as it happens; persisted evidence (ADR-004) stays the source of
//! truth for results. Within one scheduler, events are emitted in the order the transitions
//! happened; node ids are frame paths (`<subworkflow node>/<node>`), and values that came from
//! the process environment are masked.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use store::RunStatus;
use tokio::sync::mpsc;

use crate::state::NodeStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    NodeStateChanged {
        node: String,
        status: NodeStatus,
    },
    /// One poll attempt; `entry` is the trace entry of that attempt.
    PollIteration {
        node: String,
        attempt: u32,
        entry: Value,
    },
    Log {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        node: Option<String>,
        level: LogLevel,
        message: String,
    },
    /// Last event of a top-level run.
    RunFinished {
        #[serde(serialize_with = "ser_status", deserialize_with = "de_status")]
        status: RunStatus,
    },
}

/// Sending half of an event channel. Emitting never blocks the engine; events are dropped once
/// the receiver is gone.
#[derive(Debug, Clone)]
pub struct EventSink(mpsc::UnboundedSender<EngineEvent>);

impl EventSink {
    pub fn emit(&self, event: EngineEvent) {
        let _ = self.0.send(event);
    }
}

pub fn channel() -> (EventSink, mpsc::UnboundedReceiver<EngineEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (EventSink(tx), rx)
}

fn ser_status<S: Serializer>(status: &RunStatus, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(status.as_str())
}

fn de_status<'de, D: Deserializer<'de>>(d: D) -> Result<RunStatus, D::Error> {
    let text = String::deserialize(d)?;
    text.parse().map_err(serde::de::Error::custom)
}
