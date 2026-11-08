#![allow(clippy::unwrap_used)]

//! `mytool run` end to end: the real binary against wiremock and a temporary database.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use common::{FixedClock, Timestamp, UuidV7Gen};
use serde_json::json;
use store::{Db, RunRepository, RunStatus};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Workspace {
    dir: tempfile::TempDir,
}

impl Workspace {
    /// With `environments/local.yaml` pointing at `base_url`.
    fn new(base_url: &str, variables: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("environments")).unwrap();
        fs::write(
            dir.path().join("environments/local.yaml"),
            format!("base_url: {base_url}\nvariables:\n{variables}"),
        )
        .unwrap();
        Self { dir }
    }

    fn db(&self) -> PathBuf {
        self.dir.path().join("aitest.db")
    }

    fn file(&self, name: &str, yaml: &str) -> String {
        let path = self.dir.path().join(name);
        fs::write(&path, yaml).unwrap();
        path.to_str().unwrap().to_owned()
    }

    async fn mytool(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_mytool"));
        cmd.args(args)
            .env("AITEST_DB", self.db())
            .env("AITEST_TEST_SECRET", "s3cr3t-value")
            .env_remove("RUST_LOG")
            .current_dir(self.dir.path());
        tokio::task::spawn_blocking(move || cmd.output().unwrap())
            .await
            .unwrap()
    }

    async fn run(&self, dag: &str, extra: &[&str]) -> Output {
        let mut args = vec!["run", "--dag", dag, "--env", "local", "--no-llm"];
        args.extend_from_slice(extra);
        self.mytool(&args).await
    }

    fn run_status(&self, out: &Output) -> RunStatus {
        let id = run_id(out);
        let id = id.as_str();
        let clock = FixedClock::new(Timestamp::from_unix_millis(0));
        let db = Db::open(self.db(), &clock).unwrap();
        RunRepository::new(db.conn(), &clock, &UuidV7Gen)
            .get(id)
            .unwrap()
            .unwrap()
            .status
    }
}

