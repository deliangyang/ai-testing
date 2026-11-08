//! Declarative DAG types, variable reference parsing and canonical JSON.

pub mod canonical;
pub mod dag;
pub mod error;
pub mod nodes;
pub mod template;

pub use canonical::{canonical_json, content_hash, to_canonical_json, CanonicalError};
pub use dag::{Dag, Edge, Node, NodeKind};
pub use error::{Format, ModelError};
pub use template::{refs_in_value, LocatedRef, PathSeg, Template, TemplateError, VarRef};
