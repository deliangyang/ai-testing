//! Opening the database and exposing stored DAGs to the gate.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{anyhow, Context as _};
use common::clock::{Clock, SystemClock};
use engine::subworkflow::{DagSource, StoreDagSource};
use gate::rules::{ApiSpecLookup, ApiSpecLookupHandle, DagCatalog};
use gate::{Gate, Report};
use model::nodes::DagRef;
use model::Dag;
use serde_json::Value;
use store::ApiSpecRepository;
use store::Db;

/// The database shared by the gate's catalog, the engine and the command itself.
pub struct Store {
    pub db: Arc<Mutex<Db>>,
    pub clock: Arc<dyn Clock>,
}

impl Store {
    /// Creates the parent directory of `path` when needed.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let db = Db::open(path, clock.as_ref())
            .with_context(|| format!("cannot open database {}", path.display()))?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            clock,
        })
    }

    pub fn lock(&self) -> anyhow::Result<MutexGuard<'_, Db>> {
        self.db.lock().map_err(|_| anyhow!("store lock poisoned"))
    }

    /// Save/activation gate (catalog + API specs), same rules as the server.
    pub fn for_save_gate(&self) -> Gate {
        let catalog = StoreCatalog(StoreDagSource::new(self.db.clone(), self.clock.clone()));
        let api_specs: ApiSpecLookupHandle = Arc::new(StoreApiSpecLookup {
            db: self.db.clone(),
            clock: self.clock.clone(),
        });
        Gate::for_save(Arc::new(catalog), Some(api_specs))
    }

    /// The gate used before anything is stored or activated. The catalog takes the lock per
    /// lookup, so no guard may be held while this runs.
    pub fn gate(&self, dag: &Dag) -> Report {
        self.for_save_gate().validate(dag)
    }

    pub fn dag_source(&self) -> StoreDagSource {
        StoreDagSource::new(self.db.clone(), self.clock.clone())
    }
}

/// Resolves references the way a run does, so a DAG that passes here can also be loaded when
/// it runs: pending versions are invisible.
struct StoreCatalog(StoreDagSource);

impl DagCatalog for StoreCatalog {
    fn resolve(&self, dag: &DagRef) -> Option<Dag> {
        self.0.load(dag).ok().map(|(dag, _)| dag)
    }
}

struct StoreApiSpecLookup {
    db: Arc<Mutex<Db>>,
    clock: Arc<dyn Clock>,
}

impl ApiSpecLookup for StoreApiSpecLookup {
    fn response_schema(&self, id: &str) -> Option<Value> {
        let guard = self.db.lock().ok()?;
        let repo = ApiSpecRepository::new(guard.conn(), self.clock.as_ref());
        repo.get_active(id)
            .ok()
            .flatten()
            .and_then(|r| r.spec.response_schema)
    }
}
