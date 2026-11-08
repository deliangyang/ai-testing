//! Node state machine (develop.md §3.2).
//!
//! The type lives in `model` so the store persists exactly the states the engine drives; the
//! engine is the only component that moves a node from one state to the next.

pub use model::{InvalidTransition, NodeStatus};
