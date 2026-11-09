//! Reading DAG and Skill documents from disk.

use std::path::Path;

use gate::{Issue, Report};
use model::Dag;
use skills::SkillPackage;

/// Issue code for a document that is not a well-formed DAG; the gate never sees it.
pub const PARSE_CODE: &str = "document.parse";

pub enum Loaded {
    Dag(Box<Dag>),
    /// The file was read but is not a DAG; the report holds one `document.parse` error.
    Invalid(Report),
}

/// `.json` files are parsed as JSON, everything else as YAML.
pub fn load_dag(path: &Path) -> anyhow::Result<Loaded> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let is_json = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
    let parsed = if is_json {
        Dag::from_json_str(&text)
    } else {
        Dag::from_yaml_str(&text)
    };
    Ok(match parsed {
        Ok(dag) => Loaded::Dag(Box::new(dag)),
        Err(e) => Loaded::Invalid(Report::new(vec![Issue::error(PARSE_CODE, e.to_string())])),
    })
}

/// `.json` files are parsed as JSON, everything else as YAML.
pub fn load_skill(path: &Path) -> anyhow::Result<Result<SkillPackage, Report>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let is_json = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
    let parsed = if is_json {
        SkillPackage::from_json_str(&text)
    } else {
        SkillPackage::from_yaml_str(&text)
    };
    Ok(match parsed {
        Ok(pkg) => Ok(pkg),
        Err(e) => Err(Report::new(vec![Issue::error(PARSE_CODE, e.to_string())])),
    })
}
