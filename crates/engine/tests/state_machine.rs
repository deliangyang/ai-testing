#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use engine::InvalidTransition;
use engine::NodeStatus::{self, Completed, Failed, Pending, Polling, Ready, Running, Skipped};

/// Every allowed move other than staying in place, spelled out independently of the code.
const ALLOWED: [(NodeStatus, NodeStatus); 11] = [
    (Pending, Ready),
    (Pending, Skipped),
    (Ready, Running),
    (Ready, Skipped),
    (Running, Polling),
    (Running, Completed),
    (Running, Failed),
    // Interrupted by a run cancellation (ADR-003).
    (Running, Skipped),
    (Polling, Completed),
    (Polling, Failed),
    (Polling, Skipped),
];

fn expected(from: NodeStatus, to: NodeStatus) -> bool {
    (from == to && !from.is_terminal()) || ALLOWED.contains(&(from, to))
}

#[test]
fn transition_matrix_covers_every_pair() {
    let mut checked = 0;
    for from in NodeStatus::ALL {
        for to in NodeStatus::ALL {
            let ok = expected(from, to);
            assert_eq!(from.can_transition_to(to), ok, "{from} -> {to}");
            match from.transition(to) {
                Ok(next) => {
                    assert!(ok, "{from} -> {to} should be rejected");
                    assert_eq!(next, to);
                }
                Err(err) => {
                    assert!(!ok, "{from} -> {to} should be allowed");
                    assert_eq!(err, InvalidTransition { from, to });
                    assert_eq!(
                        err.to_string(),
                        format!("illegal node transition {from} -> {to}")
                    );
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 49);
}

#[test]
fn terminal_states_are_final() {
    let terminal: Vec<_> = NodeStatus::ALL
        .into_iter()
        .filter(|s| s.is_terminal())
        .collect();
    assert_eq!(terminal, [Completed, Failed, Skipped]);
    for from in terminal {
        assert!(NodeStatus::ALL
            .iter()
            .all(|&to| !from.can_transition_to(to)));
    }
}

#[test]
fn every_state_is_reachable_and_every_non_terminal_can_finish() {
    let reachable = |start: NodeStatus| -> BTreeSet<NodeStatus> {
        let mut seen = BTreeSet::from([start]);
        let mut frontier = vec![start];
        while let Some(s) = frontier.pop() {
            for to in NodeStatus::ALL {
                if s.can_transition_to(to) && seen.insert(to) {
                    frontier.push(to);
                }
            }
        }
        seen
    };
    assert_eq!(
        reachable(Pending),
        NodeStatus::ALL.into_iter().collect::<BTreeSet<_>>()
    );
    for s in NodeStatus::ALL.into_iter().filter(|s| !s.is_terminal()) {
        assert!(
            reachable(s).iter().any(|t| t.is_terminal()),
            "{s} cannot reach a terminal state"
        );
    }
}

#[test]
fn status_names_are_stable_in_text_and_json() {
    let names = [
        "PENDING",
        "READY",
        "RUNNING",
        "POLLING",
        "COMPLETED",
        "FAILED",
        "SKIPPED",
    ];
    for (status, name) in NodeStatus::ALL.into_iter().zip(names) {
        assert_eq!(status.to_string(), name);
        assert_eq!(name.parse::<NodeStatus>().unwrap(), status);
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{name}\"")
        );
        assert_eq!(
            serde_json::from_str::<NodeStatus>(&format!("\"{name}\"")).unwrap(),
            status
        );
    }
    assert!("running".parse::<NodeStatus>().is_err());
}
