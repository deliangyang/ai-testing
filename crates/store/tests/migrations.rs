#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use store::migrate::{applied, migrate};
use store::{Db, Migration, StoreError};

const V1: Migration = Migration {
    file: "V001__widgets.sql",
    sql: "CREATE TABLE widgets (id TEXT PRIMARY KEY, owner TEXT REFERENCES owners(id));",
};
const V2: Migration = Migration {
    file: "V002__owners.sql",
    sql: "CREATE TABLE owners (id TEXT PRIMARY KEY);",
};
const V3: Migration = Migration {
    file: "V003__widget_color.sql",
    sql: "ALTER TABLE widgets ADD COLUMN color TEXT;",
};

fn clock(millis: i64) -> FixedClock {
    FixedClock::new(Timestamp::from_unix_millis(millis))
}

fn tables(db: &Db) -> Vec<String> {
    let mut stmt = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn pragma<T: rusqlite::types::FromSql>(db: &Db, name: &str) -> T {
    db.conn()
        .query_row(&format!("PRAGMA {name}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn new_database_is_configured_and_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_with(dir.path().join("app.db"), &[V1, V2], &clock(0)).unwrap();

    assert_eq!(pragma::<String>(&db, "journal_mode"), "wal");
    assert_eq!(pragma::<i64>(&db, "foreign_keys"), 1);
    assert_eq!(pragma::<i64>(&db, "busy_timeout"), 5000);
    assert_eq!(db.schema_version().unwrap(), 2);
    assert_eq!(tables(&db), ["owners", "schema_migrations", "widgets"]);

    let rows = applied(db.conn()).unwrap();
    assert_eq!(rows[0].name, "widgets");
    assert_eq!(rows[0].checksum, V1.checksum());
    assert_eq!(rows[1].applied_at, "1970-01-01T00:00:00.000Z");
}

#[test]
fn foreign_keys_are_enforced() {
    let db = Db::open_with(":memory:", &[V1, V2], &clock(0)).unwrap();
    let err = db
        .conn()
        .execute("INSERT INTO widgets (id, owner) VALUES ('w', 'nobody')", [])
        .unwrap_err();
    assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
}

#[test]
fn reopening_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let first = applied(
        Db::open_with(&path, &[V1, V2], &clock(1_000))
            .unwrap()
            .conn(),
    )
    .unwrap();

    let mut db = Db::open_with(&path, &[V1, V2], &clock(9_000)).unwrap();
    assert_eq!(applied(db.conn()).unwrap(), first);
    assert_eq!(
        migrate(db.conn_mut(), &[V1, V2], &clock(9_000)).unwrap(),
        Vec::<u32>::new()
    );
}

#[test]
fn reopening_with_new_migrations_applies_only_those() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    drop(Db::open_with(&path, &[V1, V2], &clock(1_000)).unwrap());

    let mut db = Db::open_with(&path, &[V1, V2], &clock(2_000)).unwrap();
    assert_eq!(
        migrate(db.conn_mut(), &[V1, V2, V3], &clock(2_000)).unwrap(),
        [3]
    );
    let rows = applied(db.conn()).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].applied_at, "1970-01-01T00:00:01.000Z");
    assert_eq!(rows[2].applied_at, "1970-01-01T00:00:02.000Z");
}

#[test]
fn edited_migration_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    drop(Db::open_with(&path, &[V1], &clock(0)).unwrap());

    let edited = Migration {
        sql: "CREATE TABLE widgets (id TEXT PRIMARY KEY, owner TEXT, extra TEXT);",
        ..V1
    };
    let err = Db::open_with(&path, &[edited], &clock(0)).err().unwrap();
    assert!(
        matches!(err, StoreError::ChecksumMismatch { version: 1, .. }),
        "{err}"
    );
}

#[test]
fn database_from_a_newer_build_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    drop(Db::open_with(&path, &[V1, V2], &clock(0)).unwrap());

    let err = Db::open_with(&path, &[V1], &clock(0)).err().unwrap();
    assert!(
        matches!(err, StoreError::UnknownMigration { version: 2 }),
        "{err}"
    );
}

#[test]
fn migration_inserted_below_applied_version_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    drop(Db::open_with(&path, &[V1, V3], &clock(0)).unwrap());

    let err = Db::open_with(&path, &[V1, V2, V3], &clock(0))
        .err()
        .unwrap();
    assert!(
        matches!(
            err,
            StoreError::OutOfOrder {
                version: 2,
                applied: 3
            }
        ),
        "{err}"
    );
}

#[test]
fn failing_migration_rolls_back_the_whole_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let broken = Migration {
        file: "V003__broken.sql",
        sql: "CREATE TABLE ok_table (id TEXT); THIS IS NOT SQL;",
    };
    assert!(matches!(
        Db::open_with(&path, &[V1, V2, broken], &clock(0)),
        Err(StoreError::Sqlite(_))
    ));

    let db = Db::open_with(&path, &[], &clock(0)).unwrap();
    assert_eq!(db.schema_version().unwrap(), 0);
    assert_eq!(tables(&db), ["schema_migrations"]);
}

#[test]
fn non_increasing_versions_are_rejected_before_touching_the_database() {
    let err = Db::open_with(":memory:", &[V2, V1], &clock(0))
        .err()
        .unwrap();
    assert!(matches!(err, StoreError::InvalidMigration { .. }), "{err}");
}

#[test]
fn concurrent_openers_apply_each_migration_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || Db::open_with(&path, &[V1, V2, V3], &clock(i)).map(|_| ()))
        })
        .collect();
    for h in handles {
        h.join().unwrap().unwrap();
    }
    let db = Db::open_with(&path, &[V1, V2, V3], &clock(0)).unwrap();
    assert_eq!(applied(db.conn()).unwrap().len(), 3);
}

#[test]
fn built_in_migrations_apply_to_a_fresh_database() {
    let db = Db::open_in_memory(&clock(0)).unwrap();
    assert_eq!(db.schema_version().unwrap(), store::MIGRATIONS.len() as u32);
}
