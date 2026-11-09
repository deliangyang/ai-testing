//! Assemble [`DagGenerateContext`] from user inputs; redact before LLM-bound rendering.

use std::collections::BTreeMap;

use llm_gateway::{redact_chat_request, redact_text, ChatMessage, ChatRequest, MessageRole};
use model::to_canonical_json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::PromptError;
use crate::render::{render, DagGenerateContext};
use crate::template::PromptTemplate;

pub const SKILL_CONTRACTS_EMPTY: &str = "(empty)";
pub const PREFERENCES_EMPTY: &str = "(empty)";
const RULES_EMPTY: &str = "(none)";

/// HTTP endpoint schema document (shape-compatible with `importer::ApiSpec`, no crate edge).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EndpointDoc {
    pub id: String,
    pub name: String,
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<EndpointParam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_scheme_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EndpointParam {
    pub name: String,
    pub location: ParamLocation,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ParamLocation {
    Query,
    Path,
    Header,
    Body,
}

/// Skill contract slice of the prompt; empty until Skill packages land (P3+).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillContracts(pub Option<String>);

/// Correction-preference slice; empty until P4-18/P4-19.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorrectionPreferences(pub Option<String>);

#[derive(Debug, Clone, PartialEq)]
pub enum ApiSchemaInput {
    /// OpenAPI document or other schema JSON verbatim (after trim).
    SchemaJson(String),
    /// Normalized endpoint list (e.g. values converted from `importer::ApiSpec` via JSON).
    EndpointDocs(Vec<EndpointDoc>),
}

/// Raw inputs for DAG generation (develop.md §6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct DagGenerateInputs {
    pub natural_language_description: String,
    pub business_rules: String,
    pub api_schema: ApiSchemaInput,
    pub skill_contracts: SkillContracts,
    pub preferences: CorrectionPreferences,
}

/// Redacted placeholder values ready for [`render`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationContext {
    pub dag: DagGenerateContext,
}

impl GenerationContext {
    pub fn assemble(inputs: &DagGenerateInputs) -> Result<Self, PromptError> {
        let dag = assemble_dag_generate_context(inputs)?;
        Ok(Self {
            dag: redact_dag_generate_context(dag),
        })
    }

    pub fn rendered_prompt(&self, template: &PromptTemplate) -> String {
        let body = render(template, &self.dag);
        redact_text(&body)
    }

    pub fn redacted_chat_request(
        &self,
        template: &PromptTemplate,
        model: &str,
        role: MessageRole,
    ) -> ChatRequest {
        let content = render(template, &self.dag);
        redact_chat_request(ChatRequest {
            model: model.to_owned(),
            messages: vec![ChatMessage { role, content }],
        })
    }
}

pub fn assemble_dag_generate_context(
    inputs: &DagGenerateInputs,
) -> Result<DagGenerateContext, PromptError> {
    let openapi_json = api_schema_prompt_json(&inputs.api_schema)?;
    Ok(DagGenerateContext {
        openapi_json,
        business_rules: format_business_rules_block(
            &inputs.natural_language_description,
            &inputs.business_rules,
        ),
        skill_contracts: skill_contracts_text(&inputs.skill_contracts),
        preference_rules: preferences_text(&inputs.preferences),
    })
}

#[must_use]
pub fn redact_dag_generate_context(ctx: DagGenerateContext) -> DagGenerateContext {
    DagGenerateContext {
        openapi_json: redact_text(&ctx.openapi_json),
        business_rules: redact_text(&ctx.business_rules),
        skill_contracts: redact_text(&ctx.skill_contracts),
        preference_rules: redact_text(&ctx.preference_rules),
    }
}

pub fn api_schema_prompt_json(input: &ApiSchemaInput) -> Result<String, PromptError> {
    match input {
        ApiSchemaInput::SchemaJson(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok("{}".into());
            }
            Ok(trimmed.to_owned())
        }
        ApiSchemaInput::EndpointDocs(specs) => endpoint_docs_canonical_json(specs),
    }
}

/// Parse endpoint docs from JSON (e.g. serialized `importer::ApiSpec` values).
pub fn parse_endpoint_docs_json(json: &str) -> Result<Vec<EndpointDoc>, PromptError> {
    let value: Value = serde_json::from_str(json.trim())
        .map_err(|e| PromptError::InvalidApiSchema(e.to_string()))?;
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|item| {
                serde_json::from_value(item)
                    .map_err(|e| PromptError::InvalidApiSchema(e.to_string()))
            })
            .collect(),
        one => {
            let doc: EndpointDoc = serde_json::from_value(one)
                .map_err(|e| PromptError::InvalidApiSchema(e.to_string()))?;
            Ok(vec![doc])
        }
    }
}

