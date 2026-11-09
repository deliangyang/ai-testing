use std::collections::BTreeSet;

use model::scope::node_refs;
use model::{Dag, NodeKind};

use crate::codes;
use crate::{Context, Issue, Rule};

/// The DAG must be runnable as written: required fields are non-empty, every node is wired
/// into the graph, cleanup nodes are sinks, and each http call is checked by some assert.
pub struct CompletenessRule;

impl Rule for CompletenessRule {
    fn name(&self) -> &'static str {
        "completeness"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let dag = ctx.dag;
        check_required(dag, out);
        check_orphans(dag, out);
        check_cleanup_sinks(dag, out);
        check_asserted(dag, out);
    }
}

fn empty(path: String, what: &str) -> Issue {
    Issue::error(codes::EMPTY_FIELD, format!("{what} must not be empty")).at_path(path)
}

fn check_required(dag: &Dag, out: &mut Vec<Issue>) {
    if dag.id.trim().is_empty() {
        out.push(empty("/id".into(), "DAG id"));
    }
    if dag.nodes.is_empty() {
        out.push(empty("/nodes".into(), "nodes"));
    }
    for (i, node) in dag.nodes.iter().enumerate() {
        let base = format!("/nodes/{i}");
        let mut fields: Vec<(String, &str, bool)> = Vec::new();
        match &node.kind {
            NodeKind::Http(cfg) => {
                fields.push((format!("{base}/path"), "path", is_blank(&cfg.path)))
            }
            NodeKind::Poll(cfg) => fields.push((
                format!("{base}/request/path"),
                "request path",
                is_blank(&cfg.request.path),
            )),
            NodeKind::Assert(cfg) => {
                fields.push((format!("{base}/target"), "target", is_blank(&cfg.target)));
                fields.push((
                    format!("{base}/assertions"),
                    "assertions",
                    cfg.assertions.is_empty(),
                ));
            }
            NodeKind::DbQuery(cfg) => {
                fields.push((
                    format!("{base}/connection"),
                    "connection",
                    is_blank(&cfg.connection),
                ));
                fields.push((format!("{base}/sql"), "sql", is_blank(&cfg.sql)));
            }
            NodeKind::Subworkflow(cfg) => match (&cfg.dag, &cfg.skill) {
                (Some(dag), None) => {
                    fields.push((format!("{base}/dag/id"), "dag id", is_blank(&dag.id)));
                }
                (None, Some(skill)) => {
                    fields.push((format!("{base}/skill/id"), "skill id", is_blank(&skill.id)));
                }
                _ => fields.push((format!("{base}/target"), "dag or skill", true)),
            },
            NodeKind::Cleanup(cfg) => {
                fields.push((
                    format!("{base}/requests"),
                    "requests",
                    cfg.requests.is_empty(),
                ));
                for (k, req) in cfg.requests.iter().enumerate() {
                    fields.push((
                        format!("{base}/requests/{k}/path"),
                        "request path",
                        is_blank(&req.path),
                    ));
                }
            }
        }
        out.extend(
            fields
                .into_iter()
                .filter(|(_, _, missing)| *missing)
                .map(|(path, what, _)| empty(path, what).at_node(&node.id)),
        );
    }
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn check_orphans(dag: &Dag, out: &mut Vec<Issue>) {
    if dag.nodes.len() < 2 {
        return;
    }
    let ids: BTreeSet<&str> = dag.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut connected = BTreeSet::new();
    for e in &dag.edges {
        // Dangling edges and self loops are reported by StructureRule and connect nothing.
        if e.from != e.to && ids.contains(e.from.as_str()) && ids.contains(e.to.as_str()) {
            connected.insert(e.from.as_str());
            connected.insert(e.to.as_str());
        }
    }
    for (i, node) in dag.nodes.iter().enumerate() {
        if !connected.contains(node.id.as_str()) {
            out.push(
                Issue::error(
                    codes::ORPHAN_NODE,
                    format!("node '{}' has no edges to the rest of the DAG", node.id),
                )
                .at_node(&node.id)
                .at_path(format!("/nodes/{i}")),
            );
        }
    }
}

fn check_cleanup_sinks(dag: &Dag, out: &mut Vec<Issue>) {
    for (i, e) in dag.edges.iter().enumerate() {
        if let Some(node) = dag.node(&e.from) {
            if matches!(node.kind, NodeKind::Cleanup(_)) {
                out.push(
                    Issue::error(
                        codes::CLEANUP_HAS_SUCCESSOR,
                        format!(
                            "cleanup node '{}' runs last and cannot precede '{}'",
                            e.from, e.to
                        ),
                    )
                    .at_node(&e.from)
                    .at_path(format!("/edges/{i}")),
                );
            }
        }
    }
}

fn check_asserted(dag: &Dag, out: &mut Vec<Issue>) {
    let mut asserted = BTreeSet::new();
    for node in &dag.nodes {
        if let NodeKind::Assert(cfg) = &node.kind {
            asserted.insert(cfg.target.clone());
            if let Ok(refs) = node_refs(node) {
                asserted.extend(
                    refs.iter()
                        .filter_map(|r| r.var.node_id().map(str::to_owned)),
                );
            }
        }
    }
    for (i, node) in dag.nodes.iter().enumerate() {
        if matches!(node.kind, NodeKind::Http(_)) && !asserted.contains(&node.id) {
            out.push(
                Issue::warn(
                    codes::UNASSERTED_HTTP,
                    format!("no assert checks the response of http node '{}'", node.id),
                )
                .at_node(&node.id)
                .at_path(format!("/nodes/{i}")),
            );
        }
    }
}
