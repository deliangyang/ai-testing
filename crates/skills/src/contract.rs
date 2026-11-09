//! Typed input/output contracts exposed to parent DAGs and the generator.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-serializable value kinds for Skill boundaries (gate P3-05 maps bindings to these).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SkillValueType {
    String,
    Integer,
    Number,
    Boolean,
    Object,
    Array,
    Null,
}

/// One named field on the Skill input or output contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillField {
    pub name: String,
    pub ty: SkillValueType,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional JSON Schema for nested object/array shapes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
}

/// Declarative contract for Skill inputs or outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillIoContract {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<SkillField>,
}
