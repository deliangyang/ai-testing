//! `mytool run`: execute a DAG deterministically, printing progress as it happens.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context as _};
use common::{TokioSleeper, UuidV7Gen};
use engine::{
    event_channel, CancelToken, EngineEvent, Environment, HttpExecutor, LogLevel, NodeRouter,
    NodeStatus, RuleClassifier, RunOutcome, RunRecorder, Scheduler, StoreRecorder,
    SubworkflowExecutor, SystemEnv,
};
use model::Dag;
use serde_json::Value;
use store::{DagRepository, NewRun, RunMode, RunStatus};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::args::RunArgs;
use crate::dag::{print_issues, rejected, CREATED_BY};
use crate::exit;
use crate::input::{load_dag, Loaded};
use crate::store::Store;

pub fn run(db_path: &Path, args: &RunArgs) -> anyhow::Result<u8> {
    let runtime = tokio::runtime::Builder::new_current_thread()
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
    let target = match resolve(&store, &args.dag)? {
        Ok(target) => target,
        Err(code) => return Ok(code),
    };
    let env = Environment::load(&args.env_root, &args.env, &SystemEnv)?;
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
    let result = Scheduler::new(Arc::new(router))
        .with_shared_environment(Arc::clone(&env))
        .with_max_parallel(parallel)
        .with_events(Some(sink))
        .with_recorder(Some(sink_recorder))
        .with_cancel(cancel)
        .run(&run.id, &target.dag)
        .await;
    interrupt.abort();
    printer.await.context("progress printer failed")?;

    let outcome = result?;
    let finished = recorder.finish_run(&run.id, &outcome)?;
    print_summary(&run.id, &outcome, &env)?;
    Ok(match finished.status {
        RunStatus::Completed => exit::SUCCESS,
        RunStatus::Cancelled => exit::CANCELLED,
        _ => exit::RUN_FAILED,
    })
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
