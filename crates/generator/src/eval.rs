//! Generation evaluation harness (P2-25): gate first-pass and final acceptance rates.

use crate::error::GenerateError;
use crate::gate_loop::{
    generate_dag_with_gate, DagGenerateWithGateOptions, GenerateDagWithGateResult,
};
use crate::template::PromptTemplate;
use crate::DagGenerateInputs;
use llm_gateway::{CassetteProvider, LlmProvider};

/// One of the three reference flows (`examples/*.yaml`).
#[derive(Debug, Clone)]
pub struct EvalChain {
    pub id: &'static str,
    pub cassette_model: &'static str,
    pub inputs: DagGenerateInputs,
}

/// Target from develop.md §7.3 / M0 Q3 (deferred to phase 2).
pub const TARGET_ACCEPTANCE_RATE: f64 = 0.70;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalTrial {
    pub chain_id: &'static str,
    pub iteration: u32,
    pub accepted_first_attempt: bool,
    pub accepted_final: bool,
    pub llm_attempts: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalSummary {
    pub trials: Vec<EvalTrial>,
    pub iterations_per_chain: u32,
    pub mode: &'static str,
}

impl EvalSummary {
    pub fn total_trials(&self) -> usize {
        self.trials.len()
    }

    pub fn first_pass_rate(&self) -> f64 {
        rate(
            self.trials
                .iter()
                .filter(|t| t.accepted_first_attempt)
                .count(),
        )
    }

    pub fn final_accept_rate(&self) -> f64 {
        rate(self.trials.iter().filter(|t| t.accepted_final).count())
    }

    pub fn meets_target(&self) -> bool {
        self.final_accept_rate() >= TARGET_ACCEPTANCE_RATE
            && self.first_pass_rate() >= TARGET_ACCEPTANCE_RATE
    }

    pub fn to_markdown(&self) -> String {
        let first = self
            .trials
            .iter()
            .filter(|t| t.accepted_first_attempt)
            .count();
        let final_ok = self.trials.iter().filter(|t| t.accepted_final).count();
        let total = self.total_trials();
        let verdict = if self.meets_target() {
            "PASS"
        } else {
            "BELOW TARGET"
        };
        let mut rows = String::new();
        for t in &self.trials {
            rows.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                t.chain_id,
                t.iteration,
                yes_no(t.accepted_first_attempt),
                yes_no(t.accepted_final),
                t.llm_attempts
            ));
        }
        format!(
            "# DAG generation gate evaluation (P2-25)\n\n\
             Mode: **{}** · iterations per chain: **{}** · target: **≥{:.0}%** first and final\n\n\
             | Metric | Pass | Total | Rate |\n|---|---:|---:|---:|\n\
             | First gate accept | {first} | {total} | {:.1}% |\n\
             | Final accept (after feedback) | {final_ok} | {total} | {:.1}% |\n\n\
             | Chain | Run | First pass | Final | LLM attempts |\n|---|---:|:---:|:---:|---:|\n\
             {rows}\n**Verdict**: {verdict} (both rates ≥ {:.0}%)\n",
            self.mode,
            self.iterations_per_chain,
            TARGET_ACCEPTANCE_RATE * 100.0,
            pct(first, total),
            pct(final_ok, total),
            TARGET_ACCEPTANCE_RATE * 100.0,
        )
    }
}

