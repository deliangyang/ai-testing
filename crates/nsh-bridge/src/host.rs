//! Process-wide host for NSH `#[module]` DataSource (embedded API + sync reads).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use server::{DagDetailDto, DagVersionDto, GateResponse, NodeExecutionDto, RunDto};
use store::RunQuery;

use crate::embed::EmbeddedApi;
use crate::read;
use crate::BridgeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub db_path: PathBuf,
    pub env_root: PathBuf,
}

impl Default for HostConfig {
    fn default() -> Self {
        let env_root = std::env::var("AITEST_ENV_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let db_path = std::env::var("AITEST_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env_root.join("aitest.db"));
        Self { db_path, env_root }
    }
}

struct HostInner {
    api: EmbeddedApi,
}

static HOST: OnceLock<Mutex<Option<HostInner>>> = OnceLock::new();

fn host_slot() -> &'static Mutex<Option<HostInner>> {
    HOST.get_or_init(|| Mutex::new(None))
}

fn with_host<T>(f: impl FnOnce(&HostInner) -> Result<T, BridgeError>) -> Result<T, BridgeError> {
    let guard = host_slot()
        .lock()
        .map_err(|_| store::StoreError::Invalid("host lock poisoned".into()))?;
    let inner = guard.as_ref().ok_or_else(|| {
        store::StoreError::Invalid("call ensure_api() before other DataSource ops".into())
    })?;
    f(inner)
}

/// Start (once) the embedded `server` and return its base URL for `HttpDataSource` / WebView.
pub fn ensure_api(config: HostConfig) -> Result<String, BridgeError> {
    let mut guard = host_slot()
        .lock()
        .map_err(|_| store::StoreError::Invalid("host lock poisoned".into()))?;
    if let Some(existing) = guard.as_ref() {
        return Ok(existing.api.base_url.clone());
    }
    let api = EmbeddedApi::spawn_ephemeral(config.db_path, config.env_root)?;
    let url = api.base_url.clone();
    *guard = Some(HostInner { api });
    Ok(url)
}

/// Same as [`ensure_api`] with [`HostConfig::default`].
pub fn ensure_default_api() -> Result<String, BridgeError> {
    ensure_api(HostConfig::default())
}

pub fn list_dags() -> Result<Vec<DagVersionDto>, BridgeError> {
    with_host(|h| read::list_dags(&h.api.state))
}

pub fn get_active_dag(dag_id: String) -> Result<DagDetailDto, BridgeError> {
    with_host(|h| read::get_active_dag(&h.api.state, &dag_id))
}

pub fn get_dag_version(dag_id: String, version: u32) -> Result<DagDetailDto, BridgeError> {
    with_host(|h| read::get_dag_version(&h.api.state, &dag_id, version))
}

pub fn list_runs(dag_id: Option<String>) -> Result<Vec<RunDto>, BridgeError> {
    with_host(|h| {
        read::list_runs(
            &h.api.state,
            &RunQuery {
                dag_id,
                status: None,
                limit: None,
            },
        )
    })
}

pub fn get_run(run_id: String) -> Result<RunDto, BridgeError> {
    with_host(|h| read::get_run(&h.api.state, &run_id))
}

pub fn list_executions(run_id: String) -> Result<Vec<NodeExecutionDto>, BridgeError> {
    with_host(|h| read::list_executions(&h.api.state, &run_id))
}

pub fn validate_dag_json(dag_json: String) -> Result<GateResponse, BridgeError> {
    let dag: model::Dag =
        serde_json::from_str(&dag_json).map_err(|e| store::StoreError::Invalid(e.to_string()))?;
    Ok(read::validate_dag(&dag))
}

#[doc(hidden)]
pub fn reset_host_for_tests() {
    if let Ok(mut guard) = host_slot().lock() {
        *guard = None;
    }
}
