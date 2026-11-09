//! `mytool api import` — parse external API definitions and store draft specs.

use std::path::Path;

use anyhow::Context as _;
use clap::ValueEnum;
use importer::{
    parse_import_file, persist_imported_specs, ImportFormat as PipelineFormat, MergeStrategy,
};

use crate::exit;
use crate::store::Store;

pub const IMPORT_CREATED_BY: &str = "api-import";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum ImportFormat {
    Openapi,
    Postman,
    Curl,
    Har,
    Md,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum ImportStrategy {
    Merge,
    Overwrite,
    Skip,
}

impl From<ImportStrategy> for MergeStrategy {
    fn from(value: ImportStrategy) -> Self {
        match value {
            ImportStrategy::Merge => MergeStrategy::Merge,
            ImportStrategy::Overwrite => MergeStrategy::Overwrite,
            ImportStrategy::Skip => MergeStrategy::Skip,
        }
    }
}

impl From<ImportFormat> for PipelineFormat {
    fn from(value: ImportFormat) -> Self {
        match value {
            ImportFormat::Openapi => PipelineFormat::OpenApi,
            ImportFormat::Postman => PipelineFormat::Postman,
            ImportFormat::Curl => PipelineFormat::Curl,
            ImportFormat::Har => PipelineFormat::Har,
            ImportFormat::Md => PipelineFormat::Markdown,
        }
    }
}

pub fn import(
    db_path: &Path,
    format: ImportFormat,
    file: &Path,
    strategy: ImportStrategy,
    cassette_dir: Option<&Path>,
    md_model: &str,
) -> anyhow::Result<u8> {
    let incoming = parse_import_file(format.into(), file, cassette_dir, md_model)
        .with_context(|| format!("parse {}", file.display()))?;
    let store = Store::open(db_path)?;
    let db = store.lock()?;
    let repo = store::ApiSpecRepository::new(db.conn(), store.clock.as_ref());
    let summary = persist_imported_specs(&repo, &incoming, strategy.into(), IMPORT_CREATED_BY)?;
    println!(
        "import complete: {} added, {} updated, {} unchanged (strategy={strategy:?})",
        summary.added, summary.updated, summary.unchanged
    );
    Ok(exit::SUCCESS)
}
