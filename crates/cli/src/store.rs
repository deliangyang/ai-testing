//! Opening the database and exposing stored DAGs to the gate.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{anyhow, Context as _};
use common::clock::{Clock, SystemClock};
use engine::subworkflow::{DagSource, StoreDagSource};
use gate::rules::DagCatalog;
use gate::{Gate, Report};
use model::nodes::DagRef;
use model::Dag;
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

    /// The gate used before anything is stored or activated. The catalog takes the lock per
    /// lookup, so no guard may be held while this runs.
    pub fn gate(&self, dag: &Dag) -> Report {
        let catalog = StoreCatalog(StoreDagSource::new(self.db.clone(), self.clock.clone()));
        Gate::for_save(Arc::new(catalog)).validate(dag)
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
