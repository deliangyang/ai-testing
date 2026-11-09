#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use common::clock::{FixedClock, Timestamp};
use importer::{
    build_markdown_import_request, extract_api_spec_drafts_from_markdown, persist_api_spec_drafts,
    MarkdownImportRequestOptions, MARKDOWN_IMPORT_PURPOSE,
};
use llm_gateway::{
    CassetteProvider, ChatMessage, ChatRequest, ChatResponse, MessageRole, TokenUsage,
};
use store::{ApiSpecRepository, ApiSpecStatus, Db};

pub(crate) const SAMPLE_MARKDOWN: &str =
    "# API\n\n## GET /ping\nHealth check.\n\n## POST /orders\nCreate order.\n";

const SAMPLE_DRAFTS_JSON: &str = r#"[
  {
    "spec": {
      "id": "ping.get",
      "name": "Health check",
      "method": "GET",
      "path": "/ping",
      "params": []
    },
    "confidence": 0.92,
    "source_lines": [3, 4]
  },
  {
    "spec": {
      "id": "orders.post",
      "name": "Create order",
      "method": "POST",
      "path": "/orders",
      "params": []
    },
    "confidence": 0.85,
    "source_lines": [6, 7]
  }
]"#;

const FENCED_DRAFTS: &str = "已从文档提取接口候选。\n\n```json\n\
[
  {
    \"spec\": {
      \"id\": \"ping.get\",
      \"name\": \"Health check\",
      \"method\": \"GET\",
      \"path\": \"/ping\",
      \"params\": []
    },
    \"confidence\": 0.88,
    \"source_lines\": [3, 4]
  }
]\n```\n";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes")
}

fn options(model: &str) -> MarkdownImportRequestOptions {
    MarkdownImportRequestOptions {
        model: model.into(),
        role: MessageRole::User,
    }
}

#[tokio::test]
async fn cassette_replay_extracts_plain_json_drafts() {
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let drafts = extract_api_spec_drafts_from_markdown(
        &provider,
        SAMPLE_MARKDOWN,
        &options("cassette-md-plain-json"),
    )
    .await
    .unwrap();
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].spec.id, "ping.get");
    assert!(drafts[0].confidence > 0.9);
    assert_eq!(drafts[1].spec.method, "POST");
    assert_eq!(drafts[1].source_lines, vec![6, 7]);
    assert_eq!(MARKDOWN_IMPORT_PURPOSE, "import_markdown");
}

#[tokio::test]
async fn cassette_replay_extracts_fenced_json_drafts() {
    let provider = CassetteProvider::from_dir(fixtures_dir()).unwrap();
    let drafts = extract_api_spec_drafts_from_markdown(
        &provider,
        SAMPLE_MARKDOWN,
        &options("cassette-md-fenced-json"),
    )
    .await
    .unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].spec.path, "/ping");
    assert!((drafts[0].confidence - 0.88).abs() < f64::EPSILON);
}

#[test]
fn persist_drafts_stays_in_queue_not_active() {
    let clock = FixedClock::new(Timestamp::from_unix_millis(1_700_000_000_000));
    let db = Db::open_in_memory(&clock).unwrap();
    let repo = ApiSpecRepository::new(db.conn(), &clock);

    let drafts = serde_json::from_str::<Vec<importer::ApiSpecDraft>>(SAMPLE_DRAFTS_JSON).unwrap();
    let saved = persist_api_spec_drafts(&repo, &drafts, "markdown-import").unwrap();
    assert_eq!(saved.len(), 2);
    for record in &saved {
        assert_eq!(record.info.status, ApiSpecStatus::Draft);
        assert_eq!(record.info.created_by, "markdown-import");
    }
    // 草稿不得进入 active。
    assert!(repo.get_active("ping.get").unwrap().is_none());
    assert!(repo.get_active("orders.post").unwrap().is_none());
    let versions = repo.list_versions("ping.get").unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].status, ApiSpecStatus::Draft);
}

#[test]
#[ignore = "authoring helper: run once to refresh cassettes under tests/fixtures/cassettes"]
fn write_markdown_import_cassettes() {
    let scenarios: &[(&str, &str)] = &[
        ("cassette-md-plain-json", SAMPLE_DRAFTS_JSON),
        ("cassette-md-fenced-json", FENCED_DRAFTS),
    ];

    for (model, body) in scenarios {
        let request = build_markdown_import_request(SAMPLE_MARKDOWN, &options(model));
        let response = ChatResponse {
            content: body.to_string(),
            model: (*model).into(),
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            },
            finish_reason: Some("stop".into()),
        };
        let path = CassetteProvider::write_cassette(fixtures_dir(), &request, &response).unwrap();
        eprintln!("wrote {}", path.display());
    }

    // Sanity: direct request matches hand-built for documentation.
    let _sample = ChatRequest {
        model: "cassette-md-plain-json".into(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: build_markdown_import_request(
                SAMPLE_MARKDOWN,
                &options("cassette-md-plain-json"),
            )
            .messages[0]
                .content
                .clone(),
        }],
    };
}
