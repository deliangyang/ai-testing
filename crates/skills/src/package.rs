//! Skill package: manifest, contracts, internal DAG, and bundled assertion metadata.

use model::nodes::Condition;
use model::Dag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::contract::SkillIoContract;
use crate::error::SkillError;
use crate::manifest::SkillManifest;
use model::Format;

/// Skill-level assertion metadata (mirrors internal assert nodes for prompts, drift, and gate rules).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundledAssertion {
    pub id: String,
    /// Node id inside `dag` whose output is checked.
    pub target: String,
    pub assertion: Condition,
}

/// Self-contained Skill artifact (develop.md §4.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub inputs: SkillIoContract,
    pub outputs: SkillIoContract,
    pub dag: Dag,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<BundledAssertion>,
}

impl SkillPackage {
    pub fn from_json_str(input: &str) -> Result<Self, SkillError> {
        let de = &mut serde_json::Deserializer::from_str(input);
        serde_path_to_error::deserialize(de).map_err(|e| SkillError::parse(Format::Json, e))
    }

    pub fn from_yaml_str(input: &str) -> Result<Self, SkillError> {
        let de = yaml_serde::Deserializer::from_str(input);
        serde_path_to_error::deserialize(de).map_err(|e| SkillError::parse(Format::Yaml, e))
    }

    pub fn to_json_pretty(&self) -> Result<String, SkillError> {
        serde_json::to_string_pretty(self).map_err(|e| SkillError::Serialize(e.to_string()))
    }

    pub fn to_yaml(&self) -> Result<String, SkillError> {
        yaml_serde::to_string(self).map_err(|e| SkillError::Serialize(e.to_string()))
    }
}
