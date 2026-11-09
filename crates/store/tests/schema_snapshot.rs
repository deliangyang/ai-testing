//! The migrated schema is checked in as `tests/snapshots/schema.sql` so every schema change
//! shows up in review. Regenerate with `UPDATE_SCHEMAS=1 cargo test -p store --test schema_snapshot`.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use common::clock::{FixedClock, Timestamp};
use store::Db;

fn db() -> Db {
    Db::open_in_memory(&FixedClock::new(Timestamp::from_unix_millis(0))).unwrap()
}

fn dump(db: &Db) -> String {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT sql FROM sqlite_master
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
             ORDER BY tbl_name, type DESC, name",
        )
        .unwrap();
    let statements: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    statements
        .iter()
        .map(|s| format!("{};\n", s.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn schema_matches_snapshot() {
    let actual = dump(&db());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/schema.sql");
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        actual == expected,
        "schema changed; rerun with UPDATE_SCHEMAS=1 and review {}",
        path.display()
    );
}

fn exec(db: &Db, sql: &str) -> Result<usize, String> {
    db.conn().execute(sql, []).map_err(|e| e.to_string())
}

fn seed_dag(db: &Db, version: i64, status: &str) {
    exec(
        db,
        &format!(
            "INSERT INTO dag_definitions (id, version, content_json, status, created_by, created_at)
             VALUES ('d', {version}, '{{}}', '{status}', 'test', '2026-01-01T00:00:00.000Z')"
        ),
    )
    .unwrap();
}

#[test]
fn only_one_active_version_per_api_spec() {
    let db = db();
    exec(
        &db,
        "INSERT INTO api_specs (
            id, version, name, method, path, status, created_by, created_at
         ) VALUES ('api', 1, 'n', 'GET', '/x', 'active', 't', '2026-01-01T00:00:00.000Z')",
    )
    .unwrap();
    exec(
        &db,
        "INSERT INTO api_specs (
            id, version, name, method, path, status, created_by, created_at
         ) VALUES ('api', 2, 'n', 'GET', '/x', 'draft', 't', '2026-01-01T00:00:00.000Z')",
    )
    .unwrap();
    let err = exec(
        &db,
        "UPDATE api_specs SET status = 'active' WHERE version = 2",
    )
    .unwrap_err();
    assert!(err.contains("UNIQUE"), "{err}");
}

#[test]
fn only_one_active_version_per_dag() {
    let db = db();
    seed_dag(&db, 1, "active");
    seed_dag(&db, 2, "pending");
    let err = exec(
        &db,
        "UPDATE dag_definitions SET status = 'active' WHERE version = 2",
    )
    .unwrap_err();
    assert!(err.contains("UNIQUE"), "{err}");
    exec(
        &db,
        "UPDATE dag_definitions SET status = 'archived' WHERE version = 1",
    )
    .unwrap();
    exec(
        &db,
        "UPDATE dag_definitions SET status = 'active' WHERE version = 2",
    )
    .unwrap();
}

#[test]
fn enum_columns_and_json_are_checked() {
    let db = db();
    seed_dag(&db, 1, "active");
    for bad in [
        "UPDATE dag_definitions SET status = 'live'",
        "UPDATE dag_definitions SET content_json = '{not json'",
        "INSERT INTO dag_runs (id, dag_id, dag_version, env_name, mode)
         VALUES ('r', 'd', 1, 'dev', 'nightly')",
        "INSERT INTO dag_runs (id, dag_id, dag_version, env_name, mode, status)
         VALUES ('r', 'd', 1, 'dev', 'ci', 'running')",
        "INSERT INTO dag_definitions (id, version, content_json, created_by, created_at)
         VALUES ('e', 'one', '{}', 't', 'x')",
    ] {
        let err = exec(&db, bad).unwrap_err();
        assert!(
            err.contains("CHECK") || err.contains("cannot store"),
            "{bad}: {err}"
        );
    }
}

#[test]
fn runs_reference_existing_dag_versions_and_cascade_to_executions() {
    let db = db();
    let err = exec(
        &db,
        "INSERT INTO dag_runs (id, dag_id, dag_version, env_name, mode)
         VALUES ('r', 'd', 1, 'dev', 'ci')",
    )
    .unwrap_err();
    assert!(err.contains("FOREIGN KEY"), "{err}");

    seed_dag(&db, 1, "active");
    exec(
        &db,
        "INSERT INTO dag_runs (id, dag_id, dag_version, env_name, mode)
         VALUES ('r', 'd', 1, 'dev', 'ci')",
    )
    .unwrap();
    exec(
        &db,
        "INSERT INTO node_executions (run_id, node_id, attempt, status)
         VALUES ('r', 'n', 1, 'RUNNING')",
    )
    .unwrap();
    exec(
        &db,
        "INSERT INTO failure_records (id, run_id, failure_category, raw_message, classified_at)
         VALUES ('f', 'r', 'network', 'timeout', '2026-01-01T00:00:00.000Z')",
    )
    .unwrap();
    let err = exec(
        &db,
        "INSERT INTO node_executions (run_id, node_id, attempt, status)
         VALUES ('ghost', 'n', 1, 'RUNNING')",
    )
    .unwrap_err();
    assert!(err.contains("FOREIGN KEY"), "{err}");

    exec(&db, "DELETE FROM dag_runs WHERE id = 'r'").unwrap();
    let left: i64 = db
        .conn()
        .query_row(
            "SELECT (SELECT count(*) FROM node_executions) + (SELECT count(*) FROM failure_records)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