fn yes_no(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

fn rate(passed: usize) -> f64 {
    rate_over(passed, 1)
}

fn rate_over(passed: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    passed as f64 / total as f64
}

fn pct(passed: usize, total: usize) -> f64 {
    rate_over(passed, total) * 100.0
}

/// Built-in three chains aligned with `examples/serial.yaml`, `async-poll.yaml`, `parallel.yaml`.
pub fn default_eval_chains() -> Vec<EvalChain> {
    vec![
        EvalChain {
            id: "serial",
            cassette_model: "cassette-dag-serial-parity",
            inputs: DagGenerateInputs {
                natural_language_description:
                    "创建订单、支付、断言支付 HTTP 状态为 200".into(),
                business_rules:
                    "串行 create → pay → assert，pay 路径引用 create 输出的订单 id".into(),
                api_schema: crate::ApiSchemaInput::SchemaJson(
                    r#"{"paths":{"/orders":{"post":{}}}}"#.into(),
                ),
                skill_contracts: crate::SkillContracts::default(),
                preferences: crate::CorrectionPreferences::default(),
            },
        },
        EvalChain {
            id: "async-poll",
            cassette_model: "cassette-eval-async-poll",
            inputs: DagGenerateInputs {
                natural_language_description:
                    "创建订单、支付后轮询订单状态直到 PAID，并断言 pay 返回 200".into(),
                business_rules: "含 poll 节点：GET 订单、condition status=PAID、backoff 与 timeout"
                    .into(),
                api_schema: crate::ApiSchemaInput::SchemaJson(
                    r#"{"paths":{"/orders":{"post":{},"get":{}}}}"#.into(),
                ),
                skill_contracts: crate::SkillContracts::default(),
                preferences: crate::CorrectionPreferences::default(),
            },
        },
        EvalChain {
            id: "parallel",
            cassette_model: "cassette-eval-parallel",
            inputs: DagGenerateInputs {
                natural_language_description:
                    "创建订单后并行查询库存与价格，在 assert 中合并校验".into(),
                business_rules: "create 后 stock 与 price 并行，check 汇聚两路 HTTP 状态".into(),
                api_schema: crate::ApiSchemaInput::SchemaJson(
                    r#"{"paths":{"/orders":{"post":{}},"/stock/A-1":{"get":{}},"/price/A-1":{"get":{}}}}"#.into(),
                ),
                skill_contracts: crate::SkillContracts::default(),
                preferences: crate::CorrectionPreferences::default(),
            },
        },
    ]
}

/// Run cassette-backed evaluation (`iterations_per_chain` repeats per chain; deterministic).
pub async fn run_cassette_eval<P: LlmProvider + ?Sized>(
    provider: &P,
    template: &PromptTemplate,
    chains: &[EvalChain],
    iterations_per_chain: u32,
    options: &DagGenerateWithGateOptions,
) -> Result<EvalSummary, GenerateError> {
    let iterations = iterations_per_chain.max(1);
    let mut trials = Vec::new();
    for chain in chains {
        let mut chain_options = options.clone();
        chain_options.llm.model = chain.cassette_model.to_string();
        for iteration in 1..=iterations {
            let result =
                generate_dag_with_gate(provider, template, &chain.inputs, &chain_options).await?;
            let (accepted_final, attempts) = match &result {
                GenerateDagWithGateResult::Accepted { attempts, .. } => (true, *attempts),
                GenerateDagWithGateResult::NeedsManualReview { attempts, .. } => (false, *attempts),
            };
            let accepted_first = matches!(
                &result,
                GenerateDagWithGateResult::Accepted { attempts: 1, .. }
            );
            trials.push(EvalTrial {
                chain_id: chain.id,
                iteration,
                accepted_first_attempt: accepted_first,
                accepted_final,
                llm_attempts: attempts,
            });
        }
    }
    Ok(EvalSummary {
        trials,
        iterations_per_chain: iterations,
        mode: "cassette",
    })
}

/// Load cassettes from `dir` and run [`default_eval_chains`].
pub async fn run_default_cassette_eval(
    cassette_dir: &std::path::Path,
    template: &PromptTemplate,
    iterations_per_chain: u32,
) -> Result<EvalSummary, GenerateError> {
    let provider = CassetteProvider::from_dir(cassette_dir).map_err(|e| {
        GenerateError::Prompt(crate::error::PromptError::InvalidManifest(e.to_string()))
    })?;
    let options = DagGenerateWithGateOptions::default();
    run_cassette_eval(
        &provider,
        template,
        &default_eval_chains(),
        iterations_per_chain,
        &options,
    )
    .await
}
