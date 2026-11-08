//! Declarative DAG types, variable reference parsing and canonical JSON.

pub mod dag;
pub mod error;
pub mod nodes;

pub use dag::{Dag, Edge, Node, NodeKind};
pub use error::{Format, ModelError};
