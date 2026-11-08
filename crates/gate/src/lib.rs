//! Deterministic validation gate for DAG definitions (§4.3).
//!
//! Rules are pure functions of the DAG document: no network, no clock, no model calls. The same
//! input always yields the same, identically ordered report.

pub mod codes;
pub mod issue;
mod output;
pub mod rules;
pub mod schema_path;

pub use issue::{Issue, Report, Severity};

use model::Dag;

/// Everything a rule may look at.
pub struct Context<'a> {
    pub dag: &'a Dag,
}

pub trait Rule: Send + Sync {
    /// Stable identifier used in logs and to enable/disable rules.
    fn name(&self) -> &'static str;
    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>);
}

#[derive(Default)]
pub struct Gate {
    rules: Vec<Box<dyn Rule>>,
}

impl Gate {
    pub fn new() -> Self {
        Self::default()
    }

    /// All built-in rules; this is the gate every save and activation goes through.
    pub fn standard() -> Self {
        Self::new()
            .with_rule(rules::StructureRule)
            .with_rule(rules::AcyclicRule)
            .with_rule(rules::ReferenceRule)
            .with_rule(rules::FieldRule)
            .with_rule(rules::DecidabilityRule)
            .with_rule(rules::CompletenessRule)
    }

    #[must_use]
    pub fn with_rule(mut self, rule: impl Rule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    pub fn rule_names(&self) -> Vec<&'static str> {
        self.rules.iter().map(|r| r.name()).collect()
    }

    pub fn validate(&self, dag: &Dag) -> Report {
        let ctx = Context { dag };
        let mut issues = Vec::new();
        for rule in &self.rules {
            rule.check(&ctx, &mut issues);
        }
        Report::new(issues)
    }
}
