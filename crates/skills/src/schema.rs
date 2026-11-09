//! JSON Schema of the Skill package, published as `schemas/skill.schema.json`.

use schemars::{schema_for, Schema};

use crate::package::SkillPackage;

pub fn skill_schema() -> Schema {
    schema_for!(SkillPackage)
}

/// Pretty-printed with a trailing newline, exactly as stored in the repository.
pub fn skill_schema_json() -> String {
    let mut out = serde_json::to_string_pretty(&skill_schema()).unwrap_or_default();
    out.push('\n');
    out
}
