#![allow(clippy::unwrap_used)]

use generator::{
    default_prompts_dir, render, ApiSchemaInput, CorrectionPreferences, DagGenerateInputs,
    GenerationContext, PromptCatalog, SkillContracts,
};
use importer::{normalize_api_spec, ApiSpec};
use serde_json::json;

#[test]
fn assembled_context_placeholders_snapshot() {
    let inputs = DagGenerateInputs {
        natural_language_description: "覆盖创建订单、轮询支付、查询订单状态".into(),
        business_rules: "- 创建成功后订单状态必须为 pending".into(),
        api_schema: ApiSchemaInput::SchemaJson(
            r#"{"openapi":"3.0.0","info":{"title":"Orders","version":"1.0.0"}}"#.into(),
        ),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    };
    let ctx = GenerationContext::assemble(&inputs).unwrap();
    insta::assert_yaml_snapshot!(ctx.dag);
}

#[test]
fn assembled_from_importer_api_specs_snapshot() {
    let specs = vec![
        normalize_api_spec(ApiSpec {
            id: "orders.create".into(),
            name: "Create order".into(),
            method: "POST".into(),
            path: "/orders".into(),
            version: None,
            params: vec![],
            response_schema: Some(json!({
                "type": "object",
                "properties": { "id": { "type": "string" } }
            })),
            auth_scheme_ref: Some("keychain://orders-bearer".into()),
        }),
        normalize_api_spec(ApiSpec {
            id: "orders.get".into(),
            name: "Get order".into(),
            method: "GET".into(),
            path: "/orders/{id}".into(),
            version: None,
            params: vec![],
            response_schema: None,
            auth_scheme_ref: None,
        }),
    ];
    let json = serde_json::to_string(&specs).unwrap();
    let docs = generator::parse_endpoint_docs_json(&json).unwrap();
    let inputs = DagGenerateInputs {
        natural_language_description: "下单后查询详情".into(),
        business_rules: String::new(),
        api_schema: ApiSchemaInput::EndpointDocs(docs),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    };
    let ctx = GenerationContext::assemble(&inputs).unwrap();
    insta::assert_yaml_snapshot!("api_specs_assembled", ctx.dag);
}

#[test]
fn full_render_after_assembly_snapshot() {
    let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
    let tmpl = catalog.load_dag_generate_main().unwrap();
    let inputs = DagGenerateInputs {
        natural_language_description: "演示链路".into(),
        business_rules: "最少一条 http 断言".into(),
        api_schema: ApiSchemaInput::SchemaJson(r#"{"paths":{}}"#.into()),
        skill_contracts: SkillContracts::default(),
        preferences: CorrectionPreferences::default(),
    };
    let ctx = GenerationContext::assemble(&inputs).unwrap();
    let rendered = render(&tmpl, &ctx.dag);
    insta::assert_snapshot!(rendered);
}
