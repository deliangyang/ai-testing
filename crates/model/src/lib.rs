//! Declarative DAG types, variable reference parsing and canonical JSON.

pub mod canonical;
pub mod dag;
pub mod error;
pub mod graph;
pub mod nodes;
pub mod schema;
pub mod scope;
pub mod template;

pub use canonical::{canonical_json, content_hash, to_canonical_json, CanonicalError};
pub use dag::{Dag, Edge, Node, NodeKind};
pub use error::{Format, ModelError};
pub use graph::DagGraph;
pub use template::{
    parse_json_path, refs_in_value, JsonPathError, LocatedRef, PathSeg, Template, TemplateError,
    VarRef,
};
