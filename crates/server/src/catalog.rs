//! Gate catalog backed by the same SQLite store as runs (mirrors `cli::Store`).

use std::sync::{Arc, Mutex};

use common::clock::Clock;
use engine::subworkflow::{DagSource, StoreDagSource};
use gate::rules::DagCatalog;
use gate::{Gate, Report};
use model::nodes::DagRef;
use model::Dag;
use store::Db;

struct StoreCatalog(StoreDagSource);

impl DagCatalog for StoreCatalog {
    fn resolve(&self, dag: &DagRef) -> Option<Dag> {
        self.0.load(dag).ok().map(|(dag, _)| dag)
    }
}

pub fn save_gate(db: &Arc<Mutex<Db>>, clock: &Arc<dyn Clock>, dag: &Dag) -> Report {
    let catalog = StoreCatalog(StoreDagSource::new(Arc::clone(db), Arc::clone(clock)));
    Gate::for_save(Arc::new(catalog)).validate(dag)
}

pub fn validate_gate(dag: &Dag) -> Report {
    Gate::standard().validate(dag)
}