fn run_id(output: &Output) -> String {
    stdout(output)
        .lines()
        .find_map(|l| l.strip_prefix("run ")?.split(' ').next())
        .unwrap_or_else(|| panic!("no run id in:\n{}", stdout(output)))
        .to_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

async fn serve(server: &MockServer, verb: &str, route: &str, body: serde_json::Value) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn shop() -> MockServer {
    let server = MockServer::start().await;
    serve(&server, "POST", "/orders", json!({"id": "o-1"})).await;
    serve(&server, "POST", "/orders/o-1/pay", json!({"paid": true})).await;
    serve(&server, "GET", "/stock/A-1", json!({"left": 3})).await;
    serve(&server, "GET", "/price/A-1", json!({"amount": 10})).await;
    server
}

const SERIAL: &str = r#"
id: serial
nodes:
  - { id: create, type: http, method: POST, path: /orders }
  - { id: pay, type: http, method: POST, path: "/orders/${create.output.body.id}/pay" }
  - id: check
    type: assert
    target: pay
    assertions: [{ path: $.status, op: eq, expected: 200 }]
edges:
  - { from: create, to: pay }
  - { from: pay, to: check }
"#;

const PARALLEL: &str = r#"
id: parallel
nodes:
  - { id: create, type: http, method: POST, path: /orders }
  - { id: stock, type: http, method: GET, path: /stock/A-1 }
  - { id: price, type: http, method: GET, path: /price/A-1 }
  - id: check
    type: assert
    target: stock
    assertions:
      - { path: $.status, op: eq, expected: 200 }
      - { path: $.status, op: eq, expected: "${price.output.status}" }
edges:
  - { from: create, to: stock }
  - { from: create, to: price }
  - { from: stock, to: check }
  - { from: price, to: check }
"#;

const POLL: &str = r#"
id: async
nodes:
  - { id: create, type: http, method: POST, path: /orders }
  - { id: pay, type: http, method: POST, path: "/orders/${create.output.body.id}/pay" }
  - id: wait_paid
    type: poll
    request: { method: GET, path: "/orders/${create.output.body.id}" }
    condition: { path: $.body.status, op: eq, expected: PAID }
    backoff_ms: [10, 20]
    timeout_ms: 5000
  - id: check
    type: assert
    target: pay
    assertions: [{ path: $.status, op: eq, expected: 200 }]
edges:
  - { from: create, to: pay }
  - { from: pay, to: wait_paid }
  - { from: pay, to: check }
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_serial_flow_completes_and_prints_progress() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let out = ws.run(&ws.file("serial.yaml", SERIAL), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(text.contains("saved DAG 'serial' version 1 (pending) for this run"));
    let order: Vec<usize> = ["create: RUNNING", "create: COMPLETED", "pay: COMPLETED"]
        .iter()
        .map(|line| text.find(line).unwrap_or_else(|| panic!("{line}: {text}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{text}");
    assert!(
        text.contains(": COMPLETED (3 completed, 0 failed, 0 skipped)"),
        "{text}"
    );
    assert_eq!(ws.run_status(&out), RunStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_parallel_branch_completes() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let out = ws
        .run(&ws.file("parallel.yaml", PARALLEL), &["--parallel", "2"])
        .await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(stdout(&out).contains("(4 completed, 0 failed, 0 skipped)"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_async_flow_polls_until_the_payment_lands() {
    let server = shop().await;
    Mock::given(method("GET"))
        .and(path("/orders/o-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "PENDING"})))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    serve(&server, "GET", "/orders/o-1", json!({"status": "PAID"})).await;
    let ws = Workspace::new(&server.uri(), "");
    let out = ws.run(&ws.file("async.yaml", POLL), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(text.contains("wait_paid: POLLING"), "{text}");
    assert!(text.contains("wait_paid: poll attempt 3"), "{text}");
    assert!(text.contains("wait_paid: COMPLETED"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_assertion_exits_1_with_its_classification() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let failing = SERIAL.replace("expected: 200", "expected: 201");
    let out = ws.run(&ws.file("serial.yaml", &failing), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(
        text.contains("FAILED check [assertion_failed_status/ASSERT_STATUS_MISMATCH]"),
        "{text}"
    );
    assert!(
        text.contains(": FAILED (2 completed, 1 failed, 0 skipped)"),
        "{text}"
    );
    assert_eq!(ws.run_status(&out), RunStatus::Failed);
}

#[tokio::test(flavor = "multi_thread")]
async fn rerunning_the_same_file_reuses_its_version() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let file = ws.file("serial.yaml", SERIAL);
    ws.run(&file, &[]).await;
    let again = ws.run(&file, &[]).await;
    assert_eq!(again.status.code(), Some(0));
    assert!(!stdout(&again).contains("saved DAG"), "{}", stdout(&again));
    assert!(stdout(&again).contains("of DAG 'serial' version 1 "));

    ws.file(
        "serial.yaml",
        &SERIAL.replace("path: /orders }", "path: /orders, timeout_ms: 2000 }"),
    );
    let changed = ws.run(&file, &[]).await;
    assert!(
        stdout(&changed).contains("version 2 (pending) for this run"),
        "{}",
        stdout(&changed)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stored_dag_runs_its_active_version() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let file = ws.file("serial.yaml", SERIAL);

    let out = ws.run("serial", &[]).await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("DAG 'serial' has no active version"));

    ws.mytool(&["dag", "save", &file]).await;
    ws.mytool(&["dag", "activate", "serial", "1"]).await;
    let out = ws.run("serial", &[]).await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(stdout(&out).contains("of DAG 'serial' version 1 "));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_rejected_by_the_gate_never_runs() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let cyclic = format!("{SERIAL}  - {{ from: check, to: create }}\n");
    let out = ws.run(&ws.file("bad.yaml", &cyclic), &[]).await;
    assert_eq!(out.status.code(), Some(3));
    assert!(stdout(&out).contains("error[graph.cycle]"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_problems_exit_2_before_anything_runs() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let file = ws.file("serial.yaml", SERIAL);

    let out = ws
        .mytool(&["run", "--dag", &file, "--env", "staging"])
        .await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("staging"), "{}", stderr(&out));

    let needs_var = SERIAL.replace("path: /orders }", "path: \"/orders?u=${env:USER_ID}\" }");
    let out = ws.run(&ws.file("var.yaml", &needs_var), &[]).await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("USER_ID"), "{}", stderr(&out));
    assert!(server.received_requests().await.unwrap().is_empty());
}

fn parse_report(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn canonical(value: &serde_json::Value) -> String {
    model::to_canonical_json(value).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn report_json_partitions_clocks_and_is_stable_across_runs() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let file = ws.file("serial.yaml", SERIAL);
    let first_path = ws.dir.path().join("out/first.json");
    let second_path = ws.dir.path().join("out/second.json");
    let first = ws
        .run(&file, &["--report-json", first_path.to_str().unwrap()])
        .await;
    let second = ws
        .run(
            &file,
            &[
                "--report-json",
                second_path.to_str().unwrap(),
                "--parallel",
                "1",
            ],
        )
        .await;
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}{}",
        stdout(&first),
        stderr(&first)
    );
    assert_eq!(
        second.status.code(),
        Some(0),
        "{}{}",
        stdout(&second),
        stderr(&second)
    );

    let a = parse_report(&first_path);
    let b = parse_report(&second_path);
    assert_eq!(a["schema"], "aitest.run-report/1");
    assert_eq!(
        canonical(&a["deterministic"]),
        canonical(&b["deterministic"])
    );
    assert_ne!(a["volatile"]["run_id"], b["volatile"]["run_id"]);
    assert!(a["volatile"]["started_at"].is_string());
    assert!(a["volatile"]["finished_at"].is_string());
    assert_eq!(a["deterministic"]["status"], "COMPLETED");
    assert_eq!(a["deterministic"]["nodes"]["check"]["status"], "COMPLETED");
    assert_eq!(
        a["deterministic"]["nodes"]["check"]["assertions"][0]["passed"],
        true
    );
    assert!(a["deterministic"]["nodes"]["create"]["output"]["timing"].is_null());
    assert!(
        a["volatile"]["nodes"]["create"]["timing"]["duration_ms"].is_number(),
        "{a}"
    );

    let on_disk = fs::read_to_string(&first_path).unwrap();
    assert_eq!(on_disk, canonical(&a));
}

#[tokio::test(flavor = "multi_thread")]
async fn report_deterministic_section_does_not_depend_on_parallelism() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let file = ws.file("parallel.yaml", PARALLEL);
    let one = ws.dir.path().join("p1.json");
    let four = ws.dir.path().join("p4.json");
    let a = ws
        .run(
            &file,
            &["--parallel", "1", "--report-json", one.to_str().unwrap()],
        )
        .await;
    let b = ws
        .run(
            &file,
            &["--parallel", "4", "--report-json", four.to_str().unwrap()],
        )
        .await;
    assert_eq!(a.status.code(), Some(0), "{}{}", stdout(&a), stderr(&a));
    assert_eq!(b.status.code(), Some(0), "{}{}", stdout(&b), stderr(&b));
    assert_eq!(
        canonical(&parse_report(&one)["deterministic"]),
        canonical(&parse_report(&four)["deterministic"])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn report_json_records_classification_for_failures() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let path = ws.dir.path().join("fail.json");
    let out = ws
        .run(
            &ws.file(
                "serial.yaml",
                &SERIAL.replace("expected: 200", "expected: 201"),
            ),
            &["--report-json", path.to_str().unwrap()],
        )
        .await;
    assert_eq!(out.status.code(), Some(1));
    let report = parse_report(&path);
    let check = &report["deterministic"]["nodes"]["check"];
    assert_eq!(check["status"], "FAILED");
    assert_eq!(check["failure"]["category"], "assertion_failed_status");
    assert_eq!(check["failure"]["error_code"], "ASSERT_STATUS_MISMATCH");
    assert_eq!(check["assertions"][0]["passed"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn report_json_masks_secrets() {
    let ws = Workspace::new(
        "http://127.0.0.1:1",
        "  TOKEN: \"${env:AITEST_TEST_SECRET}\"\n",
    );
    let path = ws.dir.path().join("secret.json");
    let dag = "id: secret\nnodes:\n  - { id: call, type: http, method: GET, path: \"/x?t=${env:TOKEN}\" }\n";
    let out = ws
        .run(
            &ws.file("secret.yaml", dag),
            &["--report-json", path.to_str().unwrap()],
        )
        .await;
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("s3cr3t-value"), "{text}");
    assert!(text.contains("****"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn process_secrets_are_masked_in_the_output() {
    let ws = Workspace::new(
        "http://127.0.0.1:1",
        "  TOKEN: \"${env:AITEST_TEST_SECRET}\"\n",
    );
    let dag = "id: secret\nnodes:\n  - { id: call, type: http, method: GET, path: \"/x?t=${env:TOKEN}\" }\n";
    let out = ws.run(&ws.file("secret.yaml", dag), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(text.contains("FAILED call [connection_error/"), "{text}");
    assert!(!text.contains("s3cr3t-value"), "{text}");
    assert!(text.contains("****"), "{text}");
}

const TIMEOUT_POLL: &str = r#"
id: timeout-poll
nodes:
  - { id: create, type: http, method: POST, path: /orders }
  - id: wait_paid
    type: poll
    request: { method: GET, path: "/orders/${create.output.body.id}" }
    condition: { path: $.body.status, op: eq, expected: PAID }
    backoff_ms: [20]
    timeout_ms: 80
    max_attempts: 20
edges:
  - { from: create, to: wait_paid }
"#;

#[tokio::test(flavor = "multi_thread")]
async fn run_show_replays_request_response_and_assertions() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let out = ws.run(&ws.file("serial.yaml", SERIAL), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let id = run_id(&out);

    let shown = ws.mytool(&["run", "show", &id]).await;
    assert_eq!(
        shown.status.code(),
        Some(0),
        "{}{}",
        stdout(&shown),
        stderr(&shown)
    );
    let text = stdout(&shown);
    assert!(text.contains("of DAG 'serial' version 1"), "{text}");
    assert!(text.contains("create: COMPLETED"), "{text}");
    assert!(text.contains("request:"), "{text}");
    assert!(text.contains("response:"), "{text}");
    assert!(text.contains("check: COMPLETED"), "{text}");
    assert!(text.contains("PASS $.status eq expected=200"), "{text}");

    let only = ws.mytool(&["run", "show", &id, "--node", "check"]).await;
    let only_text = stdout(&only);
    assert!(only_text.contains("check: COMPLETED"), "{only_text}");
    assert!(!only_text.contains("create:"), "{only_text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_show_replays_a_timed_out_poll_trace() {
    let server = shop().await;
    Mock::given(method("GET"))
        .and(path("/orders/o-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "PENDING"})))
        .mount(&server)
        .await;
    let ws = Workspace::new(&server.uri(), "");
    let out = ws.run(&ws.file("timeout.yaml", TIMEOUT_POLL), &[]).await;
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let id = run_id(&out);

    let json_out = ws
        .mytool(&[
            "run",
            "show",
            &id,
            "--node",
            "wait_paid",
            "--format",
            "json",
        ])
        .await;
    assert_eq!(
        json_out.status.code(),
        Some(0),
        "{}{}",
        stdout(&json_out),
        stderr(&json_out)
    );
    let doc: serde_json::Value = serde_json::from_str(&stdout(&json_out)).unwrap();
    assert_eq!(doc["nodes"][0]["id"], "wait_paid");
    assert_eq!(doc["nodes"][0]["status"], "FAILED");
    assert_eq!(doc["nodes"][0]["failure"]["category"], "timeout");
    let poll = doc["nodes"][0]["poll"].as_array().unwrap();
    assert!(!poll.is_empty(), "{doc}");
    for (i, entry) in poll.iter().enumerate() {
        assert_eq!(entry["attempt"], i as u64 + 1, "{entry}");
        assert!(entry["ts"].as_str().unwrap().contains('T'), "{entry}");
        assert_eq!(entry["passed"], false);
        assert_eq!(entry["actual"], "PENDING");
        assert_eq!(entry["expected"], "PAID");
    }

    let text = stdout(
        &ws.mytool(&["run", "show", &id, "--node", "wait_paid"])
            .await,
    );
    assert!(text.contains(&format!("#{} FAIL", poll.len())), "{text}");
    assert!(text.contains("ts="), "{text}");
    assert!(text.contains("actual=\"PENDING\""), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_show_missing_run_or_node_exits_2() {
    let server = shop().await;
    let ws = Workspace::new(&server.uri(), "");
    let missing = ws.mytool(&["run", "show", "no-such-run"]).await;
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("no-such-run"));

    let out = ws.run(&ws.file("serial.yaml", SERIAL), &[]).await;
    let ghost = ws
        .mytool(&["run", "show", &run_id(&out), "--node", "ghost"])
        .await;
    assert_eq!(ghost.status.code(), Some(2));
    assert!(stderr(&ghost).contains("ghost"));
}
