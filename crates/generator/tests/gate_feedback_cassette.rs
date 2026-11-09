#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gate::codes;
use gate::Gate;
use generator::{
    default_prompts_dir, format_gate_feedback_json, generate_dag_with_gate, ApiSchemaInput,
    CorrectionPreferences, DagGenerateInputs, DagGenerateRequestOptions,
    DagGenerateWithGateOptions, GenerateDagWithGateResult, GenerationContext, PromptCatalog,
    SkillContracts,
};
use llm_gateway::{
    redact_chat_request, redact_text, CassetteProvider, ChatMessage, ChatRequest, MessageRole,
    TokenUsage,
};

const MINIMAL_DAG_JSON: &str = r#"{"id":"serial","name":"","nodes":[{"id":"create","type":"http","method":"POST","path":"/orders"},{"id":"check","type":"assert","target":"create","assertions":[{"path":"$.status","op":"eq","expected":200}]}],"edges":[{"from":"create","to":"check"}]}"#;

/// Successor variable reference (gate fixture `11_reference_successor.yaml`).
const BAD_REF_DAG_JSON: &str = r#"{"id":"bad-ref","name":"","nodes":[{"id":"a","type":"http","method":"GET","path":"/a","headers":{"x-code":"${b.output.status}"}},{"id":"b","type":"http","method":"GET","path":"/b"}],"edges":[{"from":"a","to":"b"}]}"#;

const MODEL_CORRECTS: &str = "cassette-gate-feedback-corrects";
const MODEL_STUCK: &str = "cassette-gate-feedback-stuck";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

fn fixture_inputs() -> DagGenerateInputs {
    DagGenerateInputs {
        natural_language_description: "创建订单并断言状态码".into(),
        business_rules: "至少一条 http 与 assert".into(),
        api_schema: ApiSchemaInput::SchemaJson(r#"{"paths":{"/orders":{"post":{}}}}"#.into()),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    }
}

#[tokio::test]
async fn cassette_gate_loop_corrects_invalid_variable_reference() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let result = generate_dag_with_gate(
        &provider,
        &tmpl,
        &fixture_inputs(),
        &DagGenerateWithGateOptions {
            llm: DagGenerateRequestOptions {
                model: MODEL_CORRECTS.into(),
                role: MessageRole::User,
            },
            max_rounds: 2,
        },
    )
    .await
    .unwrap();
    match result {
        GenerateDagWithGateResult::Accepted { dag, attempts } => {
            assert_eq!(attempts, 2);
            assert_eq!(dag.id, "serial");
            let report = Gate::standard().validate(&dag);
            assert!(report.passed());
        }
        GenerateDagWithGateResult::NeedsManualReview { .. } => {
            panic!("expected gate to accept corrected DAG");
        }
    }
}

#[tokio::test]
async fn cassette_gate_loop_needs_manual_review_when_still_invalid() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let result = generate_dag_with_gate(
        &provider,
        &tmpl,
        &fixture_inputs(),
        &DagGenerateWithGateOptions {
            llm: DagGenerateRequestOptions {
                model: MODEL_STUCK.into(),
                role: MessageRole::User,
            },
            max_rounds: 3,
        },
    )
    .await
    .unwrap();
    let GenerateDagWithGateResult::NeedsManualReview { dag, report, .. } = result else {
        panic!("expected manual review after max rounds");
    };
    assert_eq!(dag.id, "bad-ref");
    assert!(!report.passed());
    assert!(report.has_code(codes::NOT_ANCESTOR));
    let Some(err) = report.errors().find(|i| i.code == codes::NOT_ANCESTOR) else {
        panic!("not_ancestor issue");
    };
    assert_eq!(err.node_id.as_deref(), Some("a"));
    assert!(err
        .json_path
        .as_deref()
        .is_some_and(|p| p.contains("headers")));
}

#[test]
fn gate_feedback_json_lists_errors_only() {
    let dag = generator::parse_dag_from_llm_content(BAD_REF_DAG_JSON).unwrap();
    let report = Gate::standard().validate(&dag);
    let block = format_gate_feedback_json(&report).unwrap();
    assert!(block.contains("【校验问题 JSON】"));
    assert!(block.contains(codes::NOT_ANCESTOR));
    assert!(!block.contains(codes::UNASSERTED_HTTP));
}

#[test]
#[ignore = "authoring helper: run once to refresh gate-feedback cassettes"]
fn write_gate_feedback_cassettes() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let inputs = fixture_inputs();
    let ctx = GenerationContext::assemble(&inputs).unwrap();
    let base_prompt = redact_text(&generator::render(&tmpl, &ctx.dag));
    let bad_dag = generator::parse_dag_from_llm_content(BAD_REF_DAG_JSON).unwrap();
    let correction = format_gate_feedback_json(&Gate::standard().validate(&bad_dag)).unwrap();
    let corrected_prompt = format!("{base_prompt}\n\n{correction}");

    let write = |model: &str, content: &str, body: &str| {
        let request = redact_chat_request(ChatRequest {
            model: model.into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: content.to_string(),
            }],
        });
        let response = llm_gateway::ChatResponse {
            content: body.to_string(),
            model: model.into(),
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            },
            finish_reason: Some("stop".into()),
        };
        let path = CassetteProvider::write_cassette(fixtures_dir(), &request, &response).unwrap();
        eprintln!("wrote {}", path.display());
    };

    write(MODEL_CORRECTS, &base_prompt, BAD_REF_DAG_JSON);
    write(MODEL_CORRECTS, &corrected_prompt, MINIMAL_DAG_JSON);

    write(MODEL_STUCK, &base_prompt, BAD_REF_DAG_JSON);
    write(MODEL_STUCK, &corrected_prompt, BAD_REF_DAG_JSON);
}
