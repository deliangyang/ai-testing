#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use generator::{default_eval_chains, default_prompts_dir, GenerationContext, PromptCatalog};
use llm_gateway::{redact_chat_request, CassetteProvider, ChatResponse, MessageRole, TokenUsage};
use model::{to_canonical_json, Dag};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

fn example_dag(id: &str) -> Dag {
    let yaml = fs::read_to_string(repo_root().join(format!("examples/{id}.yaml"))).unwrap();
    Dag::from_yaml_str(&yaml).unwrap()
}

#[tokio::test]
#[ignore = "authoring helper: record cassette-eval-async-poll and cassette-eval-parallel"]
async fn write_eval_chain_cassettes() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    for chain in default_eval_chains() {
        if chain.id == "serial" {
            continue;
        }
        let ctx = GenerationContext::assemble(&chain.inputs).unwrap();
        let request = redact_chat_request(ctx.redacted_chat_request(
            &tmpl,
            chain.cassette_model,
            MessageRole::User,
        ));
        let dag = example_dag(chain.id);
        let response = ChatResponse {
            content: to_canonical_json(&dag).unwrap(),
            model: chain.cassette_model.into(),
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
}