pub fn normalize_endpoint_doc(mut doc: EndpointDoc) -> EndpointDoc {
    doc.id = doc.id.trim().to_owned();
    doc.name = doc.name.trim().to_owned();
    doc.method = doc.method.trim().to_uppercase();
    let path = doc.path.trim();
    doc.path = if path.is_empty() || path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    doc.params.sort_by(|a, b| a.name.cmp(&b.name));
    for param in &mut doc.params {
        param.name = param.name.trim().to_owned();
    }
    if let Some(schema) = doc.response_schema.as_mut() {
        if let Ok(canonical) = to_canonical_json(schema) {
            if let Ok(parsed) = serde_json::from_str(&canonical) {
                *schema = parsed;
            }
        }
    }
    doc
}

fn endpoint_docs_canonical_json(specs: &[EndpointDoc]) -> Result<String, PromptError> {
    let mut normalized: Vec<EndpointDoc> =
        specs.iter().cloned().map(normalize_endpoint_doc).collect();
    normalized.sort_by(|a, b| a.id.cmp(&b.id));
    let mut rows = Vec::with_capacity(normalized.len());
    for spec in normalized {
        let mut map = BTreeMap::new();
        map.insert("id".to_owned(), Value::String(spec.id));
        map.insert("method".to_owned(), Value::String(spec.method));
        map.insert("name".to_owned(), Value::String(spec.name));
        map.insert("path".to_owned(), Value::String(spec.path));
        if let Some(auth) = spec.auth_scheme_ref {
            map.insert("auth_scheme_ref".to_owned(), Value::String(auth));
        }
        if !spec.params.is_empty() {
            map.insert(
                "params".to_owned(),
                serde_json::to_value(&spec.params).unwrap_or(Value::Null),
            );
        }
        if let Some(schema) = spec.response_schema {
            map.insert("response_schema".to_owned(), schema);
        }
        if let Some(version) = spec.version {
            map.insert(
                "version".to_owned(),
                Value::Number(serde_json::Number::from(version)),
            );
        }
        rows.push(Value::Object(map.into_iter().collect()));
    }
    to_canonical_json(&rows).map_err(PromptError::from)
}

fn format_business_rules_block(description: &str, rules: &str) -> String {
    let desc = description.trim();
    let rules = rules.trim();
    match (desc.is_empty(), rules.is_empty()) {
        (true, true) => RULES_EMPTY.into(),
        (false, true) => desc.into(),
        (true, false) => rules.into(),
        (false, false) => format!("{desc}\n\n{rules}"),
    }
}

fn skill_contracts_text(s: &SkillContracts) -> String {
    s.0.as_ref()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| SKILL_CONTRACTS_EMPTY.into())
}

fn preferences_text(p: &CorrectionPreferences) -> String {
    p.0.as_ref()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PREFERENCES_EMPTY.into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::loader::{default_prompts_dir, PromptCatalog};
    use llm_gateway::REDACTED;

    const RAW_SK: &str = "sk-abcdefghijklmnopqrstuvwxyz123456";

    #[test]
    fn redact_context_strips_secrets_from_placeholders() {
        let ctx = DagGenerateContext {
            openapi_json: format!("key: {RAW_SK}"),
            business_rules: "OPENAI_API_KEY=sk-live-from-env".into(),
            skill_contracts: "(empty)".into(),
            preference_rules: "(empty)".into(),
        };
        let redacted = redact_dag_generate_context(ctx);
        assert!(!redacted.openapi_json.contains(RAW_SK));
        assert!(!redacted.business_rules.contains("sk-live-from-env"));
        assert!(redacted.openapi_json.contains(REDACTED));
    }

    #[test]
    fn generation_context_rendered_prompt_is_fully_redacted() {
        let inputs = DagGenerateInputs {
            natural_language_description: format!("use token {RAW_SK}"),
            business_rules: String::new(),
            api_schema: ApiSchemaInput::SchemaJson("{}".into()),
            skill_contracts: SkillContracts::default(),
            preferences: CorrectionPreferences::default(),
        };
        let ctx = GenerationContext::assemble(&inputs).unwrap();
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let tmpl = catalog.load_dag_generate_main().unwrap();
        let prompt = ctx.rendered_prompt(&tmpl);
        assert!(!prompt.contains(RAW_SK));
        assert!(prompt.contains(REDACTED));
    }

    #[test]
    fn endpoint_docs_sorted_and_normalized_in_schema_json() {
        let docs = vec![
            EndpointDoc {
                id: "b".into(),
                name: "B".into(),
                method: "post".into(),
                path: "b".into(),
                version: None,
                params: vec![],
                response_schema: None,
                auth_scheme_ref: None,
            },
            EndpointDoc {
                id: "a".into(),
                name: "A".into(),
                method: "get".into(),
                path: "/a".into(),
                version: None,
                params: vec![],
                response_schema: None,
                auth_scheme_ref: None,
            },
        ];
        let json = api_schema_prompt_json(&ApiSchemaInput::EndpointDocs(docs)).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr[0]["id"], "a");
        assert_eq!(arr[0]["method"], "GET");
        assert_eq!(arr[1]["id"], "b");
        assert_eq!(arr[1]["path"], "/b");
    }
}
