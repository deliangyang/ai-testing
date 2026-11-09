//! Deterministic mechanical repairs for gate-fixable structure, reference and poll params (§6.6).
//!
//! LLM-backed semantic calibration is out of scope here.

use std::collections::BTreeSet;

use model::nodes::PollConfig;
use model::scope::node_refs;
use model::{Dag, DagGraph, Edge, NodeKind, VarRef};

use crate::codes;
use crate::rules::INPUT_NODE_ID;

/// Default poll backoff when `backoff_ms` is empty (aligned with `examples/async-poll.yaml` scale).
pub const DEFAULT_POLL_BACKOFF_MS: &[u64] = &[1000];
/// Default poll timeout when `timeout_ms` is zero or shorter than the first backoff interval.
pub const DEFAULT_POLL_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RepairRecord {
    pub code: String,
    pub json_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalibrateResult {
    pub dag: Dag,
    pub repairs: Vec<RepairRecord>,
}

/// Applies all supported deterministic repairs in a fixed order.
#[must_use]
pub fn calibrate(mut dag: Dag) -> CalibrateResult {
    let mut repairs = Vec::new();
    repair_poll_nodes(&mut dag, &mut repairs);
    repair_reference_edges(&mut dag, &mut repairs);
    CalibrateResult { dag, repairs }
}

fn repair_poll_nodes(dag: &mut Dag, repairs: &mut Vec<RepairRecord>) {
    for (i, node) in dag.nodes.iter_mut().enumerate() {
        let NodeKind::Poll(cfg) = &mut node.kind else {
            continue;
        };
        repair_poll_config(cfg, i, repairs);
    }
}

fn repair_poll_config(cfg: &mut PollConfig, node_index: usize, repairs: &mut Vec<RepairRecord>) {
    let base = format!("/nodes/{node_index}");
    if cfg.backoff_ms.is_empty() {
        cfg.backoff_ms = DEFAULT_POLL_BACKOFF_MS.to_vec();
        repairs.push(RepairRecord {
            code: codes::REPAIR_POLL_BACKOFF.to_owned(),
            json_path: format!("{base}/backoff_ms"),
            message: format!("set empty backoff_ms to {:?}", DEFAULT_POLL_BACKOFF_MS),
        });
    }
    let first = cfg.backoff_ms.first().copied().unwrap_or(1);
    if cfg.timeout_ms == 0 || cfg.timeout_ms < first {
        let new_timeout = DEFAULT_POLL_TIMEOUT_MS.max(first);
        let old = cfg.timeout_ms;
        cfg.timeout_ms = new_timeout;
        repairs.push(RepairRecord {
            code: codes::REPAIR_POLL_TIMEOUT.to_owned(),
            json_path: format!("{base}/timeout_ms"),
            message: format!("set timeout_ms from {old} to {new_timeout} (first backoff {first})"),
        });
    }
}

fn repair_reference_edges(dag: &mut Dag, repairs: &mut Vec<RepairRecord>) {
    let mut to_add: BTreeSet<(String, String)> = BTreeSet::new();
    for node in &dag.nodes {
        let graph = DagGraph::new(dag);
        let ancestors = graph.ancestors(&node.id);
        let mut need = |target: &str| {
            if target == node.id || target == INPUT_NODE_ID {
                return;
            }
            if !graph.contains(target) {
                return;
            }
            if ancestors.contains(target) {
                return;
            }
            if graph.ancestors(target).contains(node.id.as_str()) {
                return;
            }
            to_add.insert((target.to_owned(), node.id.clone()));
        };
        if let NodeKind::Assert(cfg) = &node.kind {
            need(&cfg.target);
        }
        if let Ok(refs) = node_refs(node) {
            for located in refs {
                if let VarRef::Node { node: target, .. } = &located.var {
                    need(target);
                }
            }
        }
    }
    for (from, to) in to_add {
        if dag.edges.iter().any(|e| e.from == from && e.to == to) {
            continue;
        }
        let edge_index = dag.edges.len();
        dag.edges.push(Edge {
            from: from.clone(),
            to: to.clone(),
        });
        repairs.push(RepairRecord {
            code: codes::REPAIR_ADD_EDGE.to_owned(),
            json_path: format!("/edges/{edge_index}"),
            message: format!("added edge {from} -> {to} so node output is an upstream dependency"),
        });
    }
}
