use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

/// Read-only view of stored API specs, implemented over the store by callers that save DAGs.
pub trait ApiSpecLookup: Send + Sync {
    /// JSON Schema of the response body for the active version of `id`, if known.
    fn response_schema(&self, id: &str) -> Option<Value>;
}

/// In-memory lookup for unit tests and offline validation.
#[derive(Debug, Clone, Default)]
pub struct MapApiSpecLookup {
    active: BTreeMap<String, Option<Value>>,
}

impl MapApiSpecLookup {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_active(mut self, id: impl Into<String>, response_schema: Option<Value>) -> Self {
        self.active.insert(id.into(), response_schema);
        self
    }
}

impl ApiSpecLookup for MapApiSpecLookup {
    fn response_schema(&self, id: &str) -> Option<Value> {
        self.active.get(id).and_then(|s| s.clone())
    }
}

/// Shared lookup handle used when building save-time gates.
pub type ApiSpecLookupHandle = Arc<dyn ApiSpecLookup>;
