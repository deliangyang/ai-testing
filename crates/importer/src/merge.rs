//! Incremental merge of normalized [`ApiSpec`] collections (develop.md §4.10).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::api_spec::{normalize_api_spec, ApiParam, ApiSpec, ParamLocation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Union of fields; incoming wins on scalar conflicts; parameters merged by `(location, name)`.
    Merge,
    /// Incoming spec fully replaces the stored spec for the same `id`.
    Overwrite,
    /// Keep stored spec when `id` already exists; only append specs with new ids.
    Skip,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiSpecChange {
    pub id: String,
    pub before: ApiSpec,
    pub after: ApiSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiSpecDiff {
    pub added: Vec<ApiSpec>,
    pub removed: Vec<ApiSpec>,
    pub changed: Vec<ApiSpecChange>,
    pub unchanged: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MergeError {
    #[error("duplicate api spec id in input: {id}")]
    DuplicateId { id: String },
}

/// Compare two spec sets keyed by normalized `id` (deterministic ordering in the diff).
pub fn diff_api_specs(base: &[ApiSpec], incoming: &[ApiSpec]) -> Result<ApiSpecDiff, MergeError> {
    let base_map = index_specs(base)?;
    let incoming_map = index_specs(incoming)?;

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();

    for (id, after) in &incoming_map {
        match base_map.get(id) {
            None => added.push(after.clone()),
            Some(before) => {
                if specs_equal(before, after) {
                    unchanged.push(id.clone());
                } else {
                    changed.push(ApiSpecChange {
                        id: id.clone(),
                        before: before.clone(),
                        after: after.clone(),
                    });
                }
            }
        }
    }

    for (id, before) in &base_map {
        if !incoming_map.contains_key(id) {
            removed.push(before.clone());
        }
    }

    sort_specs_by_id(&mut added);
    sort_specs_by_id(&mut removed);
    changed.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(ApiSpecDiff {
        added,
        removed,
        changed,
        unchanged,
    })
}

/// Apply merge / overwrite / skip to produce the next spec list (sorted by `id`).
pub fn apply_merge_strategy(
    base: &[ApiSpec],
    incoming: &[ApiSpec],
    strategy: MergeStrategy,
) -> Result<Vec<ApiSpec>, MergeError> {
    let base_map = index_specs(base)?;
    let incoming_map = index_specs(incoming)?;

    let mut out: BTreeMap<String, ApiSpec> = BTreeMap::new();

    match strategy {
        MergeStrategy::Overwrite => {
            out = base_map;
            for (id, spec) in incoming_map {
                out.insert(id, spec);
            }
        }
        MergeStrategy::Skip => {
            out = base_map;
            for (id, spec) in incoming_map {
                out.entry(id).or_insert(spec);
            }
        }
        MergeStrategy::Merge => {
            for (id, spec) in base_map {
                out.insert(id, spec);
            }
            for (id, incoming_spec) in incoming_map {
                match out.remove(&id) {
                    None => {
                        out.insert(id, incoming_spec);
                    }
                    Some(base_spec) => {
                        out.insert(id, merge_pair(base_spec, incoming_spec));
                    }
                }
            }
        }
    }

    Ok(out.into_values().collect())
}

fn index_specs(specs: &[ApiSpec]) -> Result<BTreeMap<String, ApiSpec>, MergeError> {
    let mut map = BTreeMap::new();
    for spec in specs {
        let normalized = normalize_api_spec(spec.clone());
        let id = normalized.id.clone();
        if map.contains_key(&id) {
            return Err(MergeError::DuplicateId { id });
        }
        map.insert(id, normalized);
    }
    Ok(map)
}

fn specs_equal(a: &ApiSpec, b: &ApiSpec) -> bool {
    let a = normalize_api_spec(a.clone());
    let b = normalize_api_spec(b.clone());
    a.to_canonical_json()
        .ok()
        .zip(b.to_canonical_json().ok())
        .is_some_and(|(left, right)| left == right)
}

fn merge_pair(base: ApiSpec, incoming: ApiSpec) -> ApiSpec {
    let base = normalize_api_spec(base);
    let incoming = normalize_api_spec(incoming);
    debug_assert_eq!(base.id, incoming.id);

    let merged = ApiSpec {
        id: base.id,
        name: incoming.name,
        method: incoming.method,
        path: incoming.path,
        version: incoming.version.or(base.version),
        params: merge_params(&base.params, &incoming.params),
        response_schema: incoming.response_schema.or(base.response_schema),
        auth_scheme_ref: incoming.auth_scheme_ref.or(base.auth_scheme_ref),
    };
    normalize_api_spec(merged)
}

fn merge_params(base: &[ApiParam], incoming: &[ApiParam]) -> Vec<ApiParam> {
    let mut by_key: BTreeMap<(ParamLocation, String), ApiParam> = BTreeMap::new();
    for p in base {
        by_key.insert((p.location, p.name.clone()), p.clone());
    }
    for p in incoming {
        by_key.insert((p.location, p.name.clone()), p.clone());
    }
    let mut out: Vec<_> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.location.cmp(&b.location))
    });
    out
}

