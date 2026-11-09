#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use store::{Db, SkillRepository, SkillStatus};

const VALID_SKILL: &str = r#"
manifest:
  id: ping-skill
  name: Ping
  version: "1.0.0"
inputs:
  fields: []
outputs:
  fields: []
dag:
  id: ping-inner
  nodes:
    - id: call
      type: http
      method: GET
      path: /ping
"#;

const BAD_DAG_SKILL: &str = r#"
manifest:
  id: cyclic
  name: Cyclic
  version: "1.0.0"
inputs:
  fields: []
outputs:
  fields: []
dag:
  id: cyclic-inner
  nodes:
    - { id: a, type: http, method: GET, path: /a }
    - { id: b, type: http, method: GET, path: /b }
  edges:
    - { from: a, to: b }
    - { from: b, to: a }
"#;

struct Workspace {
    dir: tempfile::TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn db(&self) -> PathBuf {
        self.dir.path().join("state").join("aitest.db")
    }

    fn file(&self, name: &str, yaml: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, yaml).unwrap();
        path
    }

    fn mytool(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_mytool"))
            .args(args)
            .env("AITEST_DB", self.db())
            .env_remove("RUST_LOG")
            .output()
            .unwrap()
    }

    fn save(&self, file: &Path) -> Output {
        self.mytool(&["skill", "save", file.to_str().unwrap()])
    }

    fn list_json(&self) -> Vec<(String, u64, String, String)> {
        let out = self.mytool(&["skill", "list", "--format", "json"]);
        assert!(out.status.success(), "{}", stderr(&out));
        let rows: Value = serde_json::from_str(&stdout(&out)).unwrap();
        rows.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_owned(),
                    r["version"].as_u64().unwrap(),
                    r["manifest_version"].as_str().unwrap().to_owned(),
                    r["status"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn gate_rejects_cyclic_internal_dag() {
    let ws = Workspace::new();
    let file = ws.file("bad.yaml", BAD_DAG_SKILL);
    let out = ws.save(&file);
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));
}

#[test]
fn saved_revisions_are_pending_until_activated() {
    let ws = Workspace::new();
    let file = ws.file("skill.yaml", VALID_SKILL);
    assert!(ws.save(&file).status.success());

    let mut semver_skill = VALID_SKILL.replace("1.0.0", "1.0.1");
    semver_skill = semver_skill.replace("Ping", "Ping B");
    let file2 = ws.file("skill2.yaml", &semver_skill);
    assert!(ws.save(&file2).status.success());

    assert_eq!(
        ws.list_json(),
        [
            ("ping-skill".into(), 1, "1.0.0".into(), "pending".into()),
            ("ping-skill".into(), 2, "1.0.1".into(), "pending".into()),
        ]
    );

    let out = ws.mytool(&["skill", "activate", "ping-skill", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("activated Skill 'ping-skill' revision 1"));

    let out = ws.mytool(&["skill", "activate", "ping-skill", "2"]);
    assert!(out.status.success());
    assert_eq!(
        ws.list_json(),
        [
            ("ping-skill".into(), 1, "1.0.0".into(), "archived".into()),
            ("ping-skill".into(), 2, "1.0.1".into(), "active".into()),
        ]
    );
}

#[test]
fn repository_round_trip_matches_cli_save() {
    let ws = Workspace::new();
    let file = ws.file("skill.yaml", VALID_SKILL);
    assert!(ws.save(&file).status.success());

    let db = Db::open(ws.db(), &common::clock::SystemClock).unwrap();
    let record = SkillRepository::new(db.conn())
        .get("ping-skill", 1)
        .unwrap()
        .unwrap();
    assert_eq!(record.info.status, SkillStatus::Pending);
    assert_eq!(record.package.manifest.version, "1.0.0");
    assert_eq!(record.package.dag.id, "ping-inner");
}
