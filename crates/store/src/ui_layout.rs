//! Persisted shell layout and workspace presets (develop.md §5.4).

use common::clock::{Clock, Timestamp};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLayoutRecord {
    pub scope_key: String,
    pub layout: Value,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePresetRecord {
    pub id: String,
    pub name: String,
    pub layout: Value,
    pub updated_at: Timestamp,
}

pub struct UiLayoutRepository<'a> {
    conn: &'a Connection,
    clock: &'a dyn Clock,
}

impl<'a> UiLayoutRepository<'a> {
    pub fn new(conn: &'a Connection, clock: &'a dyn Clock) -> Self {
        Self { conn, clock }
    }

    pub fn get(&self, scope_key: &str) -> Result<Option<UiLayoutRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT scope_key, layout_json, updated_at FROM ui_layouts WHERE scope_key = ?1",
        )?;
        stmt.query_row(params![scope_key], map_layout)
            .optional()
            .map_err(StoreError::from)
    }

    pub fn upsert(&self, scope_key: &str, layout: &Value) -> Result<UiLayoutRecord> {
        let json = serde_json::to_string(layout).map_err(|e| StoreError::Corrupt(e.to_string()))?;
        let now = self.clock.now();
        self.conn.execute(
            "INSERT INTO ui_layouts (scope_key, layout_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(scope_key) DO UPDATE SET
               layout_json = excluded.layout_json,
               updated_at = excluded.updated_at",
            params![scope_key, json, now.to_string()],
        )?;
        Ok(UiLayoutRecord {
            scope_key: scope_key.to_owned(),
            layout: layout.clone(),
            updated_at: now,
        })
    }

    pub fn list_presets(&self) -> Result<Vec<WorkspacePresetRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, layout_json, updated_at FROM workspace_presets ORDER BY id")?;
        let rows = stmt.query_map([], map_preset)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_preset(&self, id: &str) -> Result<Option<WorkspacePresetRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, layout_json, updated_at FROM workspace_presets WHERE id = ?1",
        )?;
        stmt.query_row(params![id], map_preset)
            .optional()
            .map_err(StoreError::from)
    }
}

fn decode_layout(scope_key: String, json: String, updated_at: String) -> Result<UiLayoutRecord> {
    let layout: Value = serde_json::from_str(&json).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let updated_at: Timestamp = updated_at
        .parse()
        .map_err(|e| StoreError::Corrupt(format!("ui_layout.updated_at: {e}")))?;
    Ok(UiLayoutRecord {
        scope_key,
        layout,
        updated_at,
    })
}

fn map_layout(row: &Row<'_>) -> rusqlite::Result<UiLayoutRecord> {
    decode_layout(row.get(0)?, row.get(1)?, row.get(2)?).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

fn decode_preset(id: String, name: String, json: String, updated_at: String) -> Result<WorkspacePresetRecord> {
    let layout: Value = serde_json::from_str(&json).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let updated_at: Timestamp = updated_at
        .parse()
        .map_err(|e| StoreError::Corrupt(format!("workspace_presets.updated_at: {e}")))?;
    Ok(WorkspacePresetRecord {
        id,
        name,
        layout,
        updated_at,
    })
}

fn map_preset(row: &Row<'_>) -> rusqlite::Result<WorkspacePresetRecord> {
    decode_preset(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}
