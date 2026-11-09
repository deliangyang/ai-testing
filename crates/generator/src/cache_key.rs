use std::collections::BTreeMap;

use model::content_hash;
use serde::Serialize;

use crate::error::PromptError;
use crate::render::DagGenerateContext;
use crate::template::PromptTemplate;

/// Inputs that participate in LLM response caches (P3-14 and similar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheKeyMaterial {
    pub template_id: String,
    pub prompt_version: String,
    pub placeholders: BTreeMap<String, String>,
}

impl CacheKeyMaterial {
    pub fn from_dag_generate(template: &PromptTemplate, ctx: &DagGenerateContext) -> Self {
        let mut placeholders = BTreeMap::new();
        for (name, value) in ctx.placeholder_map() {
            placeholders.insert(name.to_owned(), value.to_owned());
        }
        Self {
            template_id: template.id.clone(),
            prompt_version: template.prompt_version.clone(),
            placeholders,
        }
    }

    /// Lowercase hex SHA-256 of the canonical JSON material.
    pub fn hash(&self) -> Result<String, PromptError> {
        Ok(content_hash(self)?)
    }
}

/// Build cache key material for a rendered DAG-generation prompt.
pub fn cache_key_material(template: &PromptTemplate, ctx: &DagGenerateContext) -> CacheKeyMaterial {
    CacheKeyMaterial::from_dag_generate(template, ctx)
}

/// Convenience: canonical hash of [`cache_key_material`].
pub fn cache_key_hash(
    template: &PromptTemplate,
    ctx: &DagGenerateContext,
) -> Result<String, PromptError> {
    cache_key_material(template, ctx).hash()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::loader::{default_prompts_dir, PromptCatalog};

    fn sample_ctx() -> DagGenerateContext {
        DagGenerateContext {
            openapi_json: "{}".into(),
            business_rules: "b".into(),
            skill_contracts: "s".into(),
            preference_rules: "p".into(),
        }
    }

    #[test]
    fn cache_key_stable_for_same_inputs() {
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let tmpl = catalog.load_dag_generate_main().unwrap();
        let ctx = sample_ctx();
        let a = cache_key_hash(&tmpl, &ctx).unwrap();
        let b = cache_key_hash(&tmpl, &ctx).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn cache_key_changes_with_prompt_version() {
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let mut tmpl = catalog.load_dag_generate_main().unwrap();
        let ctx = sample_ctx();
        let v1 = cache_key_hash(&tmpl, &ctx).unwrap();
        tmpl.prompt_version = "1.0.1".into();
        let v2 = cache_key_hash(&tmpl, &ctx).unwrap();
        assert_ne!(v1, v2);
    }

    #[test]
    fn cache_key_changes_with_placeholder_value() {
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let tmpl = catalog.load_dag_generate_main().unwrap();
        let a = cache_key_hash(&tmpl, &sample_ctx()).unwrap();
        let mut other = sample_ctx();
        other.business_rules = "other".into();
        let b = cache_key_hash(&tmpl, &other).unwrap();
        assert_ne!(a, b);
    }
}
