#![allow(clippy::unwrap_used)]

use std::time::Duration;

use common::clock::{FixedClock, Timestamp};
use common::id::SeqIdGen;
use model::Dag;
use store::RunStatus::{Cancelled, Completed, Failed, Pending, Running};
use store::{DagRepository, Db, NewRun, RunMode, RunQuery, RunRepository, RunStatus, StoreError};

struct Fixture {
    db: Db,
    clock: FixedClock,
    ids: SeqIdGen,
}

impl Fixture {
    fn new() -> Self {
        let clock = FixedClock::new(Timestamp::from_unix_millis(0));
        let db = Db::open_in_memory(&clock).unwrap();
        let dags = DagRepository::new(db.conn(), &clock);
        for id in ["orders", "users"] {
            let dag = Dag::from_json_str(&format!(
                r#"{{"id": "{id}", "nodes": [{{"id": "a", "type": "http", "method": "GET", "path": "/"}}]}}"#
            ))
            .unwrap();
            dags.save(&dag, "t").unwrap();
        }
        Self {
            db,
            clock,
            ids: SeqIdGen::new("run"),
        }
    }

    fn runs(&self) -> RunRepository<'_> {
        RunRepository::new(self.db.conn(), &self.clock, &self.ids)
    }

    fn create(&self, dag_id: &str) -> String {
        self.runs()
            .create(&NewRun {
                dag_id: dag_id.into(),
                dag_version: 1,
                env_name: "dev".into(),
                mode: RunMode::Ci,
            })
            .unwrap()
            .id
    }

    /// A fresh run brought to `status` through legal transitions.
    fn run_in(&self, status: RunStatus) -> String {
        let id = self.create("orders");
        let path: &[RunStatus] = match status {
            Pending => &[],
            Running => &[Running],
            Completed => &[Running, Completed],
            Failed => &[Running, Failed],
            Cancelled => &[Cancelled],
        };
        for &step in path {
            self.runs().transition(&id, step).unwrap();
        }
        id
    }
}

#[test]
fn create_starts_pending_and_requires_an_existing_dag_version() {
    let f = Fixture::new();
    let id = f.create("orders");
    let run = f.runs().get(&id).unwrap().unwrap();
    assert_eq!(run.id, "run-000001");
    assert_eq!(run.status, Pending);
    assert_eq!(run.mode, RunMode::Ci);
    assert_eq!((run.started_at, run.finished_at), (None, None));

    let err = f
        .runs()
        .create(&NewRun {
            dag_id: "orders".into(),
            dag_version: 2,
            env_name: "dev".into(),
            mode: RunMode::Manual,
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");
}

#[test]
fn transition_matrix() {
    // (from, to, allowed)
    let allowed = [
        (Pending, Running),
        (Pending, Failed),
        (Pending, Cancelled),
        (Running, Completed),
        (Running, Failed),
        (Running, Cancelled),
    ];
    let f = Fixture::new();
    for from in RunStatus::ALL {
        for to in RunStatus::ALL {
            let expect_ok = allowed.contains(&(from, to));
            assert_eq!(from.can_transition_to(to), expect_ok, "{from} -> {to}");

            let id = f.run_in(from);
            let result = f.runs().transition(&id, to);
            match (expect_ok, result) {
                (true, Ok(run)) => assert_eq!(run.status, to),
                (false, Err(StoreError::InvalidTransition { from: a, to: b, .. })) => {
                    assert_eq!((a.as_str(), b.as_str()), (from.as_str(), to.as_str()));
                    assert_eq!(f.runs().get(&id).unwrap().unwrap().status, from);
                }
                (_, other) => panic!("{from} -> {to}: unexpected {other:?}"),
            }
        }
    }
}

#[test]
fn transitions_stamp_start_and_finish_times() {
    let f = Fixture::new();
    let id = f.create("orders");
    f.clock.advance(Duration::from_secs(1));
    let running = f.runs().transition(&id, Running).unwrap();
    assert_eq!(running.started_at, Some(Timestamp::from_unix_millis(1_000)));
    assert_eq!(running.finished_at, None);

    f.clock.advance(Duration::from_secs(2));
    let done = f.runs().transition(&id, Completed).unwrap();
    assert_eq!(done.started_at, Some(Timestamp::from_unix_millis(1_000)));
    assert_eq!(done.finished_at, Some(Timestamp::from_unix_millis(3_000)));

    let cancelled = f.runs().transition(&f.create("orders"), Cancelled).unwrap();
    assert_eq!(cancelled.started_at, None);
    assert_eq!(
        cancelled.finished_at,
        Some(Timestamp::from_unix_millis(3_000))
    );
}

#[test]
fn unknown_run_is_not_found() {
    let f = Fixture::new();
    assert_eq!(f.runs().get("ghost").unwrap(), None);
    let err = f.runs().transition("ghost", Running).unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");
}

#[test]
fn list_filters_and_orders_newest_first() {
    let f = Fixture::new();
    let a = f.create("orders");
    let b = f.create("users");
    let c = f.create("orders");
    f.runs().transition(&c, Running).unwrap();

    let ids = |q: RunQuery| -> Vec<String> {
        f.runs()
            .list(&q)
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    };
    assert_eq!(
        ids(RunQuery::default()),
        vec![c.clone(), b.clone(), a.clone()]
    );
    assert_eq!(
        ids(RunQuery {
            dag_id: Some("orders".into()),
            ..Default::default()
        }),
        vec![c.clone(), a.clone()]
    );
    assert_eq!(
        ids(RunQuery {
            dag_id: Some("orders".into()),
            status: Some(Pending),
            ..Default::default()
        }),
        vec![a.clone()]
    );
    assert_eq!(
        ids(RunQuery {
            limit: Some(1),
            ..Default::default()
        }),
        vec![c.clone()]
    );
}
