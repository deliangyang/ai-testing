//! Deterministic DAG execution engine.

pub mod assert;
pub mod cancel;
pub mod classify;
pub mod cleanup;
pub mod condition;
pub mod environment;
pub mod events;
pub mod executor;
pub mod http;
pub mod jsonpath;
pub mod poll;
pub mod record;
pub mod render;
pub mod scheduler;
pub mod state;
pub mod subworkflow;

pub use assert::AssertExecutor;
pub use cancel::CancelToken;
pub use classify::{failure_input, RuleClassifier};
pub use cleanup::CleanupExecutor;
pub use condition::{evaluate, ConditionResult};
pub use environment::{EnvError, Environment, ProcessEnv, SystemEnv};
pub use events::{channel as event_channel, EngineEvent, EventSink, LogLevel};
pub use executor::{
    ExecContext, ExecError, ExecErrorKind, ExecFuture, ExecOutput, Frame, NodeExecutor,
};
pub use http::{HttpClientError, HttpExecutor};
pub use poll::PollExecutor;
pub use record::{
    load_results, FailureClassifier, NodeRecord, RecordError, RunRecorder, StoreRecorder,
};
pub use render::{RenderError, RenderFailure, VarContext};
pub use scheduler::{NodeResult, RunOutcome, Scheduler, SchedulerError, INPUT_NODE_ID};
pub use state::{InvalidTransition, NodeStatus};
pub use subworkflow::{DagSource, StoreDagSource, SubworkflowExecutor};
