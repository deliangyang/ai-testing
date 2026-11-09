//! Load versioned prompts from `prompts/` and render placeholders for LLM calls.

mod cache_key;
mod context;
mod error;
mod eval;
mod gate_loop;
mod generate;
mod loader;
mod parse;
mod render;
mod repair;
mod template;

pub use cache_key::{cache_key_hash, cache_key_material, CacheKeyMaterial};
pub use context::{
    api_schema_prompt_json, assemble_dag_generate_context, normalize_endpoint_doc,
    parse_endpoint_docs_json, redact_dag_generate_context, ApiSchemaInput, CorrectionPreferences,
    DagGenerateInputs, EndpointDoc, EndpointParam, GenerationContext, ParamLocation,
    SkillContracts, PREFERENCES_EMPTY, SKILL_CONTRACTS_EMPTY,
};
pub use error::{GenerateError, PromptError};
pub use eval::{
    default_eval_chains, run_cassette_eval, run_default_cassette_eval, EvalChain, EvalSummary,
    EvalTrial, TARGET_ACCEPTANCE_RATE,
};
pub use gate_loop::{
    format_gate_feedback_json, generate_dag_with_gate, DagGenerateWithGateOptions,
    GenerateDagWithGateResult,
};
pub use generate::{generate_dag, DagGenerateRequestOptions};
pub use loader::{default_prompts_dir, PromptCatalog};
pub use parse::{extract_dag_json, parse_dag_from_llm_content};
pub use render::{render, DagGenerateContext};
pub use repair::{repair_dag_with_gate, repair_prompt_preview};
pub use template::PromptTemplate;

pub use llm_gateway as llm;
