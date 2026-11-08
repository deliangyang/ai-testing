//! Declarative DAG document (§4.1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{Format, ModelError};
use crate::nodes::{
    AssertConfig, CleanupConfig, DbQueryConfig, HttpConfig, PollConfig, SubworkflowConfig,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dag {
    pub id: String,
    /// Assigned by the store on save; absent in hand-written files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Value>,
}

impl Dag {
    pub fn from_json_str(input: &str) -> Result<Self, ModelError> {
        let de = &mut serde_json::Deserializer::from_str(input);
        serde_path_to_error::deserialize(de).map_err(|e| ModelError::parse(Format::Json, e))
    }

    pub fn from_yaml_str(input: &str) -> Result<Self, ModelError> {
        let de = yaml_serde::Deserializer::from_str(input);
        serde_path_to_error::deserialize(de).map_err(|e| ModelError::parse(Format::Yaml, e))
    }

    pub fn to_json_pretty(&self) -> Result<String, ModelError> {
        serde_json::to_string_pretty(self).map_err(|e| ModelError::Serialize(e.to_string()))
    }

    pub fn to_yaml(&self) -> Result<String, ModelError> {
        yaml_serde::to_string(self).map_err(|e| ModelError::Serialize(e.to_string()))
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// A node is `{ id, name?, type, ...type-specific config }` in a single flat object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawNode")]
pub struct Node {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(flatten)]
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    Http(HttpConfig),
    Poll(PollConfig),
    Assert(AssertConfig),
    DbQuery(DbQueryConfig),
    Subworkflow(SubworkflowConfig),
    Cleanup(CleanupConfig),
}

impl NodeKind {
    pub const TYPE_NAMES: [&'static str; 6] = [
        "http",
        "poll",
        "assert",
        "db_query",
        "subworkflow",
        "cleanup",
    ];

    pub fn type_name(&self) -> &'static str {
        match self {
            NodeKind::Http(_) => "http",
            NodeKind::Poll(_) => "poll",
            NodeKind::Assert(_) => "assert",
            NodeKind::DbQuery(_) => "db_query",
            NodeKind::Subworkflow(_) => "subworkflow",
            NodeKind::Cleanup(_) => "cleanup",
        }
    }
}

/// Intermediate form so each config type can keep `deny_unknown_fields`, which serde does not
/// support together with `flatten`.
#[derive(Deserialize)]
struct RawNode {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

impl TryFrom<RawNode> for Node {
    type Error = String;

    fn try_from(raw: RawNode) -> Result<Self, Self::Error> {
        fn config<T: serde::de::DeserializeOwned>(
            node_id: &str,
            rest: Map<String, Value>,
        ) -> Result<T, String> {
            serde_path_to_error::deserialize(Value::Object(rest))
                .map_err(|e| format!("node '{node_id}': {}: {}", e.path(), e.inner()))
        }

        let id = raw.id;
        let kind = match raw.node_type.as_str() {
            "http" => NodeKind::Http(config(&id, raw.rest)?),
            "poll" => NodeKind::Poll(config(&id, raw.rest)?),
            "assert" => NodeKind::Assert(config(&id, raw.rest)?),
            "db_query" => NodeKind::DbQuery(config(&id, raw.rest)?),
            "subworkflow" => NodeKind::Subworkflow(config(&id, raw.rest)?),
            "cleanup" => NodeKind::Cleanup(config(&id, raw.rest)?),
            other => {
                return Err(format!(
                    "node '{id}': unknown node type '{other}', expected one of {}",
                    NodeKind::TYPE_NAMES.join(", ")
                ))
            }
        };
        Ok(Node {
            id,
            name: raw.name,
            kind,
        })
    }
}
