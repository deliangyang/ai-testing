#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use generator::{
    default_prompts_dir, generate_dag, parse_dag_from_llm_content, ApiSchemaInput,
    CorrectionPreferences, DagGenerateInputs, DagGenerateRequestOptions, GenerateError,
    PromptCatalog, SkillContracts,
};
use llm_gateway::{
    CassetteProvider, ChatMessage, ChatRequest, ChatResponse, MessageRole, TokenUsage,
};

const MINIMAL_DAG_JSON: &str = r#"{"id":"serial","name":"","nodes":[{"id":"create","type":"http","method":"POST","path":"/orders"},{"id":"check","type":"assert","target":"create","assertions":[{"path":"$.status","op":"eq","expected":200}]}],"edges":[{"from":"create","to":"check"}]}"#;

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

fn options(model: &str) -> DagGenerateRequestOptions {
    DagGenerateRequestOptions {
        model: model.into(),
        role: MessageRole::User,
    }
}

#[tokio::test]
async fn cassette_replay_parses_plain_json_dag() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let dag = generate_dag(
        &provider,
        &tmpl,
        &fixture_inputs(),
        &options("cassette-dag-plain-json"),
    )
    .await
    .unwrap();
    assert_eq!(dag.id, "serial");
    assert_eq!(dag.nodes.len(), 2);
}

#[tokio::test]
async fn cassette_replay_parses_json_with_explanation() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let dag = generate_dag(
        &provider,
        &tmpl,
        &fixture_inputs(),
        &options("cassette-dag-with-preamble"),
    )
    .await
    .unwrap();
    assert_eq!(dag.id, "serial");
    assert_eq!(dag.edges.len(), 1);
}

#[tokio::test]
async fn cassette_replay_invalid_json_is_rejected() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let err = generate_dag(
        &provider,
        &tmpl,
        &fixture_inputs(),
        &options("cassette-dag-invalid-json"),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        GenerateError::InvalidDag(_) | GenerateError::NoJsonFound
    ));
}

#[test]
fn parse_fixture_responses_without_llm() {
    let explained = format!("说明：满足轮询与断言要求。\n\n```json\n{MINIMAL_DAG_JSON}\n```\n");
    let dag = parse_dag_from_llm_content(&explained).unwrap();
    assert_eq!(dag.id, "serial");

    let code = "```typescript\nexport const dag = () => fetch('/x');\n```";
    let err = parse_dag_from_llm_content(code).unwrap_err();
    assert!(matches!(err, GenerateError::RejectedCodeOutput));
}

#[test]
#[ignore = "authoring helper: run once to refresh cassettes under tests/fixtures/cassettes"]
fn write_dag_generate_cassettes() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let inputs = fixture_inputs();
    let ctx = generator::GenerationContext::assemble(&inputs).unwrap();
    let content = generator::render(&tmpl, &ctx.dag);

    let scenarios: &[(&str, &str)] = &[
        ("cassette-dag-plain-json", MINIMAL_DAG_JSON),
        (
            "cassette-dag-with-preamble",
            &format!("已根据接口 schema 生成 DAG，请审阅。\n\n```json\n{MINIMAL_DAG_JSON}\n```\n"),
        ),
        ("cassette-dag-invalid-json", r#"{"id":"broken","nodes":[}"#),
    ];

    for (model, body) in scenarios {
        let request = ChatRequest {
            model: (*model).into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: content.clone(),
            }],
        };
        let response = ChatResponse {
            content: body.to_string(),
            model: (*model).into(),
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            },
            finish_reason: Some("stop".into()),
        };
        let path = CassetteProvider::write_cassette(fixtures_dir(), &request, &response).unwrap();
        eprintln!("wrote {}", path.display());
    }
}
