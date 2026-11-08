use std::collections::{BTreeMap, BTreeSet};

use crate::codes;
use crate::{Context, Issue, Rule};

/// Node ids usable in `${...}` references; `input`, `env` and `auth` are taken by the template
/// grammar and subworkflow inputs.
pub const RESERVED_NODE_IDS: [&str; 3] = ["input", "env", "auth"];

pub struct StructureRule;

impl Rule for StructureRule {
    fn name(&self) -> &'static str {
        "structure"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, node) in ctx.dag.nodes.iter().enumerate() {
            let path = format!("/nodes/{i}/id");
            if let Some(first) = seen.insert(&node.id, i) {
                seen.insert(&node.id, first);
                out.push(
                    Issue::error(
                        codes::DUPLICATE_NODE_ID,
                        format!("node id '{}' is already used by /nodes/{first}", node.id),
                    )
                    .at_node(&node.id)
                    .at_path(&path),
                );
            }
            if !is_valid_node_id(&node.id) {
                out.push(
                    Issue::error(
                        codes::INVALID_NODE_ID,
                        format!(
                            "node id '{}' must match [A-Za-z_][A-Za-z0-9_-]* to be referenceable",
                            node.id
                        ),
                    )
                    .at_node(&node.id)
                    .at_path(&path),
                );
            } else if RESERVED_NODE_IDS.contains(&node.id.as_str()) {
                out.push(
                    Issue::error(
                        codes::RESERVED_NODE_ID,
                        format!("node id '{}' is reserved", node.id),
                    )
                    .at_node(&node.id)
                    .at_path(&path),
                );
            }
        }

        let mut edges_seen: BTreeSet<(&str, &str)> = BTreeSet::new();
        for (i, edge) in ctx.dag.edges.iter().enumerate() {
            let from_known = seen.contains_key(edge.from.as_str());
            let to_known = seen.contains_key(edge.to.as_str());
            let anchor = if from_known { &edge.from } else { &edge.to };
            for (end, id, known) in [("from", &edge.from, from_known), ("to", &edge.to, to_known)] {
                if !known {
                    let mut issue = Issue::error(
                        codes::DANGLING_EDGE,
                        format!("edge {end} '{id}' does not name a node"),
                    )
                    .at_path(format!("/edges/{i}/{end}"));
                    if from_known || to_known {
                        issue = issue.at_node(anchor);
                    }
                    out.push(issue);
                }
            }
            if edge.from == edge.to {
                out.push(
                    Issue::error(
                        codes::SELF_LOOP,
                        format!("edge from '{}' to itself", edge.from),
                    )
                    .at_node(&edge.from)
                    .at_path(format!("/edges/{i}")),
                );
            } else if !edges_seen.insert((&edge.from, &edge.to)) {
                out.push(
                    Issue::error(
                        codes::DUPLICATE_EDGE,
                        format!("edge '{}' -> '{}' is declared twice", edge.from, edge.to),
                    )
                    .at_node(&edge.from)
                    .at_path(format!("/edges/{i}")),
                );
            }
        }
    }
}

pub fn is_valid_node_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
