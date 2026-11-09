//! Gate catalog backed by the same SQLite store as runs (mirrors `cli::Store`).

use std::sync::{Arc, Mutex};

use common::clock::Clock;
use engine::subworkflow::{DagSource, StoreDagSource};
use gate::rules::{ApiSpecLookup, ApiSpecLookupHandle, DagCatalog};
use gate::{Gate, Report};
use model::nodes::DagRef;
use model::Dag;
use serde_json::Value;
use store::ApiSpecRepository;
use store::Db;

struct StoreCatalog(StoreDagSource);

impl DagCatalog for StoreCatalog {
    fn resolve(&self, dag: &DagRef) -> Option<Dag> {
        self.0.load(dag).ok().map(|(dag, _)| dag)
    }
}

pub fn for_save_gate(db: &Arc<Mutex<Db>>, clock: &Arc<dyn Clock>) -> Gate {
    let catalog = StoreCatalog(StoreDagSource::new(Arc::clone(db), Arc::clone(clock)));
    let api_specs: ApiSpecLookupHandle = Arc::new(StoreApiSpecLookup {
        db: Arc::clone(db),
        clock: Arc::clone(clock),
    });
    Gate::for_save(Arc::new(catalog), Some(api_specs))
}

pub fn save_gate(db: &Arc<Mutex<Db>>, clock: &Arc<dyn Clock>, dag: &Dag) -> Report {
    for_save_gate(db, clock).validate(dag)
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

pub fn validate_gate(dag: &Dag) -> Report {
    Gate::standard().validate(dag)
}
