//! `mytool run`: execute a DAG deterministically, printing progress as it happens.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context as _};
use common::{TokioSleeper, UuidV7Gen};
use engine::{
    event_channel, load_results, CancelToken, EngineEvent, Environment, HttpExecutor, LogLevel,
    NodeRouter, NodeStatus, RuleClassifier, RunOutcome, RunRecorder, Scheduler, StoreRecorder,
    SubworkflowExecutor, SystemEnv,
};
use model::{to_canonical_json, Dag};
use serde_json::{json, Value};
use store::{
    DagRepository, NewRun, NodeExecution, NodeExecutionRepository, RunMode, RunRepository,
    RunStatus,
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::args::{OutputFormat, RunArgs};
use crate::dag::{print_issues, rejected, CREATED_BY};
use crate::exit;
use crate::input::{load_dag, Loaded};
use crate::store::Store;

pub fn run(db_path: &Path, args: &RunArgs) -> anyhow::Result<u8> {
    // multi_thread: scheduler records via spawn_blocking while print_events runs concurrently.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    runtime.block_on(run_async(db_path, args))
}

/// A DAG ready to run: its content and the stored version runs are recorded against.
struct Target {
    dag: Dag,
    version: u32,
}

async fn run_async(db_path: &Path, args: &RunArgs) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let partial = partial_seed(&store, args)?;
    let (dag_ref, env_name) = if let Some(seed_meta) = &partial {
        let source = &seed_meta.source;
        (
            args.dag.as_deref().unwrap_or(source.dag_id.as_str()),
            args.env.as_deref().unwrap_or(source.env_name.as_str()),
        )
    } else {
        (
            args.dag
                .as_deref()
                .ok_or_else(|| anyhow!("--dag is required (or use `run show <run-id>`)"))?,
            args.env
                .as_deref()
                .ok_or_else(|| anyhow!("--env is required"))?,
        )
    };
    let target = match resolve(&store, dag_ref)? {
        Ok(target) => target,
        Err(code) => return Ok(code),
    };
    if let Some(seed_meta) = &partial {
        let source = &seed_meta.source;
        let from_node = &seed_meta.from_node;
        if target.dag.id != source.dag_id {
            return Err(anyhow!(
                "DAG '{}' does not match source run '{}' (dag '{}')",
                target.dag.id,
                source.id,
                source.dag_id
            ));
        }
        if target.version != source.dag_version {
            return Err(anyhow!(
                "DAG version {} does not match source run '{}' (version {})",
                target.version,
                source.id,
                source.dag_version
            ));
        }
        if target.dag.node(from_node).is_none() {
            return Err(anyhow!(
                "node '{from_node}' is not in DAG '{}'",
                target.dag.id
            ));
        }
    }
    let env = Environment::load(&args.env_root, env_name, &SystemEnv)?;
    env.check_dag(&target.dag)?;

    let parallel = usize::from(args.parallel);
    let http = Arc::new(HttpExecutor::new(store.clock.clone())?);
    let router = NodeRouter::new(http, store.clock.clone(), Arc::new(TokioSleeper))
        .with_subworkflows(
            SubworkflowExecutor::new(Arc::new(store.dag_source())).with_max_parallel(parallel),
        );
    let recorder = Arc::new(StoreRecorder::new(
        store.db.clone(),
        store.clock.clone(),
        Arc::new(UuidV7Gen),
        Arc::new(RuleClassifier::new()?),
    ));
    let run = recorder.begin_run(&NewRun {
        dag_id: target.dag.id.clone(),
        dag_version: target.version,
        env_name: env.name().to_owned(),
        mode: RunMode::Manual,
    })?;
    println!(
        "run {} of DAG '{}' version {} in environment '{}'",
        run.id,
        target.dag.id,
        target.version,
        env.name()
    );

    let (sink, events) = event_channel();
    let printer = tokio::spawn(print_events(events));
    let cancel = CancelToken::new();
    let interrupt = tokio::spawn(cancel_on_ctrl_c(cancel.clone()));
    let env = Arc::new(env);
    let sink_recorder: Arc<dyn RunRecorder> = recorder.clone();
    let mut scheduler = Scheduler::new(Arc::new(router))
        .with_shared_environment(Arc::clone(&env))
        .with_max_parallel(parallel)
        .with_events(Some(sink))
        .with_recorder(Some(sink_recorder))
        .with_cancel(cancel);
    if let Some(seed_meta) = partial {
        let db = store.lock()?;
        let seed = load_results(&db, store.clock.as_ref(), &seed_meta.source.id)?;
        scheduler = scheduler.with_partial_rerun(seed_meta.from_node, seed);
    }
    let result = scheduler.run(&run.id, &target.dag).await;
    interrupt.abort();
    // Drop the event sink so `print_events` can finish after `RunFinished`.
    drop(scheduler);
    printer.await.context("progress printer failed")?;

    let outcome = result?;
    let finished = recorder.finish_run(&run.id, &outcome)?;
    if let Some(path) = &args.report_json {
        let classifier = RuleClassifier::new()?;
        crate::report::write(
            path,
            &crate::report::build(&finished, &outcome, &env, &classifier),
        )
        .with_context(|| format!("cannot write report {}", path.display()))?;
    }
    print_summary(&run.id, &outcome, &env)?;
    Ok(match finished.status {
        RunStatus::Completed => exit::SUCCESS,
        RunStatus::Cancelled => exit::CANCELLED,
        _ => exit::RUN_FAILED,
    })
}

