#![allow(clippy::unwrap_used)]

use std::time::Duration;

use common::clock::{FixedClock, Timestamp};
use common::id::SeqIdGen;
use model::Dag;
use store::{
    DagRepository, Db, FailureRecordRepository, NewFailure, NewRun, RunMode, RunRepository,
    StoreError,
};

struct Fixture {
    db: Db,
    clock: FixedClock,
    ids: SeqIdGen,
    runs: Vec<String>,
}

impl Fixture {
    fn new() -> Self {
        let clock = FixedClock::new(Timestamp::from_unix_millis(0));
        let db = Db::open_in_memory(&clock).unwrap();
        let dag = Dag::from_json_str(
            r#"{"id": "d", "nodes": [{"id": "a", "type": "http", "method": "GET", "path": "/"}]}"#,
        )
        .unwrap();
        DagRepository::new(db.conn(), &clock)
            .save(&dag, "t")
            .unwrap();
        let run_ids = SeqIdGen::new("run");
        let runs = (0..2)
            .map(|_| {
                RunRepository::new(db.conn(), &clock, &run_ids)
                    .create(&NewRun {
                        dag_id: "d".into(),
                        dag_version: 1,
                        env_name: "dev".into(),
                        mode: RunMode::Ci,
                    })
                    .unwrap()
                    .id
            })
            .collect();
        Self {
            db,
            clock,
            ids: SeqIdGen::new("fail"),
            runs,
        }
    }

    fn repo(&self) -> FailureRecordRepository<'_> {
        FailureRecordRepository::new(self.db.conn(), &self.clock, &self.ids)
    }

    fn record(&self, run: usize, node: Option<&str>, category: &str) -> String {
        self.clock.advance(Duration::from_millis(1));
        self.repo()
            .insert(&NewFailure {
                run_id: self.runs[run].clone(),
                node_id: node.map(str::to_owned),
                failure_category: category.into(),
                error_code: Some(format!("{}_E", category.to_uppercase())),
                raw_message: "connection reset".into(),
            })
            .unwrap()
            .id
    }
}

#[test]
fn insert_stamps_id_and_time() {
    let f = Fixture::new();
    let id = f.record(0, Some("a"), "network");
    let rows = f.repo().list_for_run(&f.runs[0]).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, id);
    assert_eq!(row.id, "fail-000001");
    assert_eq!(row.node_id.as_deref(), Some("a"));
    assert_eq!(row.error_code.as_deref(), Some("NETWORK_E"));
    assert_eq!(row.classified_at, Timestamp::from_unix_millis(1));
    assert_eq!(row.resolution, None);
}

#[test]
fn queries_by_run_and_by_category() {
    let f = Fixture::new();
    let a = f.record(0, Some("a"), "network");
    let b = f.record(1, Some("a"), "assertion");
    let c = f.record(0, None, "environment");
    let d = f.record(1, Some("b"), "network");

    let ids = |rows: Vec<store::FailureRecord>| -> Vec<String> {
        rows.into_iter().map(|r| r.id).collect()
    };
    assert_eq!(
        ids(f.repo().list_for_run(&f.runs[0]).unwrap()),
        vec![a.clone(), c]
    );
    assert_eq!(
        ids(f.repo().list_for_run(&f.runs[1]).unwrap()),
        vec![b, d.clone()]
    );
    assert_eq!(
        ids(f.repo().list_by_category("network", 10).unwrap()),
        vec![d.clone(), a]
    );
    assert_eq!(
        ids(f.repo().list_by_category("network", 1).unwrap()),
        vec![d]
    );
    assert!(f.repo().list_by_category("timeout", 10).unwrap().is_empty());
}

#[test]
fn rejects_unknown_runs_and_empty_categories() {
    let f = Fixture::new();
    let new = |run: &str, category: &str| NewFailure {
        run_id: run.into(),
        node_id: None,
        failure_category: category.into(),
        error_code: None,
        raw_message: "x".into(),
    };
    let err = f.repo().insert(&new("ghost", "network")).unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");
    let err = f.repo().insert(&new(&f.runs[0], " ")).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");
}
