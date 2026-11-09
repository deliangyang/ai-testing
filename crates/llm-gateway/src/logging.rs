//! Provider wrapper that persists per-call metadata after each LLM invocation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use common::clock::Clock;
use common::id::IdGen;
use store::{
    Db, LlmCallLogRepository, LlmCallOutcome, NewLlmCallLog as StoreNewLlmCallLog, StoreError,
};

use crate::error::LlmError;
use crate::provider::{ChatFuture, LlmProvider};
use crate::types::{ChatRequest, ChatResponse};

/// Caller-supplied correlation fields; never includes prompt content or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCallContext {
    pub run_id: Option<String>,
    pub node_id: Option<String>,
    pub purpose: String,
}

impl LlmCallContext {
    #[must_use]
    pub fn new(purpose: impl Into<String>) -> Self {
        Self {
            run_id: None,
            node_id: None,
            purpose: purpose.into(),
        }
    }

    #[must_use]
    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    #[must_use]
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }
}

/// Persists [`StoreNewLlmCallLog`] rows without exposing prompts or credentials.
pub trait LlmCallLogger: Send + Sync {
    fn record(&self, entry: StoreNewLlmCallLog) -> Result<(), StoreError>;
}

/// Opens the on-disk SQLite file for each record so logging works from async tasks without
/// sharing a `Connection` across threads.
#[derive(Clone)]
pub struct DbLlmCallLogger {
    db_path: Arc<PathBuf>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGen>,
}

impl DbLlmCallLogger {
    pub fn new(db_path: impl Into<PathBuf>, clock: Arc<dyn Clock>, ids: Arc<dyn IdGen>) -> Self {
        Self {
            db_path: Arc::new(db_path.into()),
            clock,
            ids,
        }
    }
}

impl LlmCallLogger for DbLlmCallLogger {
    fn record(&self, entry: StoreNewLlmCallLog) -> Result<(), StoreError> {
        let db = Db::open(&*self.db_path, self.clock.as_ref())?;
        LlmCallLogRepository::new(db.conn(), self.ids.as_ref()).insert(&entry)?;
        Ok(())
    }
}

/// In-memory recorder for unit tests.
#[derive(Default)]
pub struct RecordingLlmCallLogger {
    entries: std::sync::Mutex<Vec<StoreNewLlmCallLog>>,
}

impl RecordingLlmCallLogger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take(&self) -> Vec<StoreNewLlmCallLog> {
        let mut guard = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *guard)
    }
}

impl LlmCallLogger for RecordingLlmCallLogger {
    fn record(&self, entry: StoreNewLlmCallLog) -> Result<(), StoreError> {
        let mut guard = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.push(entry);
        Ok(())
    }
}

/// Delegates chat to an inner [`LlmProvider`] and writes audit metadata afterward.
pub struct LoggingLlmProvider<P> {
    inner: Arc<P>,
    logger: Arc<dyn LlmCallLogger>,
}

impl<P: LlmProvider + Send + Sync + 'static> LoggingLlmProvider<P> {
    pub fn new(inner: P, logger: Arc<dyn LlmCallLogger>) -> Self {
        Self {
            inner: Arc::new(inner),
            logger,
        }
    }

    pub fn chat_with_context(&self, ctx: LlmCallContext, request: ChatRequest) -> ChatFuture {
        let start = Instant::now();
        let inner = Arc::clone(&self.inner);
        let logger = Arc::clone(&self.logger);
        let purpose = ctx.purpose;
        let run_id = ctx.run_id;
        let node_id = ctx.node_id;
        Box::pin(async move {
            let result = LlmProvider::chat(inner.as_ref(), request).await;
            persist_after_call(logger.as_ref(), run_id, node_id, purpose, start, &result)?;
            result
        })
    }
}

impl<P: LlmProvider + Send + Sync + 'static> LlmProvider for LoggingLlmProvider<P> {
    fn chat(&self, request: ChatRequest) -> ChatFuture {
        self.chat_with_context(LlmCallContext::new("unspecified"), request)
    }
}

fn persist_after_call(
    logger: &dyn LlmCallLogger,
    run_id: Option<String>,
    node_id: Option<String>,
    purpose: String,
    start: Instant,
    result: &Result<ChatResponse, LlmError>,
) -> Result<(), LlmError> {
    let latency_ms = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
    let (outcome, tokens_in, tokens_out) = match result {
        Ok(response) => (
            LlmCallOutcome::Success,
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
        ),
        Err(_) => (LlmCallOutcome::Error, 0, 0),
    };
    let log = StoreNewLlmCallLog {
        run_id,
        node_id,
        purpose,
        tokens_in,
        tokens_out,
        cost: None,
        latency_ms,
        outcome,
    };
    logger
        .record(log)
        .map_err(|e| LlmError::LogPersist(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::{ChatMessage, MessageRole, TokenUsage};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubProvider {
        fails: bool,
        calls: AtomicUsize,
    }

    impl LlmProvider for StubProvider {
        fn chat(&self, _request: ChatRequest) -> ChatFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let fails = self.fails;
            Box::pin(async move {
                if fails {
                    Err(LlmError::Transport("down".into()))
                } else {
                    Ok(ChatResponse {
                        content: "ok".into(),
                        model: "m".into(),
                        usage: TokenUsage {
                            prompt_tokens: 3,
                            completion_tokens: 2,
                            total_tokens: 5,
                        },
                        finish_reason: None,
                    })
                }
            })
        }
    }

    #[tokio::test]
    async fn logs_success_and_error_outcomes() {
        let recorder = Arc::new(RecordingLlmCallLogger::new());
        let logger: Arc<dyn LlmCallLogger> = Arc::clone(&recorder) as Arc<dyn LlmCallLogger>;
        let ok = LoggingLlmProvider::new(
            StubProvider {
                fails: false,
                calls: AtomicUsize::new(0),
            },
            Arc::clone(&logger),
        );
        let response = ok
            .chat_with_context(
                LlmCallContext::new("repair").with_run_id("r1"),
                ChatRequest {
                    model: "m".into(),
                    messages: vec![ChatMessage {
                        role: MessageRole::User,
                        content: "hi".into(),
                    }],
                },
            )
            .await
            .unwrap();
        assert_eq!(response.content, "ok");
        let entries = recorder.take();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].purpose, "repair");
        assert_eq!(entries[0].run_id.as_deref(), Some("r1"));
        assert_eq!(entries[0].tokens_in, 3);
        assert_eq!(entries[0].outcome, LlmCallOutcome::Success);

        let err = LoggingLlmProvider::new(
            StubProvider {
                fails: true,
                calls: AtomicUsize::new(0),
            },
            logger,
        );
        assert!(err
            .chat_with_context(
                LlmCallContext::new("repair"),
                ChatRequest {
                    model: "m".into(),
                    messages: vec![],
                },
            )
            .await
            .is_err());
        let entries = recorder.take();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, LlmCallOutcome::Error);
        assert_eq!(entries[0].tokens_in, 0);
    }
}
