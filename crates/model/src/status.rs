//! Node execution states (develop.md §3.2), shared by the engine and the store.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    /// Sub-state of a running poll node; may repeat for every iteration.
    Polling,
    Completed,
    Failed,
    /// Not executed because an upstream node failed, or not finished because the run was
    /// cancelled.
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("illegal node transition {from} -> {to}")]
pub struct InvalidTransition {
    pub from: NodeStatus,
    pub to: NodeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown node status '{0}'")]
pub struct ParseStatusError(pub String);

impl NodeStatus {
    pub const ALL: [NodeStatus; 7] = [
        NodeStatus::Pending,
        NodeStatus::Ready,
        NodeStatus::Running,
        NodeStatus::Polling,
        NodeStatus::Completed,
        NodeStatus::Failed,
        NodeStatus::Skipped,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NodeStatus::Pending => "PENDING",
            NodeStatus::Ready => "READY",
            NodeStatus::Running => "RUNNING",
            NodeStatus::Polling => "POLLING",
            NodeStatus::Completed => "COMPLETED",
            NodeStatus::Failed => "FAILED",
            NodeStatus::Skipped => "SKIPPED",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    }

    /// `PENDING -> READY -> RUNNING -> (POLLING ->) COMPLETED | FAILED`; nodes that have not
    /// started may be `SKIPPED`, and so may running nodes interrupted by a run cancellation
    /// (ADR-003). Staying in a non-terminal status is allowed so a polling node can report
    /// every iteration. Terminal statuses are final.
    pub fn can_transition_to(self, to: NodeStatus) -> bool {
        use NodeStatus::*;
        if self.is_terminal() {
            return false;
        }
        self == to
            || matches!(
                (self, to),
                (Pending, Ready | Skipped)
                    | (Ready, Running | Skipped)
                    | (Running, Polling | Completed | Failed | Skipped)
                    | (Polling, Completed | Failed | Skipped)
            )
    }

    pub fn transition(self, to: NodeStatus) -> Result<NodeStatus, InvalidTransition> {
        if self.can_transition_to(to) {
            Ok(to)
        } else {
            Err(InvalidTransition { from: self, to })
        }
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for NodeStatus {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NodeStatus::ALL
            .into_iter()
            .find(|st| st.as_str() == s)
            .ok_or_else(|| ParseStatusError(s.to_owned()))
    }
}
