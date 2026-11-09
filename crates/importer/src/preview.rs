//! Import preview and confirmed commit (develop.md §6.1).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use store::ApiSpecRepository;

use crate::api_spec::{normalize_api_spec, ApiSpec};
use crate::markdown_draft::ApiSpecDraft;
use crate::merge::{apply_merge_strategy, diff_api_specs, MergeStrategy};
use crate::pipeline::{load_effective_api_specs, persist_imported_specs, ImportPipelineError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportMergeAction {
    Add,
    Update,
    Unchanged,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportPreviewItem {
    pub spec: ApiSpec,
    pub confidence: f64,
    pub source_lines: Vec<u32>,
    pub merge_action: ImportMergeAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImportPreviewSummary {
    pub add: usize,
    pub update: usize,
    pub unchanged: usize,
    pub skip: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportPreviewResponse {
    pub strategy: MergeStrategy,
    pub items: Vec<ImportPreviewItem>,
    pub summary: ImportPreviewSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportCommitItem {
    pub spec: ApiSpec,
    pub accepted: bool,
}

/// Resolved merge outcome for each parsed draft under `strategy`.
pub fn build_import_preview(
    repo: &ApiSpecRepository<'_>,
    drafts: &[ApiSpecDraft],
    strategy: MergeStrategy,
) -> Result<ImportPreviewResponse, ImportPipelineError> {
    let base = load_effective_api_specs(repo)?;
    let base_map = specs_by_id(&base);
    let incoming: Vec<ApiSpec> = drafts.iter().map(|d| d.spec.clone()).collect();
    let merged = apply_merge_strategy(&base, &incoming, strategy)?;
    let diff = diff_api_specs(&base, &merged)?;

    let mut items: Vec<ImportPreviewItem> = drafts
        .iter()
        .map(|draft| ImportPreviewItem {
            spec: draft.spec.clone(),
            confidence: draft.confidence,
            source_lines: draft.source_lines.clone(),
            merge_action: merge_action_for_draft(&base_map, draft, strategy, &diff),
        })
        .collect();
    items.sort_by(|a, b| a.spec.id.cmp(&b.spec.id));

    let summary = summarize(&items);
    Ok(ImportPreviewResponse {
        strategy,
        items,
        summary,
    })
}

/// Persist only `accepted` rows from the review queue.
pub fn persist_confirmed_import(
    repo: &ApiSpecRepository<'_>,
    items: &[ImportCommitItem],
    strategy: MergeStrategy,
    created_by: &str,
) -> Result<crate::pipeline::ImportPersistSummary, ImportPipelineError> {
    let incoming: Vec<ApiSpec> = items
        .iter()
        .filter(|row| row.accepted)
        .map(|row| normalize_api_spec(row.spec.clone()))
        .collect();
    persist_imported_specs(repo, &incoming, strategy, created_by)
}

fn specs_by_id(specs: &[ApiSpec]) -> BTreeMap<String, ApiSpec> {
    specs
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect::<BTreeMap<_, _>>()
}

fn merge_action_for_draft(
    base_map: &BTreeMap<String, ApiSpec>,
    draft: &ApiSpecDraft,
    strategy: MergeStrategy,
    diff: &crate::merge::ApiSpecDiff,
) -> ImportMergeAction {
    let id = &draft.spec.id;
    if diff.added.iter().any(|s| &s.id == id) {
        return ImportMergeAction::Add;
    }
    if strategy == MergeStrategy::Skip {
        if let Some(base_spec) = base_map.get(id) {
            let same = normalize_api_spec(base_spec.clone())
                .to_canonical_json()
                .ok()
                == normalize_api_spec(draft.spec.clone())
                    .to_canonical_json()
                    .ok();
            return if same {
                ImportMergeAction::Unchanged
            } else {
                ImportMergeAction::Skip
            };
        }
    }
    if diff.changed.iter().any(|c| &c.id == id) {
        return ImportMergeAction::Update;
    }
    if diff.unchanged.iter().any(|u| u == id) {
        return ImportMergeAction::Unchanged;
    }
    ImportMergeAction::Add
}

fn summarize(items: &[ImportPreviewItem]) -> ImportPreviewSummary {
    let mut summary = ImportPreviewSummary {
        add: 0,
        update: 0,
        unchanged: 0,
        skip: 0,
    };
    for item in items {
        match item.merge_action {
            ImportMergeAction::Add => summary.add += 1,
            ImportMergeAction::Update => summary.update += 1,
            ImportMergeAction::Unchanged => summary.unchanged += 1,
            ImportMergeAction::Skip => summary.skip += 1,
        }
    }
    summary
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use common::clock::{FixedClock, Timestamp};
    use store::Db;

    #[test]
    fn skip_marks_existing_conflicts_as_skip() {
        let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
        let db = Db::open_in_memory(&clock).unwrap();
        let repo = ApiSpecRepository::new(db.conn(), &clock);
        let base = ApiSpec {
            id: "get.foo".into(),
            name: "Foo".into(),
            method: "GET".into(),
            path: "/foo".into(),
            version: None,
            params: vec![],
            response_schema: None,
            auth_scheme_ref: None,
        };
        repo.save(&base.clone().into_save_input(), "seed").unwrap();
        let draft = ApiSpecDraft {
            spec: ApiSpec {
                name: "Foo changed".into(),
                ..base
            },
            confidence: 1.0,
            source_lines: vec![],
        };
        let preview = build_import_preview(&repo, &[draft], MergeStrategy::Skip).unwrap();
        assert_eq!(preview.items[0].merge_action, ImportMergeAction::Skip);
        assert_eq!(preview.summary.skip, 1);
    }
}
