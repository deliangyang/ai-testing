//! Injectable identifier generation.

use std::sync::atomic::{AtomicU64, Ordering};

pub trait IdGen: Send + Sync {
    fn next_id(&self) -> String;
}

/// Time-ordered UUID v7, so ids sort by creation time in SQLite.
#[derive(Debug, Default, Clone, Copy)]
pub struct UuidV7Gen;

impl IdGen for UuidV7Gen {
    fn next_id(&self) -> String {
        uuid::Uuid::now_v7().to_string()
    }
}

/// Deterministic ids for tests: `<prefix>-000001`, `<prefix>-000002`, ...
#[derive(Debug)]
pub struct SeqIdGen {
    prefix: String,
    counter: AtomicU64,
}

impl SeqIdGen {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            counter: AtomicU64::new(0),
        }
    }
}

impl IdGen for SeqIdGen {
    fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        format!("{}-{n:06}", self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_id_gen_is_sequential_and_padded() {
        let ids = SeqIdGen::new("run");
        assert_eq!(ids.next_id(), "run-000001");
        assert_eq!(ids.next_id(), "run-000002");
    }

    #[test]
    fn seq_id_gen_is_unique_across_threads() {
        let ids = std::sync::Arc::new(SeqIdGen::new("n"));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let ids = ids.clone();
                std::thread::spawn(move || (0..250).map(|_| ids.next_id()).collect::<Vec<_>>())
            })
            .collect();
        let mut all: Vec<String> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 1000);
    }

    #[test]
    fn uuid_v7_ids_are_unique() {
        let a = UuidV7Gen.next_id();
        let b = UuidV7Gen.next_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
    }
}
