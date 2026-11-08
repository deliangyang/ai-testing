use model::scope::node_refs;
use model::{DagGraph, NodeKind, VarRef};

use crate::codes;
use crate::{Context, Issue, Rule};

/// Node id under which a subworkflow child sees its inputs.
pub const INPUT_NODE_ID: &str = "input";

/// `${x.output...}` and assert `target` must name a transitive predecessor of the node.
pub struct ReferenceRule;

impl Rule for ReferenceRule {
    fn name(&self) -> &'static str {
        "references"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let graph = DagGraph::new(ctx.dag);
        for (i, node) in ctx.dag.nodes.iter().enumerate() {
            let base = format!("/nodes/{i}");
            let ancestors = graph.ancestors(&node.id);
            let mut check = |target: &str, pointer: String, what: &str| {
                let problem = if target == node.id {
                    Some((
                        codes::NOT_ANCESTOR,
                        format!("{what} refers to the node itself"),
                    ))
                } else if !graph.contains(target) {
                    Some((
                        codes::UNKNOWN_NODE,
                        format!("{what} refers to unknown node '{target}'"),
                    ))
                } else if !ancestors.contains(target) {
                    Some((
                        codes::NOT_ANCESTOR,
                        format!("{what} refers to '{target}', which is not an upstream node"),
                    ))
                } else {
                    None
                };
                if let Some((code, message)) = problem {
                    out.push(
                        Issue::error(code, message)
                            .at_node(&node.id)
                            .at_path(pointer),
                    );
                }
            };

            if let NodeKind::Assert(cfg) = &node.kind {
                check(&cfg.target, format!("{base}/target"), "assert target");
            }
            match node_refs(node) {
                Ok(refs) => {
                    for located in refs {
                        if let VarRef::Node { node: target, .. } = &located.var {
                            if target != INPUT_NODE_ID {
                                let what = format!("reference {}", located.var);
                                check(target, format!("{base}{}", located.pointer), &what);
                            }
                        }
                    }
                }
                Err(err) => out.push(
                    Issue::error(codes::INVALID_TEMPLATE, err.source.to_string())
                        .at_node(&node.id)
                        .at_path(format!("{base}{}", err.pointer)),
                ),
            }
        }
    }
}
