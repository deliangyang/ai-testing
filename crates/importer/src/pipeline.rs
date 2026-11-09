//! Parse imported files and persist merged drafts into the store.

use std::path::Path;

use store::{ApiSpecRecord, ApiSpecRepository};

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec};
use crate::merge::{apply_merge_strategy, diff_api_specs, MergeError, MergeStrategy};
use crate::{
    parse_curl_command, parse_har_str, parse_openapi, parse_postman_collection_str, ApiSpecDraft,
    CurlParseError, HarParseError, OpenApiImportError, PostmanParseError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    OpenApi,
    Postman,
    Curl,
    Har,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ImportPersistSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub saved_ids: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportPipelineError {
    #[error("read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("utf-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("openapi: {0}")]
    OpenApi(#[from] OpenApiImportError),
    #[error("postman: {0}")]
    Postman(#[from] PostmanParseError),
    #[error("curl: {0}")]
    Curl(#[from] CurlParseError),
    #[error("har: {0}")]
    Har(#[from] HarParseError),
    #[error("markdown cassette: {0}")]
    MarkdownCassette(#[from] crate::MarkdownCassetteError),
    #[error("merge: {0}")]
    Merge(#[from] MergeError),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
    #[error("markdown drafts: {0}")]
    MarkdownDraft(#[from] crate::MarkdownDraftParseError),
}

/// Parse UTF-8 content into review drafts (deterministic formats use confidence `1.0`).
pub fn parse_import_text(
    format: ImportFormat,
    text: &str,
    markdown_cassette_dir: Option<&Path>,
    markdown_model: &str,
) -> Result<Vec<ApiSpecDraft>, ImportPipelineError> {
    if matches!(format, ImportFormat::Markdown) {
        let dir = markdown_cassette_dir.ok_or_else(|| {
            ImportPipelineError::MarkdownCassette(crate::MarkdownCassetteError::DirMissing(
                "markdown import requires --cassette-dir or AITEST_CASSETTE_DIR".into(),
            ))
        })?;
        return Ok(crate::extract_api_spec_drafts_from_markdown_cassette(
            text,
            dir,
            markdown_model,
        )?
        .into_iter()
        .map(|mut d| {
            d.spec = normalize_api_spec(d.spec);
            d
        })
        .collect());
    }
    let specs = match format {
        ImportFormat::OpenApi => parse_openapi(text)?,
        ImportFormat::Postman => parse_postman_collection_str(text)?,
        ImportFormat::Curl => vec![parse_curl_command(text.trim())?],
        ImportFormat::Har => parse_har_str(text)?,
        ImportFormat::Markdown => unreachable!("handled above"),
    };
    Ok(specs
        .into_iter()
        .map(normalize_api_spec)
        .map(|spec| ApiSpecDraft {
            spec,
            confidence: 1.0,
            source_lines: Vec::new(),
        })
        .collect())
}

/// Parse a file into normalized specs (markdown uses cassette replay).
pub fn parse_import_file(
    format: ImportFormat,
    path: &Path,
    markdown_cassette_dir: Option<&Path>,
    markdown_model: &str,
) -> Result<Vec<ApiSpec>, ImportPipelineError> {
    let bytes = std::fs::read(path)?;
    let text = std::str::from_utf8(&bytes)?;
    let drafts = parse_import_text(format, text, markdown_cassette_dir, markdown_model)?;
    Ok(drafts.into_iter().map(|d| d.spec).collect())
}

/// Apply merge against effective store rows and save new drafts for added/changed specs.
pub fn persist_imported_specs(
    repo: &ApiSpecRepository<'_>,
    incoming: &[ApiSpec],
    strategy: MergeStrategy,
    created_by: &str,
) -> Result<ImportPersistSummary, ImportPipelineError> {
    let base = load_effective_api_specs(repo)?;
    let merged = apply_merge_strategy(&base, incoming, strategy)?;
    let diff = diff_api_specs(&base, &merged)?;

    let mut saved_ids = Vec::new();
    for spec in &diff.added {
        save_draft(repo, spec, created_by)?;
        saved_ids.push(spec.id.clone());
    }
    for change in &diff.changed {
        save_draft(repo, &change.after, created_by)?;
        saved_ids.push(change.after.id.clone());
    }
    saved_ids.sort();

    Ok(ImportPersistSummary {
        added: diff.added.len(),
        updated: diff.changed.len(),
        unchanged: diff.unchanged.len(),
        saved_ids,
    })
}

/// Markdown drafts extracted elsewhere (e.g. live LLM) with the same merge path.
pub fn persist_imported_drafts(
    repo: &ApiSpecRepository<'_>,
    drafts: &[ApiSpecDraft],
    strategy: MergeStrategy,
    created_by: &str,
) -> Result<ImportPersistSummary, ImportPipelineError> {
    let incoming: Vec<ApiSpec> = drafts.iter().map(|d| d.spec.clone()).collect();
    persist_imported_specs(repo, &incoming, strategy, created_by)
}

fn save_draft(
    repo: &ApiSpecRepository<'_>,
    spec: &ApiSpec,
    created_by: &str,
) -> Result<(), ImportPipelineError> {
    let input = normalize_api_spec(spec.clone()).into_save_input();
    repo.save(&input, created_by)?;
    Ok(())
}

pub(crate) fn load_effective_api_specs(
    repo: &ApiSpecRepository<'_>,
) -> Result<Vec<ApiSpec>, ImportPipelineError> {
    repo.list_effective_specs()?
        .into_iter()
        .map(record_to_api_spec)
        .collect()
}

fn record_to_api_spec(record: ApiSpecRecord) -> Result<ApiSpec, ImportPipelineError> {
    let params = match record.spec.params {
        Some(value) => serde_json::from_value::<Vec<ApiParam>>(value).map_err(|e| {
            ImportPipelineError::Store(store::StoreError::Corrupt(format!(
                "invalid stored params for {}: {e}",
                record.spec.id
            )))
        })?,
        None => Vec::new(),
    };
    Ok(normalize_api_spec(ApiSpec {
        id: record.spec.id,
        name: record.spec.name,
        method: record.spec.method,
        path: record.spec.path,
        version: Some(record.info.version),
        params,
        response_schema: record.spec.response_schema,
        auth_scheme_ref: record.spec.auth_scheme_ref,
    }))
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use common::clock::{FixedClock, Timestamp};
    use store::Db;

    #[test]
    fn merge_skip_leaves_existing_unchanged() {
        let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
        let db = Db::open_in_memory(&clock).unwrap();
        let repo = ApiSpecRepository::new(db.conn(), &clock);
        let existing = ApiSpec {
            id: "a.get".into(),
            name: "A".into(),
            method: "GET".into(),
            path: "/a".into(),
            version: None,
            params: vec![],
            response_schema: None,
            auth_scheme_ref: None,
        };
        repo.save(&existing.clone().into_save_input(), "seed")
            .unwrap();
        let incoming = vec![existing];
        let summary =
            persist_imported_specs(&repo, &incoming, MergeStrategy::Skip, "import").unwrap();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.updated, 0);
        assert_eq!(summary.unchanged, 1);
    }
}
