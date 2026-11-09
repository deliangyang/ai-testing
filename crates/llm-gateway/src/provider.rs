use std::future::Future;
use std::pin::Pin;

use crate::error::LlmError;
use crate::types::{ChatRequest, ChatResponse};

pub type ChatFuture = Pin<Box<dyn Future<Output = Result<ChatResponse, LlmError>> + Send>>;

pub trait LlmProvider: Send + Sync {
    fn chat(&self, request: ChatRequest) -> ChatFuture;
}
