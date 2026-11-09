//! Resolving DAG `connection` names to stored [`DbConnectionRecord`]s.

use std::sync::{Arc, Mutex};

use store::{ConnectionRepository, Db, DbConnectionRecord};

pub trait ConnectionResolver: Send + Sync {
    fn resolve(&self, env_name: &str, logical_name: &str) -> Result<DbConnectionRecord, String>;
}

pub struct StoreConnectionResolver {
    db: Arc<Mutex<Db>>,
}

impl StoreConnectionResolver {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self { db }
    }
}

impl ConnectionResolver for StoreConnectionResolver {
    fn resolve(&self, env_name: &str, logical_name: &str) -> Result<DbConnectionRecord, String> {
        let db = self
            .db
            .lock()
            .map_err(|_| "store lock poisoned".to_owned())?;
        let repo = ConnectionRepository::new(db.conn());
        repo.resolve_for_environment(env_name, logical_name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no connection '{logical_name}' in environment '{env_name}'"))
    }
}
