//! Rule-based failure classification. Never calls an LLM.

pub mod category;

pub use category::{catalog_markdown, ErrorCode, FailureCategory, ParseError};
