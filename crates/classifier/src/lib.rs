//! Rule-based failure classification. Never calls an LLM.

pub mod category;
pub mod rules;

pub use category::{catalog_markdown, ErrorCode, FailureCategory, ParseError};
pub use rules::{Classification, Classifier, ErrorKind, FailedCheck, FailureInput, RuleError};
