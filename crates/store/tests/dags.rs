#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::{Arc, Barrier};

use common::clock::{FixedClock, Timestamp};
use model::Dag;
use store::{DagRepository, DagStatus, Db, StoreError};

fn clock() -> FixedClock {
    FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000))
}

fn dag(id: &str, path: &str) -> Dag {
    Dag::from_json_str(&format!(
        r#"{{"id": "{id}", "nodes": [{{"id": "a", "type": "http", "method": "GET", "path": "{path}"}}]}}"#
    ))
    .unwrap()
}

fn statuses(repo: &DagRepository<'_>, id: &str) -> Vec<(u32, DagStatus)> {
    repo.versions(id)
        .unwrap()
        .into_iter()
        .map(|v| (v.version, v.status))
        .collect()
}

#[test]
fn save_assigns_increasing_versions_per_dag() {
    let clock = clock();
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);

    let first = repo.save(&dag("orders", "/v1"), "alice").unwrap();
    clock.advance(std::time::Duration::from_secs(1));
    let second = repo.save(&dag("orders", "/v2"), "bob").unwrap();
    let other = repo.save(&dag("users", "/u"), "alice").unwrap();

    assert_eq!(
        (first.info.version, second.info.version, other.info.version),
        (1, 2, 1)
    );
    assert_eq!(second.info.status, DagStatus::Pending);
    assert_eq!(second.info.created_by, "bob");
    assert_eq!(
        second.info.created_at.to_string(),
        "2023-11-14T22:13:21.000Z"
    );
    assert_eq!(second.dag.version, Some(2));

    let loaded = repo.get("orders", 1).unwrap().unwrap();
    assert_eq!(loaded, first);
    assert_eq!(repo.get("orders", 3).unwrap(), None);
}

#[test]
fn content_is_stored_as_canonical_json_without_version() {
    let clock = clock();
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);
    let mut input = dag("orders", "/v1");
    input.version = Some(42);
    repo.save(&input, "alice").unwrap();

    let stored: String = db
        .conn()
        .query_row("SELECT content_json FROM dag_definitions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        stored,
        r#"{"edges":[],"id":"orders","nodes":[{"id":"a","method":"GET","path":"/v1","type":"http"}]}"#
    );
}

#[test]
fn activation_archives_the_previous_version_which_stays_readable() {
    let clock = clock();
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);
    for p in ["/v1", "/v2", "/v3"] {
        repo.save(&dag("orders", p), "alice").unwrap();
    }
    assert_eq!(repo.active("orders").unwrap(), None);

    repo.activate("orders", 1, None).unwrap();
    repo.activate("orders", 2, Some(1)).unwrap();
    assert_eq!(
        statuses(&repo, "orders"),
        [
            (1, DagStatus::Archived),
            (2, DagStatus::Active),
            (3, DagStatus::Pending)
        ]
    );
    let active = repo.active("orders").unwrap().unwrap();
    assert_eq!(active.dag.nodes[0].id, "a");
    assert_eq!(active.info.version, 2);
    let old = repo.get("orders", 1).unwrap().unwrap();
    assert_eq!(old.info.status, DagStatus::Archived);

    // Re-activating the active version is a no-op; rolling back to an archived one works.
    repo.activate("orders", 2, Some(2)).unwrap();
    repo.activate("orders", 1, Some(2)).unwrap();
    assert_eq!(
        statuses(&repo, "orders"),
        [
            (1, DagStatus::Active),
            (2, DagStatus::Archived),
            (3, DagStatus::Pending)
        ]
    );
}

#[test]
fn stale_or_missing_activation_changes_nothing() {
    let clock = clock();
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);
    repo.save(&dag("orders", "/v1"), "alice").unwrap();
    repo.save(&dag("orders", "/v2"), "alice").unwrap();
    repo.activate("orders", 1, None).unwrap();

    let err = repo.activate("orders", 2, None).unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "{err}");
    let err = repo.activate("orders", 9, Some(1)).unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");
    let err = repo.activate("ghost", 1, None).unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");

    assert_eq!(
        statuses(&repo, "orders"),
        [(1, DagStatus::Active), (2, DagStatus::Pending)]
    );
}

#[test]
fn list_returns_every_version_ordered_by_id_then_version() {
    let clock = clock();
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);
    assert!(repo.list().unwrap().is_empty());
    repo.save(&dag("users", "/u"), "alice").unwrap();
    repo.save(&dag("orders", "/v1"), "alice").unwrap();
    repo.save(&dag("orders", "/v2"), "alice").unwrap();
    repo.activate("orders", 2, None).unwrap();

    let listed: Vec<(String, u32, DagStatus)> = repo
        .list()
        .unwrap()
        .into_iter()
        .map(|v| (v.id, v.version, v.status))
        .collect();
    assert_eq!(
        listed,
        [
            ("orders".to_owned(), 1, DagStatus::Pending),
            ("orders".to_owned(), 2, DagStatus::Active),
            ("users".to_owned(), 1, DagStatus::Pending),
        ]
    );
    assert_eq!(repo.active_version("orders").unwrap(), Some(2));
    assert_eq!(repo.active_version("users").unwrap(), None);
}

const THREADS: usize = 8;

/// Runs `f(db, thread_index)` on `THREADS` connections to the same file, released together.
fn race<T: Send + 'static>(
    path: &Path,
    f: impl Fn(&Db, usize) -> T + Send + Sync + 'static,
) -> Vec<T> {
    let f = Arc::new(f);
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let (path, f, barrier) = (path.to_owned(), f.clone(), barrier.clone());
            std::thread::spawn(move || {
                let db = Db::open(&path, &clock()).unwrap();
                barrier.wait();
                f(&db, i)
            })
        })
        .collect();
    handles.into_iter().map(|h| h.join().unwrap()).collect()
}

#[test]
fn concurrent_activations_let_exactly_one_win() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    {
        let clock = clock();
        let db = Db::open(&path, &clock).unwrap();
        let repo = DagRepository::new(db.conn(), &clock);
        for i in 0..THREADS {
            repo.save(&dag("orders", &format!("/v{i}")), "alice")
                .unwrap();
        }
    }

    let results = race(&path, |db, i| {
        let clock = clock();
        DagRepository::new(db.conn(), &clock).activate("orders", i as u32 + 1, None)
    });
    let wins = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(wins, 1, "{results:?}");
    assert!(results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|e| matches!(e, StoreError::Conflict(_))));

    let clock = clock();
    let db = Db::open(&path, &clock).unwrap();
    let repo = DagRepository::new(db.conn(), &clock);
    let active: Vec<_> = statuses(&repo, "orders")
        .into_iter()
        .filter(|(_, s)| *s == DagStatus::Active)
        .collect();
    assert_eq!(active.len(), 1);
}

#[test]
fn concurrent_saves_get_distinct_versions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    drop(Db::open(&path, &clock()).unwrap());

    let mut versions = race(&path, |db, i| {
        let clock = clock();
        DagRepository::new(db.conn(), &clock)
            .save(&dag("orders", &format!("/t{i}")), "alice")
            .unwrap()
            .info
            .version
    });
    versions.sort_unstable();
    assert_eq!(versions, (1..=THREADS as u32).collect::<Vec<_>>());
}
