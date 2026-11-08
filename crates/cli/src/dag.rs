//! `mytool dag ...`

use std::path::Path;

use anyhow::anyhow;
use gate::{Gate, Report};
use model::to_canonical_json;
use serde_json::json;
use store::{DagRepository, DagVersionInfo};

use crate::args::OutputFormat;
use crate::exit;
use crate::input::{load_dag, Loaded};
use crate::store::Store;

/// Recorded as `created_by` for versions saved from the command line.
pub const CREATED_BY: &str = "cli";

/// Exit 0 when the gate passes (warnings allowed), 3 when it reports errors or the file is not
/// a DAG.
pub fn validate(file: &Path, format: OutputFormat) -> anyhow::Result<u8> {
    let report = match load_dag(file)? {
        Loaded::Dag(dag) => Gate::standard().validate(&dag),
        Loaded::Invalid(report) => report,
    };
    print_report(&file.display().to_string(), &report, format)?;
    Ok(if report.passed() {
        exit::SUCCESS
    } else {
        exit::INVALID
    })
}

/// Stores the file as a new pending version, but only if it passes the save gate (standard
/// rules plus subworkflow references against the stored DAGs).
pub fn save(db_path: &Path, file: &Path) -> anyhow::Result<u8> {
    let label = file.display().to_string();
    let dag = match load_dag(file)? {
        Loaded::Dag(dag) => dag,
        Loaded::Invalid(report) => return rejected(&label, &report),
    };
    let store = Store::open(db_path)?;
    let report = store.gate(&dag);
    if !report.passed() {
        return rejected(&label, &report);
    }
    print_issues(&report);
    let db = store.lock()?;
    let saved = DagRepository::new(db.conn(), store.clock.as_ref()).save(&dag, CREATED_BY)?;
    println!(
        "saved DAG '{}' version {} (pending)",
        saved.info.id, saved.info.version
    );
    Ok(exit::SUCCESS)
}

/// Activation is the only way a version becomes active, so the stored content goes through
/// the save gate again: the DAGs it references may have changed since it was saved.
pub fn activate(db_path: &Path, id: &str, version: u32) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let dag = {
        let db = store.lock()?;
        DagRepository::new(db.conn(), store.clock.as_ref())
            .get(id, version)?
            .ok_or_else(|| anyhow!("DAG '{id}' has no version {version}"))?
            .dag
    };
    let report = store.gate(&dag);
    if !report.passed() {
        return rejected(&format!("DAG '{id}' version {version}"), &report);
    }
    print_issues(&report);
    let db = store.lock()?;
    let repo = DagRepository::new(db.conn(), store.clock.as_ref());
    let previous = repo.active_version(id)?;
    repo.activate(id, version, previous)?;
    match previous {
        Some(old) if old != version => {
            println!("activated DAG '{id}' version {version} (version {old} archived)")
        }
        Some(_) => println!("DAG '{id}' version {version} is already active"),
        None => println!("activated DAG '{id}' version {version}"),
    }
    Ok(exit::SUCCESS)
}

pub fn list(db_path: &Path, format: OutputFormat) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let versions = {
        let db = store.lock()?;
        DagRepository::new(db.conn(), store.clock.as_ref()).list()?
    };
    match format {
        OutputFormat::Json => {
            let rows: Vec<_> = versions.iter().map(version_json).collect();
            println!("{}", to_canonical_json(&rows)?);
        }
        OutputFormat::Text if versions.is_empty() => println!("no DAGs stored"),
        OutputFormat::Text => print_versions(&versions),
    }
    Ok(exit::SUCCESS)
}

pub fn rejected(label: &str, report: &Report) -> anyhow::Result<u8> {
    print_report(label, report, OutputFormat::Text)?;
    Ok(exit::INVALID)
}

fn print_report(label: &str, report: &Report, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            let doc = json!({
                "file": label,
                "passed": report.passed(),
                "issues": report.issues,
            });
            println!("{}", to_canonical_json(&doc)?);
        }
        OutputFormat::Text => {
            print_issues(report);
            let errors = report.errors().count();
            let warnings = report.warnings().count();
            let verdict = if report.passed() {
                "passed"
            } else {
                "rejected"
            };
            println!(
                "{label}: {verdict} ({errors} error{}, {warnings} warning{})",
                plural(errors),
                plural(warnings)
            );
        }
    }
    Ok(())
}

pub fn print_issues(report: &Report) {
    for issue in &report.issues {
        println!("{issue}");
    }
}

fn version_json(v: &DagVersionInfo) -> serde_json::Value {
    json!({
        "id": v.id,
        "version": v.version,
        "status": v.status.as_str(),
        "created_by": v.created_by,
        "created_at": v.created_at.to_string(),
    })
}

fn print_versions(versions: &[DagVersionInfo]) {
    let width = versions
        .iter()
        .map(|v| v.id.len())
        .max()
        .unwrap_or(0)
        .max("ID".len());
    println!(
        "{:<width$}  VERSION  STATUS    CREATED_AT                CREATED_BY",
        "ID"
    );
    for v in versions {
        println!(
            "{:<width$}  {:<7}  {:<8}  {:<24}  {}",
            v.id,
            v.version,
            v.status.as_str(),
            v.created_at.to_string(),
            v.created_by
        );
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