struct PartialSeed {
    source: store::RunRecord,
    from_node: String,
}

fn partial_seed(store: &Store, args: &RunArgs) -> anyhow::Result<Option<PartialSeed>> {
    match (&args.from_run, &args.from_node) {
        (None, None) => Ok(None),
        (Some(run_id), Some(from_node)) => {
            let db = store.lock()?;
            let source = RunRepository::new(db.conn(), store.clock.as_ref(), &UuidV7Gen)
                .get(run_id)?
                .ok_or_else(|| anyhow!("run '{run_id}' not found"))?;
            Ok(Some(PartialSeed {
                source,
                from_node: from_node.clone(),
            }))
        }
        _ => Err(anyhow!("--from-run and --from-node must be used together")),
    }
}

/// `Err` carries the exit code when the DAG cannot be run.
fn resolve(store: &Store, dag: &str) -> anyhow::Result<Result<Target, u8>> {
    let path = Path::new(dag);
    if !is_file_ref(path) {
        let db = store.lock()?;
        let record = DagRepository::new(db.conn(), store.clock.as_ref())
            .active(dag)?
            .ok_or_else(|| anyhow!("DAG '{dag}' has no active version"))?;
        return Ok(Ok(Target {
            version: record.info.version,
            dag: record.dag,
        }));
    }
    let label = path.display().to_string();
    let dag = match load_dag(path)? {
        Loaded::Dag(dag) => *dag,
        Loaded::Invalid(report) => return Ok(Err(rejected(&label, &report)?)),
    };
    let report = store.gate(&dag);
    if !report.passed() {
        return Ok(Err(rejected(&label, &report)?));
    }
    print_issues(&report);
    let version = stored_version(store, &dag)?;
    Ok(Ok(Target { dag, version }))
}

