#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use generator::{
    default_prompts_dir, generate_dag, parse_dag_from_llm_content, ApiSchemaInput,
    CorrectionPreferences, DagGenerateInputs, DagGenerateRequestOptions, GenerationContext,
    PromptCatalog, SkillContracts,
};
use llm_gateway::{redact_chat_request, CassetteProvider, ChatResponse, MessageRole, TokenUsage};
use model::Dag;

const MODEL: &str = "cassette-dag-serial-parity";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

fn serial_inputs() -> DagGenerateInputs {
    DagGenerateInputs {
        natural_language_description: "创建订单、支付、断言支付 HTTP 状态为 200".into(),
        business_rules: "串行 create → pay → assert，pay 路径引用 create 输出的订单 id".into(),
        api_schema: ApiSchemaInput::SchemaJson(r#"{"paths":{"/orders":{"post":{}}}}"#.into()),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    }
}

fn manual_serial_dag() -> Dag {
    let yaml = fs::read_to_string(repo_root().join("examples/serial.yaml")).unwrap();
    Dag::from_yaml_str(&yaml).unwrap()
}

#[tokio::test]
async fn cassette_generates_serial_equivalent_to_example() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let dag = generate_dag(
        &provider,
        &tmpl,
        &serial_inputs(),
        &DagGenerateRequestOptions {
            model: MODEL.into(),
            role: MessageRole::User,
        },
    )
    .await
    .unwrap();
    let manual = manual_serial_dag();
    assert_eq!(dag.id, manual.id);
    assert_eq!(dag.nodes.len(), manual.nodes.len());
    let ai_json = model::to_canonical_json(&dag).unwrap();
    let manual_json = model::to_canonical_json(&manual).unwrap();
    assert_eq!(ai_json, manual_json);
}

#[tokio::test]
#[ignore = "authoring helper: record cassette-dag-serial-parity"]
async fn write_serial_parity_cassette() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let ctx = GenerationContext::assemble(&serial_inputs()).unwrap();
    let request = redact_chat_request(ctx.redacted_chat_request(&tmpl, MODEL, MessageRole::User));
    let manual = manual_serial_dag();
    let response = ChatResponse {
        content: model::to_canonical_json(&manual).unwrap(),
        model: MODEL.into(),
        finish_reason: Some("stop".into()),
        usage: TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
        },
    };
    let path = CassetteProvider::write_cassette(fixtures_dir(), &request, &response).unwrap();
    let _ = parse_dag_from_llm_content(&response.content).unwrap();
    eprintln!("wrote {}", path.display());
}
