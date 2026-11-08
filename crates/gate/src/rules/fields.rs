use model::scope::node_refs;
use model::{DagGraph, VarRef};

use crate::codes;
use crate::output::{output_shape, path_reaches_body, render_path, OutputShape};
use crate::schema_path::{lookup, Lookup};
use crate::{Context, Issue, Rule};

/// Referenced output fields must exist in the upstream node's declared output shape
/// (see [`crate::output`]).
pub struct FieldRule;

impl Rule for FieldRule {
    fn name(&self) -> &'static str {
        "fields"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let graph = DagGraph::new(ctx.dag);
        for (i, node) in ctx.dag.nodes.iter().enumerate() {
            let Ok(refs) = node_refs(node) else { continue };
            let ancestors = graph.ancestors(&node.id);
            for located in refs {
                let VarRef::Node { node: target, path } = &located.var else {
                    continue;
                };
                // Bad targets are already reported by ReferenceRule.
                if !ancestors.contains(target.as_str()) {
                    continue;
                }
                let Some(target_node) = ctx.dag.node(target) else {
                    continue;
                };
                let pointer = format!("/nodes/{i}{}", located.pointer);
                let issue = match output_shape(target_node) {
                    OutputShape::Schema(schema) => match lookup(&schema, path) {
                        Lookup::Found => None,
                        Lookup::Missing { segment, reason } => Some(Issue::error(
                            codes::UNKNOWN_FIELD,
                            format!(
                                "{} does not exist on '{target}' output ({reason}, at '{}')",
                                located.var,
                                render_path(
                                    "output",
                                    &path[..=segment.min(path.len().saturating_sub(1))]
                                )
                            ),
                        )),
                    },
                    OutputShape::Unverified { reason } if path_reaches_body(path) => {
                        Some(Issue::warn(
                            codes::UNVERIFIED_FIELD,
                            format!("{} cannot be checked: {reason}", located.var),
                        ))
                    }
                    OutputShape::Unverified { .. } => None,
                };
                if let Some(issue) = issue {
                    out.push(issue.at_node(&node.id).at_path(pointer));
                }
            }
        }
    }
}
