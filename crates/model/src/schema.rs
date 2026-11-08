//! JSON Schema of the DAG document, published as `schemas/dag.schema.json` for the UI and the
//! AI generator. A test keeps the file in sync with the Rust types.

use schemars::{schema_for, Schema};

use crate::Dag;

pub fn dag_schema() -> Schema {
    schema_for!(Dag)
}

/// Pretty-printed with a trailing newline, exactly as stored in the repository.
pub fn dag_schema_json() -> String {
    let mut out = serde_json::to_string_pretty(&dag_schema()).unwrap_or_default();
    out.push('\n');
    out
}
