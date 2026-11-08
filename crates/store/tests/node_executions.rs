#![allow(clippy::unwrap_used)]

use std::time::Duration;

use common::clock::{FixedClock, Timestamp};
use common::id::SeqIdGen;
use model::Dag;
use serde_json::json;
use store::NodeStatus::{Completed, Failed, Pending, Polling, Ready, Running, Skipped};
use store::{
    DagRepository, Db, ExecutionKey, Failure, NewRun, NodeExecutionRepository, NodeStatus,
    NodeUpdate, RunMode, RunRepository, StoreError,
};

struct Fixture {
    db: Db,
    clock: FixedClock,
    run_id: String,
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
        let ids = SeqIdGen::new("run");
        let run_id = RunRepository::new(db.conn(), &clock, &ids)
            .create(&NewRun {
                dag_id: "d".into(),
                dag_version: 1,
                env_name: "dev".into(),
                mode: RunMode::Ci,
            })
            .unwrap()
            .id;
        Self { db, clock, run_id }
    }

    fn repo(&self) -> NodeExecutionRepository<'_> {
        NodeExecutionRepository::new(self.db.conn(), &self.clock)
    }

    fn key(&self, node: &str, attempt: u32) -> ExecutionKey {
        ExecutionKey::new(&self.run_id, node, attempt)
    }
}

fn failure() -> Failure {
    Failure {
        category: "assertion".into(),
        error_code: "ASSERT_EQ".into(),
    }
}

fn update_to(status: NodeStatus) -> NodeUpdate {
    NodeUpdate {
        failure: (status == Failed).then(failure),
        ..NodeUpdate::status(status)
    }
}

#[test]
fn records_a_full_attempt_with_timestamps_and_json() {
    let f = Fixture::new();
    let key = f.key("a", 1);
    let input = json!({"method": "GET", "headers": {"authorization": "****"}});
    let row = f.repo().insert(&key, Ready, Some(&input)).unwrap();
    assert_eq!((row.status, row.started_at), (Ready, None));

    f.clock.advance(Duration::from_millis(10));
    let row = f.repo().update(&key, &NodeUpdate::status(Running)).unwrap();
    assert_eq!(row.started_at, Some(Timestamp::from_unix_millis(10)));

    f.clock.advance(Duration::from_millis(5));
    let trace = json!([{"attempt": 1, "status": 202}]);
    f.repo()
        .update(
            &key,
            &NodeUpdate {
                poll_trace: Some(trace.clone()),
                ..NodeUpdate::status(Polling)
            },
        )
        .unwrap();
    let row = f
        .repo()
        .update(
            &key,
            &NodeUpdate {
                output: Some(json!({"status": 200, "body": {"state": "done"}})),
                ..NodeUpdate::status(Completed)
            },
        )
        .unwrap();
    assert_eq!(row.status, Completed);
    assert_eq!(row.input_resolved, Some(input));
    assert_eq!(row.poll_trace, Some(trace));
    assert_eq!(row.output.unwrap()["body"]["state"], "done");
    assert_eq!(row.started_at, Some(Timestamp::from_unix_millis(10)));
    assert_eq!(row.finished_at, Some(Timestamp::from_unix_millis(15)));
    assert_eq!(row.failure, None);
}

#[test]
fn terminal_rows_cannot_be_modified() {
    let f = Fixture::new();
    for (i, terminal) in [Completed, Failed, Skipped].into_iter().enumerate() {
        let key = f.key(&format!("n{i}"), 1);
        let start = if terminal == Skipped {
            Pending
        } else {
            Running
        };
        f.repo().insert(&key, start, None).unwrap();
        let before = f.repo().update(&key, &update_to(terminal)).unwrap();

        for next in NodeStatus::ALL {
            let err = f.repo().update(&key, &update_to(next)).unwrap_err();
            assert!(
                matches!(err, StoreError::TerminalRow(_)),
                "{terminal} -> {next}: {err}"
            );
        }
        assert_eq!(f.repo().get(&key).unwrap().unwrap(), before);
    }
}

