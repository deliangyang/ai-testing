//! Repair an existing DAG using gate feedback and LLM (develop.md §6.2).

use gate::{Gate, Report};
use llm_gateway::{redact_chat_request, ChatMessage, ChatRequest, LlmProvider};
use model::{to_canonical_json, Dag};

use crate::error::{GenerateError, PromptError};
use crate::gate_loop::DagGenerateWithGateOptions;
use crate::gate_loop::{format_gate_feedback_json, GenerateDagWithGateResult};
use crate::parse::parse_dag_from_llm_content;

const DAG_REPAIR_INTRO: &str =
    "下列是当前 DAG JSON。请根据校验问题修正后仅输出完整 DAG JSON（不要解释）。";

/// Send the current DAG plus gate errors to the model; retry until pass or `max_rounds`.
pub async fn repair_dag_with_gate<P: LlmProvider + ?Sized>(
    provider: &P,
    dag: &Dag,
    options: &DagGenerateWithGateOptions,
) -> Result<GenerateDagWithGateResult, GenerateError> {
    let max_rounds = options.max_rounds.max(1);
    let gate = Gate::standard();
    let dag_json = to_canonical_json(dag).map_err(PromptError::Canonical)?;
    let base_prompt = format!("{DAG_REPAIR_INTRO}\n\n【当前 DAG】\n{dag_json}");

    let initial = gate.validate(dag);
    if initial.passed() {
        return Ok(GenerateDagWithGateResult::Accepted {
            dag: dag.clone(),
            attempts: 0,
        });
    }

    let mut correction_block = format_gate_feedback_json(&initial)?;

    for round in 0..max_rounds {
        let user_content = format!("{base_prompt}\n\n{correction_block}");
        let request = redact_chat_request(ChatRequest {
            model: options.llm.model.clone(),
            messages: vec![ChatMessage {
                role: options.llm.role,
                content: user_content,
            }],
        });
        let response = provider.chat(request).await?;
        let candidate = parse_dag_from_llm_content(&response.content)?;
        let report = gate.validate(&candidate);

        let attempts = round + 1;
        if report.passed() {
            return Ok(GenerateDagWithGateResult::Accepted {
                dag: candidate,
                attempts,
            });
        }
        if attempts >= max_rounds {
            return Ok(GenerateDagWithGateResult::NeedsManualReview {
                dag: candidate,
                report,
                attempts,
            });
        }
        correction_block = format_gate_feedback_json(&report)?;
    }

    Err(GenerateError::Prompt(PromptError::InvalidManifest(
        "repair loop finished without result".into(),
    )))
}

/// Exposed for tests that build cassette prompts without calling the provider.
pub fn repair_prompt_preview(dag: &Dag, report: &Report) -> Result<String, GenerateError> {
    let dag_json = to_canonical_json(dag).map_err(PromptError::Canonical)?;
    let feedback = format_gate_feedback_json(report)?;
    Ok(format!(
        "{DAG_REPAIR_INTRO}\n\n【当前 DAG】\n{dag_json}\n\n{feedback}"
    ))
}
