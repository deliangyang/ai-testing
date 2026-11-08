//! Graph view of a DAG shared by the gate and the engine.
//!
//! Edges with unknown endpoints and self-loops are ignored here; the gate reports them
//! separately, and only gate-approved DAGs are ever executed.

use std::collections::{BTreeMap, BTreeSet};

use crate::Dag;

#[derive(Debug, Clone)]
pub struct DagGraph<'a> {
    /// Unique node ids in lexicographic order; positions are the internal indices.
    ids: Vec<&'a str>,
    index: BTreeMap<&'a str, usize>,
    succ: Vec<BTreeSet<usize>>,
    pred: Vec<BTreeSet<usize>>,
}

impl<'a> DagGraph<'a> {
    pub fn new(dag: &'a Dag) -> Self {
        let ids: Vec<&str> = dag
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let index: BTreeMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut succ = vec![BTreeSet::new(); ids.len()];
        let mut pred = vec![BTreeSet::new(); ids.len()];
        for edge in &dag.edges {
            if let (Some(&from), Some(&to)) =
                (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
            {
                if from != to {
                    succ[from].insert(to);
                    pred[to].insert(from);
                }
            }
        }
        Self {
            ids,
            index,
            succ,
            pred,
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    pub fn predecessors(&self, id: &str) -> impl Iterator<Item = &'a str> + '_ {
        self.neighbours(id, &self.pred)
    }

    pub fn successors(&self, id: &str) -> impl Iterator<Item = &'a str> + '_ {
        self.neighbours(id, &self.succ)
    }

    fn neighbours<'s>(
        &'s self,
        id: &str,
        adjacency: &'s [BTreeSet<usize>],
    ) -> impl Iterator<Item = &'a str> + 's {
        self.index
            .get(id)
            .into_iter()
            .flat_map(move |&i| adjacency[i].iter().map(move |&j| self.ids[j]))
    }

    /// Kahn's algorithm; among ready nodes the smallest id goes first, so the order is unique.
    ///
    /// On failure returns one cycle as `[a, b, c]` meaning `a -> b -> c -> a`, rotated so the
    /// smallest id comes first.
    pub fn topo_order(&self) -> Result<Vec<&'a str>, Vec<&'a str>> {
        let mut in_degree: Vec<usize> = self.pred.iter().map(BTreeSet::len).collect();
        let mut ready: BTreeSet<usize> =
            (0..self.ids.len()).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(self.ids.len());
        while let Some(i) = ready.pop_first() {
            order.push(self.ids[i]);
            for &j in &self.succ[i] {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        if order.len() == self.ids.len() {
            return Ok(order);
        }
        let remaining: BTreeSet<usize> =
            (0..self.ids.len()).filter(|&i| in_degree[i] > 0).collect();
        Err(self.find_cycle(&remaining))
    }

    /// Every node left after Kahn has a remaining predecessor, so walking predecessors must
    /// revisit a node; the revisited stretch is a cycle.
    fn find_cycle(&self, remaining: &BTreeSet<usize>) -> Vec<&'a str> {
        let Some(&start) = remaining.first() else {
            return Vec::new();
        };
        let mut position: BTreeMap<usize, usize> = BTreeMap::new();
        let mut walk = Vec::new();
        let mut current = start;
        while !position.contains_key(&current) {
            position.insert(current, walk.len());
            walk.push(current);
            match self.pred[current].iter().find(|p| remaining.contains(p)) {
                Some(&p) => current = p,
                None => return Vec::new(),
            }
        }
        let mut cycle: Vec<usize> = walk[position[&current]..].to_vec();
        cycle.reverse();
        if let Some(min_pos) = cycle
            .iter()
            .enumerate()
            .min_by_key(|(_, &n)| n)
            .map(|(i, _)| i)
        {
            cycle.rotate_left(min_pos);
        }
        cycle.into_iter().map(|i| self.ids[i]).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dag(nodes: &[&str], edges: &[(&str, &str)]) -> Dag {
        let nodes: Vec<_> = nodes
            .iter()
            .map(|id| json!({"id": id, "type": "cleanup"}))
            .collect();
        let edges: Vec<_> = edges
            .iter()
            .map(|(f, t)| json!({"from": f, "to": t}))
            .collect();
        Dag::from_json_str(&json!({"id": "g", "nodes": nodes, "edges": edges}).to_string()).unwrap()
    }

    #[test]
    fn topo_order_breaks_ties_by_id() {
        let d = dag(
            &["e", "c", "a", "d", "b"],
            &[("a", "c"), ("a", "d"), ("c", "e"), ("d", "e"), ("b", "e")],
        );
        assert_eq!(
            DagGraph::new(&d).topo_order().unwrap(),
            vec!["a", "b", "c", "d", "e"]
        );
    }

    #[test]
    fn independent_of_node_and_edge_declaration_order() {
        let a = dag(&["x", "y", "z"], &[("x", "z"), ("y", "z")]);
        let b = dag(&["z", "y", "x"], &[("y", "z"), ("x", "z")]);
        assert_eq!(
            DagGraph::new(&a).topo_order(),
            DagGraph::new(&b).topo_order()
        );
    }

    #[test]
    fn reports_cycle_rotated_to_smallest_id() {
        let d = dag(
            &["c", "b", "a", "z"],
            &[("b", "c"), ("c", "a"), ("a", "b"), ("z", "a")],
        );
        assert_eq!(
            DagGraph::new(&d).topo_order().unwrap_err(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn finds_cycle_downstream_of_acyclic_part() {
        let d = dag(&["s", "p", "q"], &[("s", "p"), ("p", "q"), ("q", "p")]);
        assert_eq!(DagGraph::new(&d).topo_order().unwrap_err(), vec!["p", "q"]);
    }

    #[test]
    fn ignores_self_loops_and_dangling_edges() {
        let d = dag(&["a", "b"], &[("a", "a"), ("a", "ghost"), ("a", "b")]);
        let g = DagGraph::new(&d);
        assert_eq!(g.topo_order().unwrap(), vec!["a", "b"]);
        assert_eq!(g.successors("a").collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(g.predecessors("b").collect::<Vec<_>>(), vec!["a"]);
        assert!(!g.contains("ghost"));
    }
}
