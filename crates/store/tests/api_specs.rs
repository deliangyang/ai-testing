#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use serde_json::json;
use store::{ApiSpecRepository, ApiSpecSaveInput, ApiSpecStatus, Db, StoreError};

fn clock() -> FixedClock {
    FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000))
}

fn db() -> Db {
    Db::open_in_memory(&clock()).unwrap()
}

fn spec(auth: Option<&str>) -> ApiSpecSaveInput {
    ApiSpecSaveInput {
        id: "orders.get".into(),
        name: "Get order".into(),
        method: "GET".into(),
        path: "/orders/{id}".into(),
        params: None,
        response_schema: None,
        auth_scheme_ref: auth.map(str::to_owned),
    }
}

fn rich_spec() -> ApiSpecSaveInput {
    ApiSpecSaveInput {
        id: "orders.get".into(),
        name: "Get order".into(),
        method: "get".into(),
        path: "/orders/{id}".into(),
        params: Some(json!([
            { "name": "id", "location": "path", "required": true, "schema": { "type": "string" } }
        ])),
        response_schema: Some(json!({
            "type": "object",
            "properties": { "id": { "type": "string" } }
        })),
        auth_scheme_ref: Some("${env:AITEST_API_KEY}".into()),
    }
}

fn statuses(repo: &ApiSpecRepository<'_>, id: &str) -> Vec<(u32, ApiSpecStatus)> {
    repo.list_versions(id)
        .unwrap()
        .into_iter()
        .map(|v| (v.version, v.status))
        .collect()
}

#[test]
fn rejects_plaintext_auth_scheme_ref() {
    let clock = clock();
    let database = db();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    let err = repo
        .save(&spec(Some("Bearer sk-secret-token")), "alice")
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)));
    assert!(err.to_string().contains("plaintext"));
}

#[test]
fn accepts_keychain_and_env_auth_scheme_refs() {
    let clock = clock();
    let database = db();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    repo.save(&spec(Some("keychain://ai-testing/ci-runner")), "alice")
        .unwrap();
    repo.save(&spec(Some("${env:AITEST_API_KEY}")), "alice")
        .unwrap();
    repo.save(&spec(None), "alice").unwrap();
}

#[test]
fn save_assigns_increasing_versions_per_api() {
    let clock = clock();
    let database = Db::open_in_memory(&clock).unwrap();
    let repo = ApiSpecRepository::new(database.conn(), &clock);

    let first = repo.save(&rich_spec(), "alice").unwrap();
    clock.advance(std::time::Duration::from_secs(1));
    let mut second_input = rich_spec();
    second_input.name = "Get order v2".into();
    let second = repo.save(&second_input, "bob").unwrap();
    let other = repo
        .save(
            &ApiSpecSaveInput {
                id: "users.list".into(),
                name: "List users".into(),
                method: "GET".into(),
                path: "/users".into(),
                params: None,
                response_schema: None,
                auth_scheme_ref: None,
            },
            "alice",
        )
        .unwrap();

    assert_eq!(
        (first.info.version, second.info.version, other.info.version),
        (1, 2, 1)
    );
    assert_eq!(second.info.status, ApiSpecStatus::Draft);
    assert_eq!(second.info.created_by, "bob");

    let loaded = repo.get("orders.get", 1).unwrap().unwrap();
    assert_eq!(loaded.info, first.info);
}

#[test]
fn json_columns_are_stored_as_canonical_json() {
    let clock = clock();
    let database = Db::open_in_memory(&clock).unwrap();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    repo.save(&rich_spec(), "alice").unwrap();

    let params: String = database
        .conn()
        .query_row(
            "SELECT params_json FROM api_specs WHERE id = 'orders.get'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        params,
        r#"[{"location":"path","name":"id","required":true,"schema":{"type":"string"}}]"#
    );

    let schema: String = database
        .conn()
        .query_row(
            "SELECT response_schema_json FROM api_specs WHERE id = 'orders.get'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        schema,
        r#"{"properties":{"id":{"type":"string"}},"type":"object"}"#
    );
}

#[test]
fn activation_deprecates_previous_active() {
    let clock = clock();
    let database = Db::open_in_memory(&clock).unwrap();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    for name in ["Get order v1", "Get order v2", "Get order v3"] {
        let mut input = rich_spec();
        input.name = name.into();
        repo.save(&input, "alice").unwrap();
    }
    assert_eq!(repo.get_active("orders.get").unwrap(), None);

    repo.activate("orders.get", 1, None).unwrap();
    repo.activate("orders.get", 2, Some(1)).unwrap();
    assert_eq!(
        statuses(&repo, "orders.get"),
        [
            (1, ApiSpecStatus::Deprecated),
            (2, ApiSpecStatus::Active),
            (3, ApiSpecStatus::Draft)
        ]
    );
    let active = repo.get_active("orders.get").unwrap().unwrap();
    assert_eq!(active.spec.name, "Get order v2");
    assert_eq!(active.info.version, 2);
}

#[test]
fn only_one_active_version_per_api_spec() {
    let clock = clock();
    let database = db();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    repo.save(&rich_spec(), "alice").unwrap();
    let mut v2 = rich_spec();
    v2.name = "v2".into();
    repo.save(&v2, "alice").unwrap();
    repo.activate("orders.get", 1, None).unwrap();

    let err = database
        .conn()
        .execute(
            "UPDATE api_specs SET status = 'active' WHERE id = 'orders.get' AND version = 2",
            [],
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("UNIQUE"), "{err}");
}

#[test]
fn stale_activation_is_conflict() {
    let clock = clock();
    let database = db();
    let repo = ApiSpecRepository::new(database.conn(), &clock);
    repo.save(&rich_spec(), "alice").unwrap();
    repo.save(&rich_spec(), "alice").unwrap();
    repo.activate("orders.get", 1, None).unwrap();
    let err = repo.activate("orders.get", 2, None).unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)));
}
