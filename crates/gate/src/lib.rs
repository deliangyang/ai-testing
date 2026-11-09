//! Deterministic validation gate for DAG definitions (§4.3).
//!
//! Rules are pure functions of the DAG document: no network, no clock, no model calls. The same
//! input always yields the same, identically ordered report.

pub mod activation;
pub mod calibrate;
pub mod codes;
pub mod issue;
mod output;
pub mod rules;
pub mod schema_path;

pub use activation::{prepare_for_activation, ActivationPrepareResult};
pub use calibrate::{calibrate, CalibrateResult, RepairRecord};
pub use issue::{Issue, Report, Severity};

use model::Dag;
use rules::{ApiSpecLookup, ApiSpecLookupHandle};

/// Everything a rule may look at.
pub struct Context<'a> {
    pub dag: &'a Dag,
    api_specs: Option<&'a dyn ApiSpecLookup>,
}

impl<'a> Context<'a> {
    pub(crate) fn api_specs(&self) -> Option<&'a dyn ApiSpecLookup> {
        self.api_specs
    }
}

pub trait Rule: Send + Sync {
    /// Stable identifier used in logs and to enable/disable rules.
    fn name(&self) -> &'static str;
    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>);
}

#[derive(Default)]
pub struct Gate {
    rules: Vec<Box<dyn Rule>>,
    api_specs: Option<ApiSpecLookupHandle>,
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

    /// The standard rules plus the cross-DAG checks that need stored DAGs; used when saving.
    pub fn for_save(
        catalog: std::sync::Arc<dyn rules::DagCatalog>,
        api_specs: Option<ApiSpecLookupHandle>,
    ) -> Self {
        Self::standard()
            .with_api_specs(api_specs)
            .with_rule(rules::SubworkflowCycleRule::new(catalog))
    }

    #[must_use]
    pub fn with_api_specs(mut self, api_specs: Option<ApiSpecLookupHandle>) -> Self {
        self.api_specs = api_specs;
        self
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
        let ctx = Context {
            dag,
            api_specs: self.api_specs.as_deref(),
        };
        let mut issues = Vec::new();
        for rule in &self.rules {
            rule.check(&ctx, &mut issues);
        }
        Report::new(issues)
    }
}