fn is_file_ref(path: &Path) -> bool {
    path.is_file()
        || path.extension().is_some_and(|ext| {
            ["yaml", "yml", "json"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// An existing version with identical content, or a new pending one (ADR-005).
fn stored_version(store: &Store, dag: &Dag) -> anyhow::Result<u32> {
    let db = store.lock()?;
    let repo = DagRepository::new(db.conn(), store.clock.as_ref());
    let mut content = dag.clone();
    content.version = None;
    for info in repo.versions(&dag.id)?.iter().rev() {
        if let Some(mut record) = repo.get(&dag.id, info.version)? {
            record.dag.version = None;
            if record.dag == content {
                return Ok(info.version);
            }
        }
    }
    let saved = repo.save(&content, CREATED_BY)?;
    println!(
        "saved DAG '{}' version {} (pending) for this run",
        saved.info.id, saved.info.version
    );
    Ok(saved.info.version)
}

async fn cancel_on_ctrl_c(cancel: CancelToken) {
    if tokio::signal::ctrl_c().await.is_ok() {
        eprintln!("interrupted: cancelling the run, cleanup still runs");
        cancel.cancel();
    }
}

async fn print_events(mut events: UnboundedReceiver<EngineEvent>) {
    while let Some(event) = events.recv().await {
        match event {
            EngineEvent::NodeStateChanged { node, status } => match status {
                NodeStatus::Pending | NodeStatus::Ready => {}
                _ => println!("  {node}: {status}"),
            },
            EngineEvent::PollIteration {
                node,
                attempt,
                entry,
            } => match entry.get("actual") {
                Some(actual) => println!("  {node}: poll attempt {attempt}, actual {actual}"),
                None => println!("  {node}: poll attempt {attempt}"),
            },
            EngineEvent::Log {
                node,
                level,
                message,
            } => {
                let level = match level {
                    LogLevel::Info => "info",
                    LogLevel::Warn => "warn",
                    LogLevel::Error => "error",
                };
                match node {
                    Some(node) => println!("  [{level}] {node}: {message}"),
                    None => println!("  [{level}] {message}"),
                }
            }
            EngineEvent::RunFinished { .. } => {}
        }
    }
}

fn print_summary(run_id: &str, outcome: &RunOutcome, env: &Environment) -> anyhow::Result<()> {
    let classifier = RuleClassifier::new()?;
    for (id, node) in &outcome.nodes {
        if node.status != NodeStatus::Failed {
            continue;
        }
        if let Some(error) = &node.error {
            let class = classifier.classify_error(error);
            let message = env.mask(&Value::String(error.message.clone()));
            println!(
                "FAILED {id} [{}/{}]: {}",
                class.category.as_str(),
                class.code.as_str(),
                message.as_str().unwrap_or_default()
            );
        }
    }
    let count = |status: NodeStatus| {
        outcome
            .nodes
            .values()
            .filter(|n| n.status == status)
            .count()
    };
    println!(
        "run {run_id}: {} ({} completed, {} failed, {} skipped)",
        outcome.status(),
        count(NodeStatus::Completed),
        count(NodeStatus::Failed),
        count(NodeStatus::Skipped)
    );
    Ok(())
}

/// Replay a recorded run's evidence.
pub fn show(
    db_path: &Path,
    run_id: &str,
    node: Option<&str>,
    format: OutputFormat,
) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let db = store.lock()?;
    let run = RunRepository::new(db.conn(), store.clock.as_ref(), &UuidV7Gen)
        .get(run_id)?
        .ok_or_else(|| anyhow!("run '{run_id}' not found"))?;
    let mut rows =
        NodeExecutionRepository::new(db.conn(), store.clock.as_ref()).list_for_run(run_id)?;
    if let Some(id) = node {
        rows.retain(|row| row.key.node_id == id);
        if rows.is_empty() {
            return Err(anyhow!(
                "run '{run_id}' has no node '{id}' (frame paths are like 'login/session')"
            ));
        }
    }
    match format {
        OutputFormat::Json => println!(
            "{}",
            to_canonical_json(&json!({
                "run": {
                    "id": run.id,
                    "dag_id": run.dag_id,
                    "dag_version": run.dag_version,
                    "env": run.env_name,
                    "status": run.status.to_string(),
                    "started_at": run.started_at.map(|t| t.to_string()),
                    "finished_at": run.finished_at.map(|t| t.to_string()),
                },
                "nodes": rows.iter().map(node_json).collect::<Vec<_>>(),
            }))?
        ),
        OutputFormat::Text => {
            println!(
                "run {} of DAG '{}' version {} in environment '{}': {}",
                run.id, run.dag_id, run.dag_version, run.env_name, run.status
            );
            for row in &rows {
                print_node(row);
            }
        }
    }
    Ok(exit::SUCCESS)
}

fn node_json(row: &NodeExecution) -> Value {
    json!({
        "id": row.key.node_id,
        "attempt": row.key.attempt,
        "status": row.status,
        "failure": row.failure.as_ref().map(|f| json!({
            "category": f.category,
            "error_code": f.error_code,
        })),
        "started_at": row.started_at.map(|t| t.to_string()),
        "finished_at": row.finished_at.map(|t| t.to_string()),
        "input": row.input_resolved,
        "output": row.output,
        "assertions": assertions_of(row),
        "poll": row.poll_trace,
    })
}

fn print_node(row: &NodeExecution) {
    print!("  {}: {}", row.key.node_id, row.status);
    if let Some(failure) = &row.failure {
        print!(" [{}/{}]", failure.category, failure.error_code);
    }
    println!();
    if let Some(input) = &row.input_resolved {
        println!("    request:");
        print_json("      ", input);
    }
    if let Some(output) = &row.output {
        if assertions_of(row).is_none() {
            println!("    response:");
            print_json("      ", output);
        }
    }
    if let Some(assertions) = assertions_of(row) {
        println!("    assertions:");
        if let Some(items) = assertions.as_array() {
            for item in items {
                print_assertion(item);
            }
        } else {
            print_json("      ", &assertions);
        }
    }
    if let Some(trace) = &row.poll_trace {
        println!("    poll:");
        if let Some(items) = trace.as_array() {
            for item in items {
                print_poll(item);
            }
        } else {
            print_json("      ", trace);
        }
    }
}

fn assertions_of(row: &NodeExecution) -> Option<Value> {
    let output = row.output.as_ref()?;
    output
        .get("results")
        .cloned()
        .or_else(|| output.get("detail")?.get("results").cloned())
}

fn print_assertion(item: &Value) {
    let passed = item.get("passed").and_then(Value::as_bool).unwrap_or(false);
    let mark = if passed { "PASS" } else { "FAIL" };
    let path = item.get("path").and_then(Value::as_str).unwrap_or("?");
    let op = item.get("operator").and_then(Value::as_str).unwrap_or("?");
    print!("      {mark} {path} {op}");
    if let Some(expected) = item.get("expected") {
        print!(" expected={expected}");
    }
    if let Some(actual) = item.get("actual") {
        print!(" actual={actual}");
    }
    if let Some(message) = item.get("message").and_then(Value::as_str) {
        print!(" ({message})");
    }
    println!();
}

fn print_poll(item: &Value) {
    let attempt = item.get("attempt").unwrap_or(&Value::Null);
    let passed = item.get("passed").and_then(Value::as_bool).unwrap_or(false);
    let mark = if passed { "PASS" } else { "FAIL" };
    let ts = item.get("ts").and_then(Value::as_str).unwrap_or("");
    print!("      #{attempt} {mark}");
    if !ts.is_empty() {
        print!(" ts={ts}");
    }
    if let Some(actual) = item.get("actual") {
        print!(" actual={actual}");
    }
    if let Some(expected) = item.get("expected") {
        print!(" expected={expected}");
    }
    if let Some(diff) = item.get("diff").and_then(Value::as_str) {
        print!(" {diff}");
    }
    if let Some(error) = item.get("error") {
        print!(" error={error}");
    }
    println!();
}

fn print_json(indent: &str, value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    for line in rendered.lines() {
        println!("{indent}{line}");
    }
}
