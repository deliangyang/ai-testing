#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::clock::{FixedClock, Timestamp};
use model::Dag;
use serde_json::Value;
use store::{DagRepository, Db, CREATED_BY_AI};

struct Workspace {
    dir: tempfile::TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// Lives in a subdirectory that does not exist yet, so the CLI has to create it.
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
        self.mytool(&["dag", "save", file.to_str().unwrap()])
    }

    fn list(&self) -> Vec<(String, u64, String)> {
        let out = self.mytool(&["dag", "list", "--format", "json"]);
        assert!(out.status.success(), "{}", stderr(&out));
        let rows: Value = serde_json::from_str(&stdout(&out)).unwrap();
        rows.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_owned(),
                    r["version"].as_u64().unwrap(),
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

fn row(id: &str, version: u64, status: &str) -> (String, u64, String) {
    (id.to_owned(), version, status.to_owned())
}

const PLAIN: &str = "id: orders\nnodes:\n  - { id: a, type: http, method: GET, path: /a }\n";
const CYCLE: &str = "id: orders\nnodes:\n  - { id: a, type: http, method: GET, path: /a }\n  - { id: b, type: http, method: GET, path: /b }\nedges:\n  - { from: a, to: b }\n  - { from: b, to: a }\n";
const PARENT: &str = "id: parent\nnodes:\n  - { id: s, type: subworkflow, dag: { id: orders } }\n";

#[test]
fn a_file_that_fails_the_gate_is_not_saved() {
    let ws = Workspace::new();
    let out = ws.save(&ws.file("cycle.yaml", CYCLE));
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("error[graph.cycle]"),
        "{}",
        stdout(&out)
    );

    let out = ws.save(&ws.file("broken.yaml", "id: x\nnodes: 3\n"));
    assert_eq!(out.status.code(), Some(3));
    assert!(stdout(&out).contains("error[document.parse]"));

    assert!(ws.list().is_empty());
}

#[test]
fn every_bad_dag_in_the_corpus_is_refused() {
    let ws = Workspace::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gate/tests/fixtures/bad_dags");
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let out = ws.save(&path);
        assert_eq!(out.status.code(), Some(3), "{}", path.display());
    }
    assert!(ws.list().is_empty());
}

#[test]
fn saved_versions_are_pending_until_activated() {
    let ws = Workspace::new();
    let file = ws.file("orders.yaml", PLAIN);
    let out = ws.save(&file);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains("saved DAG 'orders' version 1 (pending)"));
    assert!(
        stdout(&out).contains("warn[completeness.unasserted_http]"),
        "warnings are shown but do not block: {}",
        stdout(&out)
    );
    ws.save(&file);
    assert_eq!(
        ws.list(),
        [row("orders", 1, "pending"), row("orders", 2, "pending")]
    );

    let out = ws.mytool(&["dag", "activate", "orders", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains("activated DAG 'orders' version 1"));

    let out = ws.mytool(&["dag", "activate", "orders", "2"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("(version 1 archived)"),
        "{}",
        stdout(&out)
    );
    assert_eq!(
        ws.list(),
        [row("orders", 1, "archived"), row("orders", 2, "active")]
    );

    let out = ws.mytool(&["dag", "activate", "orders", "2"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("already active"));
}

#[test]
fn activating_a_missing_version_is_an_error() {
    let ws = Workspace::new();
    let out = ws.mytool(&["dag", "activate", "orders", "7"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("DAG 'orders' has no version 7"));
}

#[test]
fn subworkflow_references_must_resolve_to_an_activated_dag() {
    let ws = Workspace::new();
    let parent = ws.file("parent.yaml", PARENT);

    let out = ws.save(&parent);
    assert_eq!(out.status.code(), Some(3));
    assert!(stdout(&out).contains("error[subworkflow.unknown_dag]"));

    ws.save(&ws.file("orders.yaml", PLAIN));
    let out = ws.save(&parent);
    assert_eq!(out.status.code(), Some(3), "a pending child cannot be run");

    ws.mytool(&["dag", "activate", "orders", "1"]);
    let out = ws.save(&parent);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
}

#[test]
fn a_subworkflow_cycle_through_the_store_is_refused() {
    let ws = Workspace::new();
    ws.save(&ws.file("orders.yaml", PLAIN));
    ws.mytool(&["dag", "activate", "orders", "1"]);
    ws.save(&ws.file("parent.yaml", PARENT));
    ws.mytool(&["dag", "activate", "parent", "1"]);

    let back = "id: orders\nnodes:\n  - { id: p, type: subworkflow, dag: { id: parent } }\n";
    let out = ws.save(&ws.file("orders2.yaml", back));
    assert_eq!(out.status.code(), Some(3));
    assert!(
        stdout(&out).contains("error[subworkflow.cycle]"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn activation_runs_the_gate_on_the_stored_content() {
    let ws = Workspace::new();
    fs::create_dir_all(ws.db().parent().unwrap()).unwrap();
    {
        let clock = FixedClock::new(Timestamp::from_unix_millis(0));
        let db = Db::open(ws.db(), &clock).unwrap();
        DagRepository::new(db.conn(), &clock)
            .save(&Dag::from_yaml_str(CYCLE).unwrap(), "elsewhere")
            .unwrap();
    }
    let out = ws.mytool(&["dag", "activate", "orders", "1"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(stdout(&out).contains("error[graph.cycle]"));
    assert_eq!(ws.list(), [row("orders", 1, "pending")]);
}

#[test]
fn list_prints_a_table() {
    let ws = Workspace::new();
    let out = ws.mytool(&["dag", "list"]);
    assert_eq!(stdout(&out).trim(), "no DAGs stored");

    ws.save(&ws.file("orders.yaml", PLAIN));
    let text = stdout(&ws.mytool(&["dag", "list"]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].starts_with("ID"), "{text}");
    assert!(
        lines[1].starts_with("orders  1        pending   "),
        "{text}"
    );
    assert!(lines[1].ends_with("  cli"), "{text}");
}

#[test]
fn pending_lists_only_ai_review_queue() {
    let ws = Workspace::new();
    ws.save(&ws.file("orders.yaml", PLAIN));
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open(ws.db(), &clock).unwrap();
    DagRepository::new(db.conn(), &clock)
        .save_ai_candidate(&Dag::from_yaml_str(PLAIN).unwrap())
        .unwrap();

    let out = ws.mytool(&["dag", "pending", "--format", "json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["created_by"].as_str().unwrap(), CREATED_BY_AI);
    assert_eq!(rows[0]["status"].as_str().unwrap(), "pending");
}

const NEEDS_CALIBRATE: &str = r#"id: cal
nodes:
  - id: root
    type: http
    method: GET
    path: /
  - id: c
    type: http
    method: GET
    path: /c
  - id: d
    type: http
    method: POST
    path: /d
    body:
      x: ${c.output.body.id}
edges:
  - from: root
    to: c
  - from: root
    to: d
"#;

#[test]
fn activate_runs_calibration_then_gate() {
    let ws = Workspace::new();
    if let Some(parent) = ws.db().parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open(ws.db(), &clock).unwrap();
    DagRepository::new(db.conn(), &clock)
        .save(&Dag::from_yaml_str(NEEDS_CALIBRATE).unwrap(), "test")
        .unwrap();

    let out = ws.mytool(&["dag", "activate", "cal", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("calibrated"), "{text}");
    assert!(text.contains("activated DAG 'cal' version 1"));
}
