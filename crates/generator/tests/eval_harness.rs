#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use generator::{
    default_eval_chains, default_prompts_dir, run_default_cassette_eval, EvalSummary,
    PromptCatalog, TARGET_ACCEPTANCE_RATE,
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

#[tokio::test]
async fn cassette_eval_meets_gate_acceptance_target() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let summary = run_default_cassette_eval(fixtures_dir().as_path(), &tmpl, 1)
        .await
        .unwrap();
    assert!(
        summary.meets_target(),
        "first={:.1}% final={:.1}%",
        summary.first_pass_rate() * 100.0,
        summary.final_accept_rate() * 100.0
    );
    assert_eq!(summary.trials.len(), default_eval_chains().len());
}

#[tokio::test]
async fn generation_eval_write_report_when_env_set() {
    let out = match std::env::var("GENERATION_EVAL_OUT") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => return,
    };
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let iterations = std::env::var("GENERATION_EVAL_ITERATIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let summary = run_default_cassette_eval(fixtures_dir().as_path(), &tmpl, iterations)
        .await
        .unwrap();
    write_report(&out, &summary);
}

fn write_report(path: &std::path::Path, summary: &EvalSummary) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut body = summary.to_markdown();
    body.push_str(&format!(
        "\n_Regenerate: `scripts/generation-eval.sh` (cassette). Live LLM: set `AITEST_LIVE_LLM=1` and provider env (not in default CI). Target reference: {:.0}%._\n",
        TARGET_ACCEPTANCE_RATE * 100.0
    ));
    fs::write(path, body).unwrap();
}