#[test]
fn database_rejects_terminal_updates_even_without_the_repository() {
    let f = Fixture::new();
    let key = f.key("a", 1);
    f.repo().insert(&key, Running, None).unwrap();
    f.repo()
        .update(&key, &NodeUpdate::status(Completed))
        .unwrap();
    let err =
        f.db.conn()
            .execute("UPDATE node_executions SET output_json = '{}'", [])
            .unwrap_err();
    assert!(err.to_string().contains("immutable"), "{err}");
}

#[test]
fn failed_requires_a_classification() {
    let f = Fixture::new();
    let key = f.key("a", 1);
    f.repo().insert(&key, Running, None).unwrap();

    let err = f
        .repo()
        .update(&key, &NodeUpdate::status(Failed))
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");
    let err = f
        .repo()
        .update(
            &key,
            &NodeUpdate {
                failure: Some(failure()),
                ..NodeUpdate::status(Completed)
            },
        )
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");

    let row = f.repo().update(&key, &update_to(Failed)).unwrap();
    assert_eq!(row.failure, Some(failure()));

    let err =
        f.db.conn()
            .execute(
                "INSERT INTO node_executions (run_id, node_id, attempt, status)
             VALUES (?1, 'b', 1, 'FAILED')",
                [&f.run_id],
            )
            .unwrap_err();
    assert!(err.to_string().contains("failure_category"), "{err}");
}

#[test]
fn status_follows_the_node_state_machine() {
    let allowed = [
        (Pending, Ready),
        (Pending, Skipped),
        (Ready, Running),
        (Ready, Skipped),
        (Running, Polling),
        (Running, Completed),
        (Running, Failed),
        (Running, Skipped),
        (Polling, Completed),
        (Polling, Failed),
        (Polling, Skipped),
    ];
    let f = Fixture::new();
    let non_terminal = [Pending, Ready, Running, Polling];
    for (i, from) in non_terminal.into_iter().enumerate() {
        for (j, to) in NodeStatus::ALL.into_iter().enumerate() {
            let expect_ok = from == to || allowed.contains(&(from, to));
            assert_eq!(from.can_transition_to(to), expect_ok, "{from} -> {to}");

            let key = f.key(&format!("n{i}_{j}"), 1);
            let start = if from == Polling { Running } else { from };
            f.repo().insert(&key, start, None).unwrap();
            if from == Polling {
                f.repo().update(&key, &NodeUpdate::status(Polling)).unwrap();
            }
            match (expect_ok, f.repo().update(&key, &update_to(to))) {
                (true, Ok(row)) => assert_eq!(row.status, to),
                (false, Err(StoreError::InvalidTransition { .. })) => {}
                (_, other) => panic!("{from} -> {to}: unexpected {other:?}"),
            }
        }
    }
}

#[test]
fn attempts_are_unique_and_listed_deterministically() {
    let f = Fixture::new();
    for (node, attempt) in [("b", 1), ("a", 2), ("a", 1)] {
        f.repo().insert(&f.key(node, attempt), Ready, None).unwrap();
    }
    let err = f.repo().insert(&f.key("a", 1), Ready, None).unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "{err}");
    for bad in [
        f.repo().insert(&f.key("c", 0), Ready, None),
        f.repo().insert(&f.key("c", 1), Completed, None),
    ] {
        assert!(matches!(bad, Err(StoreError::Invalid(_))), "{bad:?}");
    }

    let order: Vec<(String, u32)> = f
        .repo()
        .list_for_run(&f.run_id)
        .unwrap()
        .into_iter()
        .map(|e| (e.key.node_id, e.key.attempt))
        .collect();
    assert_eq!(order, [("a".into(), 1), ("a".into(), 2), ("b".into(), 1)]);
    assert_eq!(
        f.repo()
            .latest(&f.run_id, "a")
            .unwrap()
            .unwrap()
            .key
            .attempt,
        2
    );
    assert_eq!(f.repo().latest(&f.run_id, "zzz").unwrap(), None);
}

#[test]
fn updating_a_missing_attempt_is_not_found() {
    let f = Fixture::new();
    let err = f
        .repo()
        .update(&f.key("a", 1), &NodeUpdate::status(Running))
        .unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)), "{err}");
}
