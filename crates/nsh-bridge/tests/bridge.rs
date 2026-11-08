#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use common::clock::{FixedClock, Timestamp};
use model::Dag;
use nsh_bridge::{list_dags, DagListEntry};
use store::DagRepository;
use store::Db;

fn sample_dag() -> Dag {
    serde_json::from_value(serde_json::json!({
        "id": "spike",
        "nodes": [{"id": "a", "type": "http", "method": "GET", "path": "/"}],
        "edges": []
    }))
    .unwrap()
}

#[test]
fn list_dags_matches_store_repository() {
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(0)));
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("t.db");
    {
        let db = Db::open(&path, clock.as_ref()).unwrap();
        DagRepository::new(db.conn(), clock.as_ref())
            .save(&sample_dag(), "spike")
            .unwrap();
    }

    let json: Vec<DagListEntry> = list_dags(&path, clock.as_ref()).unwrap();
    assert_eq!(json.len(), 1);
    assert_eq!(json[0].id, "spike");
    assert_eq!(json[0].version, 1);
}
