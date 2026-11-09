#![allow(clippy::unwrap_used)]

use common::clock::FixedClock;
use common::clock::Timestamp;
use store::{ApiSpecRepository, Db, NewApiSpec, StoreError};

fn db() -> Db {
    Db::open_in_memory(&FixedClock::new(Timestamp::from_unix_millis(
        1_700_000_000_000,
    )))
    .unwrap()
}

fn spec(auth: Option<&str>) -> NewApiSpec {
    NewApiSpec {
        id: "orders.get".into(),
        name: "Get order".into(),
        method: "GET".into(),
        path: "/orders/{id}".into(),
        auth_scheme_ref: auth.map(str::to_owned),
    }
}

#[test]
fn rejects_plaintext_auth_scheme_ref() {
    let database = db();
    let repo = ApiSpecRepository::new(database.conn());
    let err = repo
        .upsert(&spec(Some("Bearer sk-secret-token")))
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)));
    assert!(err.to_string().contains("plaintext"));
}

#[test]
fn accepts_keychain_and_env_auth_scheme_refs() {
    let database = db();
    let repo = ApiSpecRepository::new(database.conn());
    repo.upsert(&spec(Some("keychain://ai-testing/ci-runner")))
        .unwrap();
    repo.upsert(&spec(Some("${env:AITEST_API_KEY}"))).unwrap();
    repo.upsert(&spec(None)).unwrap();
}
