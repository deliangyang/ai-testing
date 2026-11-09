#![allow(clippy::unwrap_used)]

//! P0-53: official `examples/` DAGs on mock-sut produce byte-stable deterministic reports.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use mock_sut::MockSutBuilder;
use serde_json::Value;
use tokio::sync::Mutex;

/// `mock-sut` + `mytool` share process-global state; run golden cases one at a time.
static GOLDEN_MUTEX: Mutex<()> = Mutex::const_new(());

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn examples_dir() -> PathBuf {
    repo_root().join("examples")
}

fn install_rustls() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

struct RunLayout {
    dir: tempfile::TempDir,
}

impl RunLayout {
    fn new(base_url: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("environments")).unwrap();
        Self { dir }.with_base_url(base_url)
    }

    fn with_base_url(self, base_url: &str) -> Self {
        fs::write(
            self.dir.path().join("environments/local.yaml"),
            format!("base_url: {base_url}\n"),
        )
        .unwrap();
        self
    }

    fn db(&self) -> PathBuf {
        self.dir.path().join("aitest.db")
    }

    async fn mytool(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_mytool"));
        cmd.args(args)
            .env("AITEST_DB", self.db())
            .env_remove("RUST_LOG")
            .current_dir(self.dir.path());
        tokio::task::spawn_blocking(move || cmd.output().unwrap())
            .await
            .unwrap()
    }

    async fn run_example(&self, dag_file: &str, extra: &[&str]) -> Output {
        let dag = examples_dir().join(dag_file);
        let mut args = vec![
            "run",
            "--dag",
            dag.to_str().unwrap(),
            "--env",
            "local",
            "--env-root",
            self.dir.path().to_str().unwrap(),
            "--no-llm",
        ];
        args.extend_from_slice(extra);
        self.mytool(&args).await
    }
}

fn parse_report(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn deterministic_canonical_bytes(path: &Path) -> Vec<u8> {
    let report = parse_report(path);
    model::to_canonical_json(&report["deterministic"])
        .unwrap()
        .into_bytes()
}

async fn spawn_sut() -> mock_sut::MockSutHandle {
    MockSutBuilder::new()
        // After pay, status flips PAID only after this delay; first poll is immediate, second after 10ms backoff.
        .pay_delay_ms(5)
        .jitter_ms(0)
        .seed(1)
        .spawn()
        .await
        .unwrap()
}

async fn assert_twice_same_deterministic(dag_file: &str) {
    let _guard = GOLDEN_MUTEX.lock().await;
    install_rustls();
    let mut sut = spawn_sut().await;
    let layout = RunLayout::new(sut.base_url());

    let first = layout.dir.path().join("first.json");
    let second = layout.dir.path().join("second.json");
    let a = layout
        .run_example(dag_file, &["--report-json", first.to_str().unwrap()])
        .await;
    sut.shutdown().await.unwrap();
    sut = spawn_sut().await;
    let layout = layout.with_base_url(sut.base_url());
    let b = layout
        .run_example(
            dag_file,
            &["--report-json", second.to_str().unwrap(), "--parallel", "1"],
        )
        .await;
    assert_eq!(
        a.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&a.stderr)
    );
    assert_eq!(
        b.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&b.stderr)
    );

    let da = deterministic_canonical_bytes(&first);
    let db = deterministic_canonical_bytes(&second);
    assert_eq!(da, db, "deterministic report must match byte-for-byte");
    assert_ne!(
        parse_report(&first)["volatile"]["run_id"],
        parse_report(&second)["volatile"]["run_id"]
    );

    sut.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn example_serial_report_is_golden_stable() {
    assert_twice_same_deterministic("serial.yaml").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn example_parallel_report_is_golden_stable() {
    assert_twice_same_deterministic("parallel.yaml").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn example_async_poll_report_is_golden_stable() {
    assert_twice_same_deterministic("async-poll.yaml").await;
}

#[test]
fn cli_dependency_tree_excludes_llm_crates() {
    let forbidden = ["llm-gateway", "generator", "autonomy"];
    let output = Command::new("cargo")
        .current_dir(repo_root())
        .args([
            "tree",
            "-p",
            "cli",
            "-e",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    for name in forbidden {
        let pattern = format!("{name} v");
        assert!(
            !tree.lines().any(|line| line.starts_with(&pattern)),
            "cli must not depend on {name}\n{tree}"
        );
    }
}
