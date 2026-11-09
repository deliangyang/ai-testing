#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use serde_json::json;
use store::{
    ConnectionRepository, ConnectionStatus, ConnectionType, Db, DbConnectionSaveInput, StoreError,
};

fn db() -> Db {
    Db::open_in_memory(&FixedClock::new(Timestamp::from_unix_millis(0))).unwrap()
}

fn sample_connection(auth_ref: Option<&str>) -> DbConnectionSaveInput {
    DbConnectionSaveInput {
        name: "local-crm".into(),
        connection_type: ConnectionType::Postgres,
        host: Some("localhost".into()),
        port: Some(5432),
        database_name: Some("crm".into()),
        schema_name: Some("public".into()),
        auth_ref: auth_ref.map(str::to_owned),
        read_only: true,
        pool_config: Some(json!({ "max_connections": 4 })),
        status: ConnectionStatus::Active,
    }
}

#[test]
fn rejects_plaintext_auth_ref() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    let err = repo
        .save(&sample_connection(Some("postgres://user:secret@host/db")))
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)));
    assert!(err.to_string().contains("plaintext"));
}

#[test]
fn accepts_keychain_and_env_auth_refs() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    repo.save(&sample_connection(Some("keychain://ai-testing/crm-ro")))
        .unwrap();
    repo.save(&DbConnectionSaveInput {
        auth_ref: Some("${env:CRM_DB_PASSWORD}".into()),
        ..sample_connection(None)
    })
    .unwrap();
    repo.save(&sample_connection(None)).unwrap();
}

#[test]
fn crud_and_list_connections() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    let saved = repo
        .save(&sample_connection(Some("${env:CRM_DB_PASSWORD}")))
        .unwrap();
    assert_eq!(saved.connection.name, "local-crm");

    let loaded = repo.get("local-crm").unwrap().unwrap();
    assert_eq!(loaded, saved);

    let sqlite = DbConnectionSaveInput {
        name: "app-sqlite".into(),
        connection_type: ConnectionType::Sqlite,
        host: None,
        port: None,
        database_name: Some("/tmp/app.db".into()),
        schema_name: None,
        auth_ref: None,
        read_only: false,
        pool_config: None,
        status: ConnectionStatus::Draft,
    };
    repo.save(&sqlite).unwrap();

    let names: Vec<String> = repo
        .list()
        .unwrap()
        .into_iter()
        .map(|r| r.connection.name)
        .collect();
    assert_eq!(names, vec!["app-sqlite".to_owned(), "local-crm".to_owned()]);
}

#[test]
fn pool_config_is_stored_as_canonical_json() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    repo.save(&sample_connection(Some("${env:CRM_DB_PASSWORD}")))
        .unwrap();
    let raw: String = database
        .conn()
        .query_row(
            "SELECT pool_config_json FROM db_connections WHERE name = 'local-crm'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, r#"{"max_connections":4}"#);
}

#[test]
fn environment_binding_resolves_connection() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    repo.save(&sample_connection(Some("keychain://ai-testing/crm-ro")))
        .unwrap();
    repo.bind_environment("local", "crm-ro", "local-crm")
        .unwrap();
    let resolved = repo
        .resolve_for_environment("local", "crm-ro")
        .unwrap()
        .unwrap();
    assert_eq!(resolved.connection.name, "local-crm");

    let bindings = repo.list_bindings_for_environment("local").unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].logical_name, "crm-ro");

    repo.bind_environment("staging", "crm-ro", "local-crm")
        .unwrap();
    assert!(repo
        .resolve_for_environment("dev", "crm-ro")
        .unwrap()
        .is_none());
}

#[test]
fn bind_requires_existing_connection() {
    let database = db();
    let repo = ConnectionRepository::new(database.conn());
    let err = repo
        .bind_environment("local", "crm-ro", "missing")
        .unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)));
}
