use std::path::PathBuf;
use std::sync::Arc;

use common::clock::Clock;
use common::id::IdGen;
use store::{Db, Result as StoreResult};

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub ids: Arc<dyn IdGen + Send + Sync>,
}

impl AppState {
    pub fn new(
        db_path: PathBuf,
        clock: Arc<dyn Clock + Send + Sync>,
        ids: Arc<dyn IdGen + Send + Sync>,
    ) -> Self {
        Self {
            db_path,
            clock,
            ids,
        }
    }

    pub fn with_db<T>(&self, f: impl FnOnce(&Db) -> StoreResult<T>) -> StoreResult<T> {
        let db = Db::open(&self.db_path, self.clock.as_ref())?;
        f(&db)
    }
}
