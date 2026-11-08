//! Opening the database and exposing stored DAGs to the gate.

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use common::clock::Clock;
use engine::subworkflow::{DagSource, StoreDagSource};
use gate::rules::DagCatalog;
use model::nodes::DagRef;
use model::Dag;
use store::Db;

pub fn open_db(path: &Path, clock: &dyn Clock) -> anyhow::Result<Db> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    Db::open(path, clock).with_context(|| format!("cannot open database {}", path.display()))
}

/// Resolves references the way a run does, so a DAG that passes here can also be loaded when
/// it runs: pending versions are invisible.
pub struct StoreCatalog(StoreDagSource);

impl StoreCatalog {
    pub fn new(db: Arc<Mutex<Db>>, clock: Arc<dyn Clock>) -> Self {
        Self(StoreDagSource::new(db, clock))
    }
}

impl DagCatalog for StoreCatalog {
    fn resolve(&self, dag: &DagRef) -> Option<Dag> {
        self.0.load(dag).ok().map(|(dag, _)| dag)
    }
}
