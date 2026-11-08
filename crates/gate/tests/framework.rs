#![allow(clippy::unwrap_used)]

use gate::{Context, Gate, Issue, Report, Rule, Severity};
use model::Dag;

fn dag(json: &str) -> Dag {
    Dag::from_json_str(json).unwrap()
}

const EMPTY: &str = r#"{"id":"empty","nodes":[]}"#;
const TWO_NODES: &str = r#"{"id":"d","nodes":[
    {"id":"b","type":"cleanup"},
    {"id":"a","type":"http","method":"GET","path":"/"}
],"edges":[{"from":"a","to":"b"}]}"#;

/// Emits one issue per node in reverse document order, plus a DAG-level warning.
struct NoisyRule;

impl Rule for NoisyRule {
    fn name(&self) -> &'static str {
        "noisy"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        for (i, node) in ctx.dag.nodes.iter().enumerate().rev() {
            out.push(
                Issue::error("z.noisy", format!("node {}", node.id))
                    .at_node(&node.id)
                    .at_path(format!("/nodes/{i}")),
            );
            out.push(Issue::warn("a.noisy", "warn").at_node(&node.id));
        }
        out.push(Issue::warn("dag.level", "dag-wide"));
        out.push(Issue::warn("dag.level", "dag-wide"));
    }
}

struct SilentRule;

impl Rule for SilentRule {
    fn name(&self) -> &'static str {
        "silent"
    }

    fn check(&self, _: &Context<'_>, _: &mut Vec<Issue>) {}
}

#[test]
fn gate_without_rules_passes_everything() {
    let report = Gate::new().validate(&dag(TWO_NODES));
    assert!(report.passed());
    assert!(report.issues.is_empty());
}

#[test]
fn empty_and_valid_dags_pass_silent_rules() {
    let gate = Gate::new().with_rule(SilentRule);
    assert_eq!(gate.rule_names(), vec!["silent"]);
    assert_eq!(gate.validate(&dag(EMPTY)), Report::default());
    assert!(gate.validate(&dag(TWO_NODES)).passed());
}

#[test]
fn issues_are_sorted_by_node_then_code_and_deduplicated() {
    let report = Gate::new().with_rule(NoisyRule).validate(&dag(TWO_NODES));
    let order: Vec<_> = report
        .issues
        .iter()
        .map(|i| (i.node_id.as_deref().unwrap_or("-"), i.code.as_str()))
        .collect();
    assert_eq!(
        order,
        vec![
            ("-", "dag.level"),
            ("a", "a.noisy"),
            ("a", "z.noisy"),
            ("b", "a.noisy"),
            ("b", "z.noisy"),
        ]
    );
    assert!(!report.passed());
    assert_eq!(report.errors().count(), 2);
    assert_eq!(report.warnings().count(), 3);
    assert!(report.has_code("dag.level"));
}

#[test]
fn report_is_identical_across_runs_and_rule_order() {
    let d = dag(TWO_NODES);
    let first = Gate::new()
        .with_rule(NoisyRule)
        .with_rule(SilentRule)
        .validate(&d);
    let second = Gate::new()
        .with_rule(SilentRule)
        .with_rule(NoisyRule)
        .validate(&d);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn issue_renders_and_serializes_compactly() {
    let issue = Issue::error("structure.dangling_edge", "edge points to unknown node 'x'")
        .at_node("a")
        .at_path("/edges/0/to");
    assert_eq!(
        issue.to_string(),
        "error[structure.dangling_edge] node 'a' at /edges/0/to: edge points to unknown node 'x'"
    );
    let json = serde_json::to_value(Issue::warn("w", "m")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"code": "w", "severity": "warn", "message": "m"})
    );
    assert_eq!(Severity::Error, issue.severity);
}
