//! Components and modules registered from `lib.rs` (`feature = "nsh"`).

use nsh_bridge::datasource::{self, HostConfig};
use nsh_core::prelude::*;

pub fn register(registry: &mut Registry) {
    registry.module::<DataSource>();
    registry.register::<WebUiHost>();
}

#[derive(Clone, PartialEq, Props)]
pub struct WebUiHostProps {
    pub src: Prop<SharedString>,
}

/// Full-screen WebView loading the Vite `web/dist` bundle with `?api_base=` injected.
#[component]
pub fn WebUiHost(props: WebUiHostProps) -> impl IntoElement {
    web_view(move || props.src.get().to_string()).flex_1()
}

/// Rust `DataSource` surface for TSX (`import { DataSource } from "aitest/native"`).
#[module]
pub struct DataSource;

#[module]
impl DataSource {
    /// Starts the embedded SQLite API (once) and returns the HTTP base URL.
    pub fn ensure_api() -> Result<String, String> {
        datasource::ensure_default_api().map_err(|e| e.to_string())
    }

    /// File URL or `http` URL for the built web bundle (set `AITEST_WEB_DIST`).
    pub fn web_shell_url() -> Result<String, String> {
        let dist = std::env::var("AITEST_WEB_DIST").unwrap_or_else(|_| {
            format!(
                "{}/web/dist/index.html",
                std::env::var("AITEST_REPO_ROOT").unwrap_or_else(|_| ".".into())
            )
        });
        let base = datasource::ensure_default_api().map_err(|e| e.to_string())?;
        Ok(format!("{dist}?api_base={base}"))
    }

    pub fn list_dags() -> Result<String, String> {
        let rows = datasource::list_dags().map_err(|e| e.to_string())?;
        serde_json::to_string(&rows).map_err(|e| e.to_string())
    }

    pub fn configure(db_path: String, env_root: String) -> Result<String, String> {
        datasource::ensure_api(HostConfig {
            db_path: db_path.into(),
            env_root: env_root.into(),
        })
        .map_err(|e| e.to_string())
    }
}
