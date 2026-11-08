use std::path::PathBuf;
use std::sync::Arc;

use common::clock::Clock;
use common::id::IdGen;
use store::{Db, Result as StoreResult};

use crate::event_hub::RunEventHub;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub ids: Arc<dyn IdGen + Send + Sync>,
    pub hub: Arc<RunEventHub>,
}

impl AppState {
    pub fn new(
        db_path: PathBuf,
        clock: Arc<dyn Clock + Send + Sync>,
        ids: Arc<dyn IdGen + Send + Sync>,
        hub: Arc<RunEventHub>,
    ) -> Self {
        Self {
            db_path,
            clock,
            ids,
            hub,
        }
    }

    pub fn with_db<T>(&self, f: impl FnOnce(&Db) -> StoreResult<T>) -> StoreResult<T> {
        let db = Db::open(&self.db_path, self.clock.as_ref())?;
        f(&db)
    }
}
