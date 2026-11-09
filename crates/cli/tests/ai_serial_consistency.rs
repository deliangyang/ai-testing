#![allow(clippy::unwrap_used)]

//! P2-26: cassette-replayed AI serial DAG, once activated, matches `examples/serial.yaml` on mock-sut.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use common::clock::{FixedClock, Timestamp};
use generator::{
    default_prompts_dir, generate_dag, ApiSchemaInput, CorrectionPreferences, DagGenerateInputs,
    DagGenerateRequestOptions, PromptCatalog, SkillContracts,
};
use llm_gateway::{CassetteProvider, MessageRole};
use mock_sut::MockSutBuilder;
use serde_json::Value;
use store::{DagRepository, Db};
use tokio::sync::Mutex;

static GOLDEN_MUTEX: Mutex<()> = Mutex::const_new(());

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cassette_dir() -> PathBuf {
    repo_root().join("crates/generator/tests/fixtures/cassettes")
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

    async fn run_dag_file(&self, dag: &Path, report: &Path) -> Output {
        self.mytool(&[
            "run",
            "--dag",
            dag.to_str().unwrap(),
            "--env",
            "local",
            "--env-root",
            self.dir.path().to_str().unwrap(),
            "--no-llm",
            "--report-json",
            report.to_str().unwrap(),
        ])
        .await
    }
}

fn deterministic_bytes(path: &Path) -> Vec<u8> {
    let report: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    model::to_canonical_json(&report["deterministic"])
        .unwrap()
        .into_bytes()
}

async fn spawn_sut() -> mock_sut::MockSutHandle {
    MockSutBuilder::new()
        .pay_delay_ms(0)
        .jitter_ms(0)
        .seed(1)
        .spawn()
        .await
        .unwrap()
}

async fn ai_serial_inputs() -> DagGenerateInputs {
    DagGenerateInputs {
        natural_language_description: "创建订单、支付、断言支付 HTTP 状态为 200".into(),
        business_rules: "串行 create → pay → assert，pay 路径引用 create 输出的订单 id".into(),
        api_schema: ApiSchemaInput::SchemaJson(r#"{"paths":{"/orders":{"post":{}}}}"#.into()),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    }
}

async fn generate_ai_serial_dag() -> model::Dag {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(cassette_dir()).unwrap();
    generate_dag(
        &provider,
        &tmpl,
        &ai_serial_inputs().await,
        &DagGenerateRequestOptions {
            model: "cassette-dag-serial-parity".into(),
            role: MessageRole::User,
        },
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn activated_ai_serial_matches_manual_example_on_mock_sut() {
    let _guard = GOLDEN_MUTEX.lock().await;
    install_rustls();

    let ai_dag = generate_ai_serial_dag().await;
    let manual_yaml = fs::read_to_string(repo_root().join("examples/serial.yaml")).unwrap();
    let manual_dag = model::Dag::from_yaml_str(&manual_yaml).unwrap();
    let ai_json = model::to_canonical_json(&ai_dag).unwrap();
    let manual_json = model::to_canonical_json(&manual_dag).unwrap();
    assert_eq!(
        ai_json, manual_json,
        "cassette AI DAG must match manual example"
    );

    let mut sut = spawn_sut().await;
    let run_manual = RunLayout::new(sut.base_url());
    let manual_report = run_manual.dir.path().join("manual.json");
    let manual_run = run_manual
        .run_dag_file(&repo_root().join("examples/serial.yaml"), &manual_report)
        .await;
    assert_eq!(
        manual_run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&manual_run.stderr)
    );
    sut.shutdown().await.unwrap();

    let store_ws = RunLayout::new("http://127.0.0.1:9");
    if let Some(parent) = store_ws.db().parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open(store_ws.db(), &clock).unwrap();
    let saved = DagRepository::new(db.conn(), &clock)
        .save_ai_candidate(&ai_dag)
        .unwrap();
    let activate = store_ws
        .mytool(&[
            "dag",
            "activate",
            &saved.info.id,
            &saved.info.version.to_string(),
        ])
        .await;
    assert_eq!(
        activate.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&activate.stderr)
    );

    let active_yaml = store_ws.dir.path().join("ai-serial-active.yaml");
    let db = Db::open(store_ws.db(), &clock).unwrap();
    let Some(active) = DagRepository::new(db.conn(), &clock)
        .active("serial")
        .unwrap()
    else {
        panic!("active serial");
    };
    fs::write(&active_yaml, active.dag.to_yaml().unwrap()).unwrap();

    sut = spawn_sut().await;
    let run_ai = RunLayout::new(sut.base_url());
    let ai_report = run_ai.dir.path().join("ai.json");
    let ai_run = run_ai.run_dag_file(&active_yaml, &ai_report).await;
    assert_eq!(
        ai_run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&ai_run.stderr)
    );

    assert_eq!(
        deterministic_bytes(&manual_report),
        deterministic_bytes(&ai_report),
        "AI-activated serial must match manual example byte-for-byte on deterministic report"
    );

    sut.shutdown().await.unwrap();
}
