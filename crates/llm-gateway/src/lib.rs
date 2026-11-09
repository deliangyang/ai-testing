//! LLM gateway: provider trait, OpenAI-compatible HTTP client, cassette mock replay.

mod cassette;
mod error;
mod logging;
mod openai;
mod provider;
mod redact;
mod request_key;
mod types;

pub use cassette::{CassetteFile, CassetteProvider};
pub use error::LlmError;
pub use logging::{
    DbLlmCallLogger, LlmCallContext, LlmCallLogger, LoggingLlmProvider, RecordingLlmCallLogger,
};
pub use openai::{OpenAiClient, OpenAiConfig};
pub use provider::{ChatFuture, LlmProvider};
pub use redact::{redact_chat_request, redact_text, REDACTED};
pub use request_key::chat_request_key;
pub use types::{ChatMessage, ChatRequest, ChatResponse, MessageRole, TokenUsage};
