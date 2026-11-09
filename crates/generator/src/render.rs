use serde::Serialize;

use crate::template::PromptTemplate;

/// Values for the DAG generation main template placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DagGenerateContext {
    pub openapi_json: String,
    pub business_rules: String,
    pub skill_contracts: String,
    pub preference_rules: String,
}

impl DagGenerateContext {
    pub fn placeholder_map(&self) -> [(&'static str, &str); 4] {
        [
            ("openapi_json", &self.openapi_json),
            ("business_rules", &self.business_rules),
            ("skill_contracts", &self.skill_contracts),
            ("preference_rules", &self.preference_rules),
        ]
    }
}

/// Replace `{placeholder}` tokens in `template.body` with context values.
pub fn render(template: &PromptTemplate, ctx: &DagGenerateContext) -> String {
    let mut out = template.body.clone();
    for (name, value) in ctx.placeholder_map() {
        let marker = format!("{{{name}}}");
        out = out.replace(&marker, value);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::loader::{default_prompts_dir, PromptCatalog};

    #[test]
    fn render_replaces_all_placeholders() {
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let tmpl = catalog.load_dag_generate_main().unwrap();
        let ctx = DagGenerateContext {
            openapi_json: r#"{"openapi":"3.0.0"}"#.into(),
            business_rules: "rule-a".into(),
            skill_contracts: "(none)".into(),
            preference_rules: "(none)".into(),
        };
        let rendered = render(&tmpl, &ctx);
        assert!(!rendered.contains("{openapi_json}"));
        assert!(rendered.contains(r#"{"openapi":"3.0.0"}"#));
        assert!(rendered.contains("rule-a"));
    }
}
