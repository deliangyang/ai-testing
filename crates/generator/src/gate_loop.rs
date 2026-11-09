//! Generate a DAG via LLM, validate with [`gate::Gate`], and feed issue lists back into the prompt.

use gate::{Gate, Issue, Report};
use llm_gateway::{redact_chat_request, redact_text, ChatMessage, ChatRequest, LlmProvider};
use model::{to_canonical_json, Dag};

use crate::context::{DagGenerateInputs, GenerationContext};
use crate::error::{GenerateError, PromptError};
use crate::generate::DagGenerateRequestOptions;
use crate::parse::parse_dag_from_llm_content;
use crate::render::render;
use crate::template::PromptTemplate;

const GATE_FEEDBACK_INTRO: &str =
    "上一轮 DAG 未通过确定性校验闸门。请根据下列问题修正后仅输出 JSON（不要解释）。";

/// LLM round limit for gate feedback (includes the initial generation attempt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagGenerateWithGateOptions {
    pub llm: DagGenerateRequestOptions,
    pub max_rounds: u32,
}

impl Default for DagGenerateWithGateOptions {
    fn default() -> Self {
        Self {
            llm: DagGenerateRequestOptions::default(),
            max_rounds: 3,
        }
    }
}

/// Outcome when gate validation is part of generation.
#[derive(Debug, Clone)]
pub enum GenerateDagWithGateResult {
    Accepted(Dag),
    NeedsManualReview { dag: Dag, report: Report },
}

/// Canonical JSON of gate errors for appending to the user prompt (tests and cassette authoring).
pub fn format_gate_feedback_json(report: &Report) -> Result<String, GenerateError> {
    let errors: Vec<Issue> = report.errors().cloned().collect();
    let json = to_canonical_json(&errors).map_err(PromptError::Canonical)?;
    Ok(format!(
        "{GATE_FEEDBACK_INTRO}\n\n【校验问题 JSON】\n{json}"
    ))
}

/// Assemble prompt → LLM → parse → [`Gate::standard()`]; on failure append canonical issues JSON
/// to the user message and retry, up to `options.max_rounds` attempts.
pub async fn generate_dag_with_gate<P: LlmProvider + ?Sized>(
    provider: &P,
    template: &PromptTemplate,
    inputs: &DagGenerateInputs,
    options: &DagGenerateWithGateOptions,
) -> Result<GenerateDagWithGateResult, GenerateError> {
    let max_rounds = options.max_rounds.max(1);
    let ctx = GenerationContext::assemble(inputs)?;
    let base_prompt = redact_text(&render(template, &ctx.dag));
    let gate = Gate::standard();

    let mut correction_block = String::new();

    for round in 0..max_rounds {
        let user_content = if correction_block.is_empty() {
            base_prompt.clone()
        } else {
            format!("{base_prompt}\n\n{correction_block}")
        };
        let request = redact_chat_request(ChatRequest {
            model: options.llm.model.clone(),
            messages: vec![ChatMessage {
                role: options.llm.role,
                content: user_content,
            }],
        });
        let response = provider.chat(request).await?;
        let dag = parse_dag_from_llm_content(&response.content)?;
        let report = gate.validate(&dag);

        if report.passed() {
            return Ok(GenerateDagWithGateResult::Accepted(dag));
        }
        if round + 1 >= max_rounds {
            return Ok(GenerateDagWithGateResult::NeedsManualReview { dag, report });
        }
        correction_block = format_gate_feedback_json(&report)?;
    }

    Err(GenerateError::Prompt(PromptError::InvalidManifest(
        "gate loop finished without result".into(),
    )))
}
