//! `mytool skill ...`

use std::path::Path;

use anyhow::anyhow;
use gate::Gate;
use model::to_canonical_json;
use serde_json::json;
use store::{SkillRepository, SkillVersionInfo};

use crate::args::OutputFormat;
use crate::dag::{print_issues, rejected};
use crate::exit;
use crate::input::load_skill;
use crate::store::Store;

pub fn save(db_path: &Path, file: &Path) -> anyhow::Result<u8> {
    let label = file.display().to_string();
    let package = match load_skill(file)? {
        Ok(pkg) => pkg,
        Err(report) => return rejected(&label, &report),
    };
    let report = Gate::standard().validate(&package.dag);
    if !report.passed() {
        return rejected(&label, &report);
    }
    print_issues(&report);
    let store = Store::open(db_path)?;
    let db = store.lock()?;
    let saved = SkillRepository::new(db.conn()).save(&package)?;
    println!(
        "saved Skill '{}' revision {} (manifest {}, pending)",
        saved.info.id, saved.info.version, saved.info.manifest_version
    );
    Ok(exit::SUCCESS)
}

pub fn activate(db_path: &Path, id: &str, version: u32) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let record = {
        let db = store.lock()?;
        SkillRepository::new(db.conn())
            .get(id, version)?
            .ok_or_else(|| anyhow!("Skill '{id}' has no revision {version}"))?
    };
    let report = Gate::standard().validate(&record.package.dag);
    if !report.passed() {
        return rejected(&format!("Skill '{id}' revision {version}"), &report);
    }
    print_issues(&report);
    let db = store.lock()?;
    let repo = SkillRepository::new(db.conn());
    let previous = repo.active_version(id)?;
    repo.activate(id, version, previous)?;
    match previous {
        Some(old) if old != version => {
            println!("activated Skill '{id}' revision {version} (revision {old} archived)")
        }
        Some(_) => println!("Skill '{id}' revision {version} is already active"),
        None => println!("activated Skill '{id}' revision {version}"),
    }
    Ok(exit::SUCCESS)
}

pub fn list(db_path: &Path, format: OutputFormat) -> anyhow::Result<u8> {
    let store = Store::open(db_path)?;
    let versions = {
        let db = store.lock()?;
        SkillRepository::new(db.conn()).list()?
    };
    match format {
        OutputFormat::Json => {
            let rows: Vec<_> = versions.iter().map(version_json).collect();
            println!("{}", to_canonical_json(&rows)?);
        }
        OutputFormat::Text if versions.is_empty() => println!("no Skills stored"),
        OutputFormat::Text => print_versions(&versions),
    }
    Ok(exit::SUCCESS)
}

fn version_json(v: &SkillVersionInfo) -> serde_json::Value {
    json!({
        "id": v.id,
        "version": v.version,
        "manifest_version": v.manifest_version,
        "name": v.name,
        "status": v.status.as_str(),
        "tsx_plugin_ref": v.tsx_plugin_ref,
    })
}

fn print_versions(versions: &[SkillVersionInfo]) {
    let width = versions
        .iter()
        .map(|v| v.id.len())
        .max()
        .unwrap_or(0)
        .max("ID".len());
    println!("{:<width$}  REV  MANIFEST   STATUS    NAME", "ID");
    for v in versions {
        println!(
            "{:<width$}  {:<3}  {:<10}  {:<8}  {}",
            v.id,
            v.version,
            v.manifest_version,
            v.status.as_str(),
            v.name
        );
    }
}
