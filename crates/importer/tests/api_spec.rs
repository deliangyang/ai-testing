#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use importer::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};
use serde_json::json;
use store::{ApiSpecRepository, Db};

#[test]
fn normalize_method_path_and_param_order() {
    let spec = ApiSpec {
        id: "orders.get".into(),
        name: "  Get order  ".into(),
        method: " get ".into(),
        path: "orders/{id}".into(),
        version: None,
        params: vec![
            ApiParam {
                name: "q".into(),
                location: ParamLocation::Query,
                required: false,
                schema: None,
            },
            ApiParam {
                name: "id".into(),
                location: ParamLocation::Path,
                required: true,
                schema: None,
            },
        ],
        response_schema: Some(json!({
            "type": "object",
            "properties": { "id": { "type": "string" } }
        })),
        auth_scheme_ref: None,
    };
    let normalized = normalize_api_spec(spec);
    assert_eq!(normalized.method, "GET");
    assert_eq!(normalized.path, "/orders/{id}");
    assert_eq!(normalized.name, "Get order");
    assert_eq!(normalized.params[0].name, "id");
    assert_eq!(normalized.params[1].name, "q");
}

#[test]
fn canonical_json_is_stable() {
    let spec = ApiSpec {
        id: "ping".into(),
        name: "Ping".into(),
        method: "GET".into(),
        path: "/ping".into(),
        version: None,
        params: vec![],
        response_schema: None,
        auth_scheme_ref: Some("${env:KEY}".into()),
    };
    let first = spec.to_canonical_json().unwrap();
    let second = normalize_api_spec(spec).to_canonical_json().unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first,
        r#"{"auth_scheme_ref":"${env:KEY}","id":"ping","method":"GET","name":"Ping","path":"/ping"}"#
    );
}

#[test]
fn round_trip_through_store_matches_canonical_json() {
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = ApiSpecRepository::new(db.conn(), &clock);

    let spec = normalize_api_spec(ApiSpec {
        id: "health".into(),
        name: "Health".into(),
        method: "get".into(),
        path: "health".into(),
        version: None,
        params: vec![ApiParam {
            name: "verbose".into(),
            location: ParamLocation::Query,
            required: false,
            schema: Some(json!({ "type": "boolean" })),
        }],
        response_schema: Some(json!({ "type": "object" })),
        auth_scheme_ref: None,
    });
    let canonical = spec.to_canonical_json().unwrap();
    let saved = repo.save(&spec.into_save_input(), "import").unwrap();
    let loaded = repo.get("health", saved.info.version).unwrap().unwrap();

    let round = ApiSpec {
        id: loaded.spec.id,
        name: loaded.spec.name,
        method: loaded.spec.method,
        path: loaded.spec.path,
        version: None,
        params: serde_json::from_value(loaded.spec.params.unwrap()).unwrap(),
        response_schema: loaded.spec.response_schema,
        auth_scheme_ref: loaded.spec.auth_scheme_ref,
    };
    let round_canonical = normalize_api_spec(round).to_canonical_json().unwrap();
    assert_eq!(canonical, round_canonical);
}
