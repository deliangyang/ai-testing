#![allow(dead_code, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use engine::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use model::{Dag, Node};
use serde_json::{json, Value};

/// Records every call; fails the nodes listed in `fail`, otherwise outputs `{"from": <id>}`.
#[derive(Default)]
pub struct MockExecutor {
    pub fail: BTreeSet<String>,
    pub calls: Mutex<Vec<(String, ExecContext)>>,
}

impl MockExecutor {
    pub fn failing(ids: &[&str]) -> Self {
        Self {
            fail: ids.iter().map(|s| (*s).to_owned()).collect(),
            ..Self::default()
        }
    }

    pub fn order(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn context_of(&self, id: &str) -> ExecContext {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .find(|(n, _)| n == id)
            .map(|(_, ctx)| ctx.clone())
            .unwrap()
    }
}

impl NodeExecutor for MockExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        self.calls.lock().unwrap().push((node.id.clone(), ctx));
        let result = if self.fail.contains(&node.id) {
            Err(ExecError::new(
                ExecErrorKind::Assertion,
                format!("{} failed", node.id),
            ))
        } else {
            Ok(json!({"from": node.id}))
        };
        Box::pin(async move { result })
    }
}

/// A DAG of http nodes; `nodes` are declared in the given order.
pub fn dag(nodes: &[&str], edges: &[(&str, &str)]) -> Dag {
    dag_with_cleanup(nodes, &[], edges)
}

/// Like [`dag`], with `cleanup` added as cleanup nodes after the http nodes.
pub fn dag_with_cleanup(http: &[&str], cleanup: &[&str], edges: &[(&str, &str)]) -> Dag {
    let nodes: Vec<Value> = http
        .iter()
        .map(|id| json!({"id": id, "type": "http", "method": "GET", "path": "/"}))
        .chain(
            cleanup
                .iter()
                .map(|id| json!({"id": id, "type": "cleanup"})),
        )
        .collect();
    let edges: Vec<Value> = edges
        .iter()
        .map(|(f, t)| json!({"from": f, "to": t}))
        .collect();
    Dag::from_json_str(&json!({"id": "t", "nodes": nodes, "edges": edges}).to_string()).unwrap()
}

pub fn keys(map: &BTreeMap<String, Value>) -> Vec<&str> {
    map.keys().map(String::as_str).collect()
}
