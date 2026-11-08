#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use serde_json::json;
use store::{Db, UiLayoutRepository};

fn db() -> Db {
    Db::open_in_memory(&FixedClock::new(Timestamp::from_unix_millis(0))).unwrap()
}

#[test]
fn ui_layout_round_trip() {
    let db = db();
    let clock = FixedClock::new(Timestamp::from_unix_millis(1));
    let repo = UiLayoutRepository::new(db.conn(), &clock);
    let layout = json!({
        "nav_collapsed": true,
        "right_width_px": 320,
        "drawer_height_px": 180,
        "drawer_collapsed": false
    });
    let saved = repo.upsert("scenario:serial", &layout).unwrap();
    assert_eq!(saved.scope_key, "scenario:serial");
    assert_eq!(saved.layout, layout);
    let loaded = repo.get("scenario:serial").unwrap().unwrap();
    assert_eq!(loaded.layout, layout);
}

#[test]
fn workspace_presets_are_seeded() {
    let db = db();
    let clock = FixedClock::new(Timestamp::from_unix_millis(0));
    let repo = UiLayoutRepository::new(db.conn(), &clock);
    let presets = repo.list_presets().unwrap();
    assert_eq!(presets.len(), 3);
    assert!(presets.iter().any(|p| p.id == "debugger"));
}
