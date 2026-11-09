#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::clock::{FixedClock, Timestamp};
use store::{ApiSpecRepository, ApiSpecStatus, Db};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn importer_fixture(rel: &str) -> PathBuf {
    repo_root().join("crates/importer/tests/fixtures").join(rel)
}

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
        self.dir.path().join("aitest.db")
    }

    fn mytool(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_mytool"))
            .args(args)
            .env("AITEST_DB", self.db())
            .env_remove("RUST_LOG")
            .output()
            .unwrap()
    }

    fn import(&self, format: &str, file: &Path, extra: &[&str]) -> Output {
        let file_s = file.to_str().unwrap();
        let mut args = vec![
            "api",
            "import",
            file_s,
            "--format",
            format,
            "--strategy",
            "merge",
        ];
        args.extend_from_slice(extra);
        self.mytool(&args)
    }

    fn draft_count(&self) -> usize {
        let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
        let db = Db::open(self.db(), &clock).unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM api_specs WHERE status = 'draft'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        count as usize
    }

    fn has_draft(&self, id: &str) -> bool {
        let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
        let db = Db::open(self.db(), &clock).unwrap();
        let repo = ApiSpecRepository::new(db.conn(), &clock);
        !repo.list_versions(id).unwrap().is_empty()
            && repo
                .list_versions(id)
                .unwrap()
                .iter()
                .all(|v| v.status == ApiSpecStatus::Draft)
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn import_openapi_petstore_creates_drafts() {
    let ws = Workspace::new();
    let file = importer_fixture("openapi/mini_oas3.json");
    let out = ws.import("openapi", &file, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(ws.draft_count() > 0);
}

#[test]
fn import_postman_collection_creates_drafts() {
    let ws = Workspace::new();
    let file = importer_fixture("postman/sample_v21.json");
    let out = ws.import("postman", &file, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(ws.draft_count() > 0);
}

#[test]
fn import_curl_creates_draft() {
    let ws = Workspace::new();
    let curl_file = ws.dir.path().join("req.curl");
    fs::write(
        &curl_file,
        "curl -sS -X GET 'https://example.com/v1/health'",
    )
    .unwrap();
    let out = ws.import("curl", &curl_file, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(ws.draft_count(), 1);
}

#[test]
fn import_har_creates_drafts() {
    let ws = Workspace::new();
    let file = importer_fixture("har/sample.har.json");
    let out = ws.import("har", &file, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(ws.draft_count() > 0);
}

#[test]
fn import_markdown_cassette_creates_drafts_not_active() {
    let ws = Workspace::new();
    let md = ws.dir.path().join("api.md");
    fs::write(
        &md,
        "# API\n\n## GET /ping\nHealth check.\n\n## POST /orders\nCreate order.\n",
    )
    .unwrap();
    let cassettes = importer_fixture("cassettes");
    let out = ws.import(
        "md",
        &md,
        &[
            "--cassette-dir",
            cassettes.to_str().unwrap(),
            "--md-model",
            "cassette-md-plain-json",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(ws.has_draft("ping.get"));
    assert!(ws.has_draft("orders.post"));
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open(ws.db(), &clock).unwrap();
    let repo = ApiSpecRepository::new(db.conn(), &clock);
    assert!(repo.get_active("ping.get").unwrap().is_none());
}
