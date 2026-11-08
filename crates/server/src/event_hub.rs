//! Per-run buffering and live fan-out for [`engine::EngineEvent`] (SSE + reconnect replay).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use engine::EngineEvent;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub seq: u64,
    pub event: EngineEvent,
}

struct RunBuffer {
    next_seq: u64,
    history: Vec<StoredEvent>,
    live: broadcast::Sender<StoredEvent>,
}

impl RunBuffer {
    fn new() -> Self {
        let (live, _) = broadcast::channel(256);
        Self {
            next_seq: 1,
            history: Vec::new(),
            live,
        }
    }
}

/// In-memory event log per run. SQLite remains the evidence source; this is for live UI streaming.
#[derive(Default)]
pub struct RunEventHub {
    runs: RwLock<HashMap<String, Arc<RwLock<RunBuffer>>>>,
}

impl RunEventHub {
    pub fn new() -> Self {
        Self::default()
    }

    fn buffer(&self, run_id: &str) -> Arc<RwLock<RunBuffer>> {
        let mut guard = self.runs.write().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(run_id.to_owned())
            .or_insert_with(|| Arc::new(RwLock::new(RunBuffer::new())))
            .clone()
    }

    /// Assigns the next sequence number and stores the event for replay.
    pub fn push(&self, run_id: &str, event: EngineEvent) -> u64 {
        let buf = self.buffer(run_id);
        let mut inner = buf.write().unwrap_or_else(|e| e.into_inner());
        let seq = inner.next_seq;
        inner.next_seq += 1;
        let stored = StoredEvent {
            seq,
            event: event.clone(),
        };
        inner.history.push(stored.clone());
        let _ = inner.live.send(stored);
        seq
    }

    pub fn replay(&self, run_id: &str, after_seq: u64) -> Vec<StoredEvent> {
        let guard = self.runs.read().unwrap_or_else(|e| e.into_inner());
        let Some(buf) = guard.get(run_id) else {
            return Vec::new();
        };
        let inner = buf.read().unwrap_or_else(|e| e.into_inner());
        inner
            .history
            .iter()
            .filter(|e| e.seq > after_seq)
            .cloned()
            .collect()
    }

    pub fn subscribe(&self, run_id: &str) -> broadcast::Receiver<StoredEvent> {
        self.buffer(run_id)
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .live
            .subscribe()
    }
}

/// Copies every event from an engine channel into the hub (used when starting a run from the server).
pub fn forward_engine_events(
    hub: Arc<RunEventHub>,
    run_id: String,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<EngineEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            hub.push(&run_id, event);
        }
    });
}