fn sort_specs_by_id(specs: &mut [ApiSpec]) {
    specs.sort_by(|a, b| a.id.cmp(&b.id));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    fn spec(
        id: &str,
        name: &str,
        params: Vec<ApiParam>,
        schema: Option<serde_json::Value>,
    ) -> ApiSpec {
        ApiSpec {
            id: id.into(),
            name: name.into(),
            method: "GET".into(),
            path: format!("/{id}"),
            version: None,
            params,
            response_schema: schema,
            auth_scheme_ref: None,
        }
    }

    fn param(name: &str, loc: ParamLocation) -> ApiParam {
        ApiParam {
            name: name.into(),
            location: loc,
            required: false,
            schema: None,
        }
    }

    #[test]
    fn diff_classifies_added_removed_changed() {
        let base = vec![spec("a", "A", vec![], None), spec("b", "B", vec![], None)];
        let incoming = vec![
            spec("a", "A", vec![], None),
            spec("b", "B2", vec![], None),
            spec("c", "C", vec![], None),
        ];
        let diff = diff_api_specs(&base, &incoming).unwrap();
        assert_eq!(diff.unchanged, vec!["a"]);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].id, "b");
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].id, "c");
        assert_eq!(diff.removed.len(), 0);
    }

    #[test]
    fn diff_reports_removed_when_missing_from_incoming() {
        let base = vec![spec("only", "Only", vec![], None)];
        let incoming = vec![];
        let diff = diff_api_specs(&base, &incoming).unwrap();
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].id, "only");
    }

    #[test]
    fn duplicate_id_in_base_is_error() {
        let base = vec![
            spec("dup", "One", vec![], None),
            spec("dup", "Two", vec![], None),
        ];
        let incoming = vec![];
        let err = diff_api_specs(&base, &incoming).unwrap_err();
        assert_eq!(err, MergeError::DuplicateId { id: "dup".into() });
    }

    #[test]
    fn overwrite_replaces_conflicting_spec() {
        let base = vec![spec("x", "Old", vec![], None)];
        let incoming = vec![spec("x", "New", vec![], Some(json!({"type":"object"})))];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Overwrite).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "New");
        assert!(merged[0].response_schema.is_some());
    }

    #[test]
    fn overwrite_does_not_drop_unmentioned_base_specs() {
        let base = vec![
            spec("keep", "Keep", vec![], None),
            spec("touch", "Touch", vec![], None),
        ];
        let incoming = vec![spec("touch", "Touched", vec![], None)];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Overwrite).unwrap();
        let ids: Vec<_> = merged.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["keep", "touch"]);
        assert_eq!(merged[1].name, "Touched");
    }

    #[test]
    fn skip_keeps_base_on_conflict() {
        let base = vec![spec(
            "x",
            "Base",
            vec![param("q", ParamLocation::Query)],
            None,
        )];
        let incoming = vec![spec(
            "x",
            "Incoming",
            vec![param("h", ParamLocation::Header)],
            Some(json!({"type":"string"})),
        )];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Skip).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "Base");
        assert_eq!(merged[0].params.len(), 1);
        assert_eq!(merged[0].params[0].name, "q");
        assert!(merged[0].response_schema.is_none());
    }

    #[test]
    fn skip_appends_only_new_ids() {
        let base = vec![spec("a", "A", vec![], None)];
        let incoming = vec![spec("b", "B", vec![], None)];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Skip).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "b");
    }

    #[test]
    fn merge_unions_params_and_fills_optional_fields() {
        let base = vec![spec(
            "m",
            "Base",
            vec![param("q", ParamLocation::Query)],
            Some(json!({"type":"object"})),
        )];
        let incoming = vec![spec(
            "m",
            "Incoming",
            vec![param("h", ParamLocation::Header)],
            None,
        )];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Merge).unwrap();
        let m = &merged[0];
        assert_eq!(m.name, "Incoming");
        let names: Vec<_> = m.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["h", "q"]);
        assert_eq!(m.response_schema, Some(json!({"type":"object"})));
    }

    #[test]
    fn merge_incoming_param_overrides_same_key() {
        let base = vec![spec(
            "m",
            "Base",
            vec![ApiParam {
                name: "q".into(),
                location: ParamLocation::Query,
                required: false,
                schema: None,
            }],
            None,
        )];
        let incoming = vec![spec(
            "m",
            "Incoming",
            vec![ApiParam {
                name: "q".into(),
                location: ParamLocation::Query,
                required: true,
                schema: Some(json!({"type":"integer"})),
            }],
            None,
        )];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Merge).unwrap();
        assert!(merged[0].params[0].required);
        assert_eq!(merged[0].params[0].schema, Some(json!({"type":"integer"})));
    }

    #[test]
    fn apply_output_sorted_by_id() {
        let base = vec![spec("z", "Z", vec![], None)];
        let incoming = vec![spec("a", "A", vec![], None), spec("m", "M", vec![], None)];
        let merged = apply_merge_strategy(&base, &incoming, MergeStrategy::Overwrite).unwrap();
        let ids: Vec<_> = merged.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "m", "z"]);
    }
}
