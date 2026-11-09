//! Skill package format: manifest, typed I/O contracts, internal DAG, bundled assertions.

pub mod contract;
pub mod error;
pub mod manifest;
pub mod package;
pub mod schema;

pub use contract::{SkillField, SkillIoContract, SkillValueType};
pub use error::SkillError;
pub use manifest::{SkillDependency, SkillManifest};
pub use package::{BundledAssertion, SkillPackage};
pub use schema::{skill_schema, skill_schema_json};
