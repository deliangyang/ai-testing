//! Skill manifest: identity, semver string, and dependency pins.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Another Skill this package depends on; `version_req` uses semver constraint syntax (resolved in P3-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillDependency {
    pub id: String,
    pub version_req: String,
}

/// Top-level metadata for a Skill package (develop.md §4.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    /// Semantic version string, e.g. `1.0.0`.
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<SkillDependency>,
}
