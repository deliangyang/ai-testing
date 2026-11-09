//! Deterministic normalization of HTTP API definitions prior to persistence.

pub mod api_spec;
pub mod curl;
pub mod har;
pub mod heuristic;
pub mod markdown_cassette;
pub mod markdown_draft;
pub mod markdown_prompt;
pub mod merge;
pub mod openapi;
pub mod pipeline;
pub mod postman;
pub mod preview;

#[cfg(feature = "markdown")]
pub mod markdown;

pub use api_spec::{
    api_spec_schema, api_spec_schema_json, normalize_api_spec, ApiParam, ApiSpec, ParamLocation,
};
pub use curl::{parse_curl_command, CurlParseError};
pub use har::{parse_har, parse_har_str, HarParseError};
#[cfg(feature = "markdown")]
pub use markdown::{
    build_markdown_import_request, extract_api_spec_drafts_from_markdown, persist_api_spec_drafts,
    MarkdownImportError, MarkdownImportRequestOptions, MARKDOWN_IMPORT_PURPOSE,
};
pub use markdown_cassette::{
    build_markdown_cassette_request, extract_api_spec_drafts_from_markdown_cassette,
    MarkdownCassetteError, MARKDOWN_IMPORT_PURPOSE as MARKDOWN_CASSETTE_PURPOSE,
};
pub use markdown_draft::{
    parse_api_spec_drafts_from_llm_content, ApiSpecDraft, MarkdownDraftParseError,
};
pub use merge::{
    apply_merge_strategy, diff_api_specs, ApiSpecChange, ApiSpecDiff, MergeError, MergeStrategy,
};
pub use openapi::{
    parse_openapi, parse_openapi_json, parse_openapi_value, parse_openapi_yaml, resolve_value,
    OpenApiImportError,
};
pub use pipeline::{
    parse_import_file, parse_import_text, persist_imported_drafts, persist_imported_specs,
    ImportFormat, ImportPersistSummary, ImportPipelineError,
};
pub use postman::{
    api_specs_canonical_lines, parse_postman_collection, parse_postman_collection_str,
    PostmanParseError,
};
pub use preview::{
    build_import_preview, persist_confirmed_import, ImportCommitItem, ImportMergeAction,
    ImportPreviewItem, ImportPreviewResponse, ImportPreviewSummary,
};
