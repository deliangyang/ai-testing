use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use common::clock::Clock;
use common::id::IdGen;
use store::{Db, Result as StoreResult, StoreError};

use crate::event_hub::RunEventHub;
use crate::run_control::RunRegistry;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub db: Arc<Mutex<Db>>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGen>,
    pub hub: Arc<RunEventHub>,
    pub env_root: PathBuf,
    pub runs: Arc<RunRegistry>,
}

impl AppState {
    pub fn open(
        db_path: PathBuf,
        env_root: PathBuf,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGen>,
        hub: Arc<RunEventHub>,
    ) -> Result<Self, StoreError> {
        if let Some(dir) = db_path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| StoreError::Invalid(e.to_string()))?;
        }
        let db = Db::open(&db_path, clock.as_ref())?;
        Ok(Self {
            db_path,
            db: Arc::new(Mutex::new(db)),
            clock,
            ids,
            hub,
            env_root,
            runs: Arc::new(RunRegistry::default()),
        })
    }

    pub fn with_locked_db<T>(&self, f: impl FnOnce(&Db) -> StoreResult<T>) -> StoreResult<T> {
        let guard = self
            .db
            .lock()
            .map_err(|_| StoreError::Invalid("store lock poisoned".into()))?;
        f(&guard)
    }
}
