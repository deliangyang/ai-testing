//! Named DB connections and environment bindings (`db_connections`, `environment_connections`).

use std::fmt;
use std::str::FromStr;

use model::to_canonical_json;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionType {
    Postgres,
    Mysql,
    Sqlite,
}

impl ConnectionType {
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionType::Postgres => "postgres",
            ConnectionType::Mysql => "mysql",
            ConnectionType::Sqlite => "sqlite",
        }
    }
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ConnectionType {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "postgres" => Ok(ConnectionType::Postgres),
            "mysql" => Ok(ConnectionType::Mysql),
            "sqlite" => Ok(ConnectionType::Sqlite),
            other => Err(StoreError::Corrupt(format!(
                "unknown connection type '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionStatus {
    Draft,
    Active,
    Archived,
}

impl ConnectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionStatus::Draft => "draft",
            ConnectionStatus::Active => "active",
            ConnectionStatus::Archived => "archived",
        }
    }
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ConnectionStatus {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "draft" => Ok(ConnectionStatus::Draft),
            "active" => Ok(ConnectionStatus::Active),
            "archived" => Ok(ConnectionStatus::Archived),
            other => Err(StoreError::Corrupt(format!(
                "unknown connection status '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbConnectionSaveInput {
    pub name: String,
    pub connection_type: ConnectionType,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub schema_name: Option<String>,
    pub auth_ref: Option<String>,
    pub read_only: bool,
    pub pool_config: Option<Value>,
    pub status: ConnectionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbConnectionRecord {
    pub connection: DbConnectionSaveInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentConnectionBinding {
    pub env_name: String,
    pub logical_name: String,
    pub connection_name: String,
}

pub struct ConnectionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ConnectionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn save(&self, input: &DbConnectionSaveInput) -> Result<DbConnectionRecord> {
        validate_connection(input)?;
        let pool_json = json_column(input.pool_config.as_ref())?;
        self.conn.execute(
            "INSERT INTO db_connections (
                name, type, host, port, database_name, schema_name,
                auth_ref, read_only, pool_config_json, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(name) DO UPDATE SET
                type = excluded.type,
                host = excluded.host,
                port = excluded.port,
                database_name = excluded.database_name,
                schema_name = excluded.schema_name,
                auth_ref = excluded.auth_ref,
                read_only = excluded.read_only,
                pool_config_json = excluded.pool_config_json,
                status = excluded.status",
            params![
                input.name,
                input.connection_type.as_str(),
                input.host,
                input.port.map(i64::from),
                input.database_name,
                input.schema_name,
                input.auth_ref,
                i64::from(input.read_only),
                pool_json,
                input.status.as_str(),
            ],
        )?;
        self.get(&input.name)?
            .ok_or_else(|| StoreError::Corrupt("connection missing immediately after save".into()))
    }

    pub fn get(&self, name: &str) -> Result<Option<DbConnectionRecord>> {
        self.conn
            .query_row(
                "SELECT name, type, host, port, database_name, schema_name,
                        auth_ref, read_only, pool_config_json, status
                 FROM db_connections WHERE name = ?1",
                [name],
                map_connection,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list(&self) -> Result<Vec<DbConnectionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, type, host, port, database_name, schema_name,
                    auth_ref, read_only, pool_config_json, status
             FROM db_connections ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], map_connection)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn bind_environment(
        &self,
        env_name: &str,
        logical_name: &str,
        connection_name: &str,
    ) -> Result<EnvironmentConnectionBinding> {
        validate_env_name(env_name)?;
        validate_logical_name(logical_name)?;
        if self.get(connection_name)?.is_none() {
            return Err(StoreError::NotFound(format!(
                "connection '{connection_name}'"
            )));
        }
        self.conn.execute(
            "INSERT INTO environment_connections (env_name, logical_name, connection_name)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(env_name, logical_name) DO UPDATE SET
                connection_name = excluded.connection_name",
            params![env_name, logical_name, connection_name],
        )?;
        Ok(EnvironmentConnectionBinding {
            env_name: env_name.to_owned(),
            logical_name: logical_name.to_owned(),
            connection_name: connection_name.to_owned(),
        })
    }

    pub fn resolve_for_environment(
        &self,
        env_name: &str,
        logical_name: &str,
    ) -> Result<Option<DbConnectionRecord>> {
        let bound: Option<String> = self
            .conn
            .query_row(
                "SELECT connection_name FROM environment_connections
                 WHERE env_name = ?1 AND logical_name = ?2",
                params![env_name, logical_name],
                |r| r.get(0),
            )
            .optional()?;
        let Some(connection_name) = bound else {
            return Ok(None);
        };
        self.get(&connection_name)
    }

    pub fn list_bindings_for_environment(
        &self,
        env_name: &str,
    ) -> Result<Vec<EnvironmentConnectionBinding>> {
        let mut stmt = self.conn.prepare(
            "SELECT env_name, logical_name, connection_name
             FROM environment_connections
             WHERE env_name = ?1
             ORDER BY logical_name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([env_name], map_binding)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn validate_connection(input: &DbConnectionSaveInput) -> Result<()> {
    if input.name.trim().is_empty() {
        return Err(StoreError::Invalid(
            "connection name must not be empty".into(),
        ));
    }
    secrets::validate_credential_ref(input.auth_ref.as_deref())
        .map_err(|e| StoreError::Invalid(e.to_string()))?;
    Ok(())
}

fn validate_env_name(env_name: &str) -> Result<()> {
    if env_name.is_empty() {
        return Err(StoreError::Invalid("env_name must not be empty".into()));
    }
    if !env_name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(StoreError::Invalid(
            "env_name may only contain letters, digits, '_', and '-'".into(),
        ));
    }
    Ok(())
}

fn validate_logical_name(logical_name: &str) -> Result<()> {
    if logical_name.trim().is_empty() {
        return Err(StoreError::Invalid("logical_name must not be empty".into()));
    }
    Ok(())
}

fn json_column(value: Option<&Value>) -> Result<Option<String>> {
    value
        .map(to_canonical_json)
        .transpose()
        .map_err(|e| StoreError::Corrupt(e.to_string()))
}

fn map_connection(row: &Row<'_>) -> rusqlite::Result<DbConnectionRecord> {
    let read_only: i64 = row.get(7)?;
    let pool_raw: Option<String> = row.get(8)?;
    let pool_config = pool_raw
        .as_deref()
        .map(|s| {
            serde_json::from_str(s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(StoreError::Corrupt(format!(
                        "invalid pool_config_json: {e}"
                    ))),
                )
            })
        })
        .transpose()?;
    let status: String = row.get(9)?;
    let connection_type: String = row.get(1)?;
    Ok(DbConnectionRecord {
        connection: DbConnectionSaveInput {
            name: row.get(0)?,
            connection_type: connection_type.parse().map_err(|e: StoreError| {
                rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, e.into())
            })?,
            host: row.get(2)?,
            port: row.get::<_, Option<i64>>(3)?.map(|p| p as u16),
            database_name: row.get(4)?,
            schema_name: row.get(5)?,
            auth_ref: row.get(6)?,
            read_only: read_only != 0,
            pool_config,
            status: status.parse().map_err(|e: StoreError| {
                rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, e.into())
            })?,
        },
    })
}

fn map_binding(row: &Row<'_>) -> rusqlite::Result<EnvironmentConnectionBinding> {
    Ok(EnvironmentConnectionBinding {
        env_name: row.get(0)?,
        logical_name: row.get(1)?,
        connection_name: row.get(2)?,
    })
}
