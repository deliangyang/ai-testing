#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn mytool(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mytool"))
        .args(args)
        .env_remove("AITEST_DB")
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn gate_fixtures(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../gate/tests/fixtures")
        .join(dir);
    let mut files: Vec<PathBuf> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    files.sort();
    files
}

fn expected_codes(path: &Path) -> Vec<String> {
    let mut codes: Vec<String> = fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("# expect:"))
        .flat_map(|c| c.split(','))
        .map(|c| match c.trim() {
            "parse" => "document.parse".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

fn validate_json(path: &Path) -> (Option<i32>, Value) {
    let out = mytool(&[
        "dag",
        "validate",
        path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    let report: Value = serde_json::from_str(&stdout(&out))
        .unwrap_or_else(|e| panic!("{}: {e}\n{}", path.display(), stdout(&out)));
    (out.status.code(), report)
}

fn error_codes(report: &Value) -> Vec<String> {
    let mut codes: Vec<String> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["severity"] == "error")
        .map(|i| i["code"].as_str().unwrap().to_owned())
        .collect();
    codes.dedup();
    codes
}

#[test]
fn every_bad_dag_exits_3_with_its_expected_codes() {
    let files = gate_fixtures("bad_dags");
    assert!(files.len() >= 20);
    for path in files {
        let (code, report) = validate_json(&path);
        assert_eq!(code, Some(3), "{}", path.display());
        assert_eq!(report["passed"], false, "{}", path.display());
        assert_eq!(
            error_codes(&report),
            expected_codes(&path),
            "{}",
            path.display()
        );
    }
}

#[test]
fn bad_dags_exit_3_in_text_mode_too() {
    for path in gate_fixtures("bad_dags") {
        let out = mytool(&["dag", "validate", path.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(3), "{}", path.display());
        let text = stdout(&out);
        assert!(text.contains("error["), "{text}");
        assert!(text.contains(": rejected ("), "{text}");
    }
}

#[test]
fn good_dags_exit_0() {
    for path in gate_fixtures("good_dags") {
        let out = mytool(&["dag", "validate", path.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
        assert!(
            stdout(&out).contains(": passed (0 errors"),
            "{}",
            stdout(&out)
        );
    }
}

#[test]
fn warnings_alone_do_not_fail_validation() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("warn.yaml");
    fs::write(
        &file,
        "id: warn-only\nnodes:\n  - { id: a, type: http, method: GET, path: /a }\nedges: []\n",
    )
    .unwrap();
    let (code, report) = validate_json(&file);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["passed"], true);
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["severity"] == "warn"));
    assert!(!report["issues"].as_array().unwrap().is_empty());
}

#[test]
fn json_files_are_parsed_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("dag.json");
    fs::write(
        &file,
        r#"{"id":"j","nodes":[{"id":"a","type":"http","method":"GET","path":"/a"},{"id":"a","type":"http","method":"GET","path":"/b"}],"edges":[]}"#,
    )
    .unwrap();
    let (code, report) = validate_json(&file);
    assert_eq!(code, Some(3));
    assert!(error_codes(&report).contains(&"structure.duplicate_node_id".to_owned()));
}

#[test]
fn json_output_is_canonical_and_stable() {
    let path = &gate_fixtures("bad_dags")[0];
    let first = stdout(&mytool(&[
        "dag",
        "validate",
        path.to_str().unwrap(),
        "--format",
        "json",
    ]));
    let second = stdout(&mytool(&[
        "dag",
        "validate",
        path.to_str().unwrap(),
        "--format",
        "json",
    ]));
    assert_eq!(first, second);
    assert!(first.starts_with(r#"{"file":"#), "{first}");
    assert_eq!(first.lines().count(), 1);
}

#[test]
fn unreadable_file_exits_2() {
    let out = mytool(&["dag", "validate", "does/not/exist.yaml"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot read"));
}
