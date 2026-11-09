#![allow(clippy::unwrap_used)]

use generator::{default_prompts_dir, render, DagGenerateContext, PromptCatalog};

#[test]
fn dag_generate_main_render_snapshot() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let ctx = DagGenerateContext {
        openapi_json: r#"{"openapi":"3.0.0","info":{"title":"Demo"}}"#.into(),
        business_rules: "- 创建订单后必须能查询状态".into(),
        skill_contracts: "(empty)".into(),
        preference_rules: "(empty)".into(),
    };
    let rendered = render(&tmpl, &ctx);
    insta::assert_snapshot!(rendered);
}
