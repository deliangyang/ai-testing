#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gate::Gate;
use generator::{
    repair_dag_with_gate, repair_prompt_preview, DagGenerateRequestOptions,
    DagGenerateWithGateOptions, GenerateDagWithGateResult,
};
use llm_gateway::{CassetteProvider, MessageRole};
use model::Dag;
use serde_json::json;

const BROKEN_PAY_DAG: &str = r#"{
  "id": "serial",
  "nodes": [
    {"id": "create", "type": "http", "method": "POST", "path": "/orders"},
    {"id": "pay", "type": "http", "method": "POST", "path": "/broken/${missing.output.body}"},
    {"id": "check", "type": "assert", "target": "pay", "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]}
  ],
  "edges": [{"from": "create", "to": "pay"}, {"from": "pay", "to": "check"}]
}"#;

const MODEL: &str = "cassette-dag-repair-pay";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

#[test]
fn repair_prompt_includes_gate_errors() {
    let dag: Dag = serde_json::from_str(BROKEN_PAY_DAG).unwrap();
    let report = Gate::standard().validate(&dag);
    let text = repair_prompt_preview(&dag, &report).unwrap();
    assert!(text.contains("template.unknown_node") || text.contains("reference"));
    assert!(text.contains("missing"));
}

#[tokio::test]
async fn cassette_repair_fixes_broken_pay_reference() {
    let dag: Dag = serde_json::from_str(BROKEN_PAY_DAG).unwrap();
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let result = repair_dag_with_gate(
        &provider,
        &dag,
        &DagGenerateWithGateOptions {
            llm: DagGenerateRequestOptions {
                model: MODEL.into(),
                role: MessageRole::User,
            },
            max_rounds: 2,
        },
    )
    .await
    .unwrap();
    match result {
        GenerateDagWithGateResult::Accepted { dag: fixed, .. } => {
            let Some(pay_node) = fixed.nodes.iter().find(|n| n.id == "pay") else {
                panic!("pay node");
            };
            let pay = serde_json::to_value(pay_node).unwrap();
            let path = pay.get("path").and_then(|v| v.as_str()).unwrap_or("");
            assert!(path.contains("create.output"));
            assert!(Gate::standard().validate(&fixed).passed());
        }
        GenerateDagWithGateResult::NeedsManualReview { .. } => {
            panic!("expected cassette repair to pass gate");
        }
    }
}

#[tokio::test]
#[ignore = "authoring helper: record cassette-dag-repair-pay"]
async fn write_repair_pay_cassette() {
    use llm_gateway::{redact_chat_request, ChatMessage, ChatRequest, ChatResponse, TokenUsage};

    let dag: Dag = serde_json::from_str(BROKEN_PAY_DAG).unwrap();
    let report = Gate::standard().validate(&dag);
    let content = repair_prompt_preview(&dag, &report).unwrap();
    let request = redact_chat_request(ChatRequest {
        model: MODEL.into(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content,
        }],
    });
    let fixed = json!({
        "id": "serial",
        "nodes": [
            {"id": "create", "type": "http", "method": "POST", "path": "/orders"},
            {"id": "pay", "type": "http", "method": "POST", "path": "/orders/${create.output.body.id}/pay"},
            {"id": "check", "type": "assert", "target": "pay", "assertions": [{"path": "$.status", "op": "eq", "expected": 200}]}
        ],
        "edges": [{"from": "create", "to": "pay"}, {"from": "pay", "to": "check"}]
    });
    let response = ChatResponse {
        content: serde_json::to_string(&fixed).unwrap(),
        model: MODEL.into(),
        finish_reason: Some("stop".into()),
        usage: TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
        },
    };
    let path = CassetteProvider::write_cassette(fixtures_dir(), &request, &response).unwrap();
    eprintln!("wrote {}", path.display());
}
