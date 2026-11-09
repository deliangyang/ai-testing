use serde::{Deserialize, Serialize};

/// A versioned prompt body (placeholders use `{name}` syntax).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub prompt_version: String,
    pub body: String,
}

impl PromptTemplate {
    pub fn placeholder_names() -> &'static [&'static str] {
        &[
            "openapi_json",
            "business_rules",
            "skill_contracts",
            "preference_rules",
        ]
    }
}
