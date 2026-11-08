use model::DagGraph;

use crate::codes;
use crate::{Context, Issue, Rule};

pub struct AcyclicRule;

impl Rule for AcyclicRule {
    fn name(&self) -> &'static str {
        "acyclic"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let Err(cycle) = DagGraph::new(ctx.dag).topo_order() else {
            return;
        };
        let Some(&first) = cycle.first() else {
            return;
        };
        let second = cycle.get(1).copied().unwrap_or(first);
        let mut rendered: Vec<&str> = cycle.clone();
        rendered.push(first);
        let mut issue = Issue::error(
            codes::CYCLE,
            format!("cycle detected: {}", rendered.join(" -> ")),
        )
        .at_node(first);
        if let Some(i) = ctx
            .dag
            .edges
            .iter()
            .position(|e| e.from == first && e.to == second)
        {
            issue = issue.at_path(format!("/edges/{i}"));
        }
        out.push(issue);
    }
}
