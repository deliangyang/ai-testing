use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use model::nodes::DagRef;
use model::{Dag, NodeKind};

use crate::codes;
use crate::{Context, Issue, Rule};

/// Read-only view of stored DAGs, implemented over the store by callers that save DAGs.
pub trait DagCatalog: Send + Sync {
    /// The referenced DAG; `version: None` means the active version.
    fn resolve(&self, dag: &DagRef) -> Option<Dag>;
}

/// In-memory catalog: `(id, version)` entries plus which version is active.
#[derive(Debug, Clone, Default)]
pub struct MapCatalog {
    versions: BTreeMap<(String, u32), Dag>,
    active: BTreeMap<String, u32>,
}

impl MapCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `dag` under its own version (1 if unset) and marks it active.
    #[must_use]
    pub fn with_active(mut self, dag: Dag) -> Self {
        let version = dag.version.unwrap_or(1);
        self.active.insert(dag.id.clone(), version);
        self.with_version(dag)
    }

    /// Adds `dag` under its own version (1 if unset) without changing the active version.
    #[must_use]
    pub fn with_version(mut self, dag: Dag) -> Self {
        let version = dag.version.unwrap_or(1);
        self.versions.insert((dag.id.clone(), version), dag);
        self
    }
}

impl DagCatalog for MapCatalog {
    fn resolve(&self, dag: &DagRef) -> Option<Dag> {
        let version = dag.version.or_else(|| self.active.get(&dag.id).copied())?;
        self.versions.get(&(dag.id.clone(), version)).cloned()
    }
}

/// Subworkflow references must resolve, and following them must never lead back to a DAG
/// already on the path. References to the DAG under validation use the document itself,
/// not its stored versions.
pub struct SubworkflowCycleRule {
    catalog: Arc<dyn DagCatalog>,
}

impl SubworkflowCycleRule {
    pub fn new(catalog: Arc<dyn DagCatalog>) -> Self {
        Self { catalog }
    }

    fn resolve(&self, root: &Dag, dag: &DagRef) -> Option<Dag> {
        if dag.id == root.id {
            Some(root.clone())
        } else {
            self.catalog.resolve(dag)
        }
    }

    /// Depth-first search; returns the first cycle as `[a, b, ..., a]`.
    fn find_cycle(
        &self,
        root: &Dag,
        dag: &Dag,
        stack: &mut Vec<String>,
        explored: &mut BTreeSet<(String, Option<u32>)>,
    ) -> Option<Vec<String>> {
        if let Some(pos) = stack.iter().position(|id| *id == dag.id) {
            let mut cycle = stack[pos..].to_vec();
            cycle.push(dag.id.clone());
            return Some(cycle);
        }
        let key = (dag.id.clone(), dag.version);
        if explored.contains(&key) {
            return None;
        }
        stack.push(dag.id.clone());
        for node in &dag.nodes {
            if let NodeKind::Subworkflow(cfg) = &node.kind {
                if let Some(child) = self.resolve(root, &cfg.dag) {
                    if let Some(cycle) = self.find_cycle(root, &child, stack, explored) {
                        return Some(cycle);
                    }
                }
            }
        }
        stack.pop();
        explored.insert(key);
        None
    }
}

impl Rule for SubworkflowCycleRule {
    fn name(&self) -> &'static str {
        "subworkflow_cycle"
    }

    fn check(&self, ctx: &Context<'_>, out: &mut Vec<Issue>) {
        let root = ctx.dag;
        let mut explored = BTreeSet::new();
        for (i, node) in root.nodes.iter().enumerate() {
            let NodeKind::Subworkflow(cfg) = &node.kind else {
                continue;
            };
            let at = format!("/nodes/{i}/dag");
            let Some(child) = self.resolve(root, &cfg.dag) else {
                let version = cfg
                    .dag
                    .version
                    .map_or("active version".to_owned(), |v| format!("version {v}"));
                out.push(
                    Issue::error(
                        codes::SUBWORKFLOW_UNKNOWN_DAG,
                        format!(
                            "subworkflow DAG '{}' ({version}) does not exist",
                            cfg.dag.id
                        ),
                    )
                    .at_node(&node.id)
                    .at_path(&at),
                );
                continue;
            };
            let mut stack = vec![root.id.clone()];
            if let Some(cycle) = self.find_cycle(root, &child, &mut stack, &mut explored) {
                out.push(
                    Issue::error(
                        codes::SUBWORKFLOW_CYCLE,
                        format!("subworkflow reference cycle: {}", cycle.join(" -> ")),
                    )
                    .at_node(&node.id)
                    .at_path(&at),
                );
            }
        }
    }
}
