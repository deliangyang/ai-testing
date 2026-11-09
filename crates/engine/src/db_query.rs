//! `db_query` nodes: parameterized SQL via rusqlite (SQLite) or sqlx (Postgres/MySQL).

use std::sync::Arc;

use model::nodes::QueryResultMode;
use model::{Node, NodeKind};
use rusqlite::{types::ValueRef, Connection, Row};
use serde_json::{json, Map, Value};
use store::{ConnectionType, DbConnectionRecord};

use crate::connections::ConnectionResolver;
use crate::executor::{ExecContext, ExecError, ExecErrorKind, ExecFuture, NodeExecutor};
use crate::render::RenderError;

pub struct DbQueryExecutor {
    resolver: Arc<dyn ConnectionResolver>,
}

impl DbQueryExecutor {
    pub fn new(resolver: Arc<dyn ConnectionResolver>) -> Self {
        Self { resolver }
    }
}

impl NodeExecutor for DbQueryExecutor {
    fn execute<'a>(&'a self, node: &'a Node, ctx: ExecContext) -> ExecFuture<'a> {
        Box::pin(async move {
            let NodeKind::DbQuery(cfg) = &node.kind else {
                return Err(ExecError::new(
                    ExecErrorKind::Unsupported,
                    format!(
                        "db_query executor cannot run '{}' nodes",
                        node.kind.type_name()
                    ),
                ));
            };

            let vars = ctx.vars();
            let mut params = Vec::with_capacity(cfg.params.len());
            for (i, param) in cfg.params.iter().enumerate() {
                params.push(vars.render_value(param).map_err(|mut e: RenderError| {
                    e.pointer = format!("/params/{i}{}", e.pointer);
                    ExecError::from(e)
                })?);
            }

            let env_name = ctx.env.name().to_owned();
            let logical = cfg.connection.clone();
            let logical_resolve = logical.clone();
            let resolver = Arc::clone(&self.resolver);
            let record =
                tokio::task::spawn_blocking(move || resolver.resolve(&env_name, &logical_resolve))
                    .await
                    .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?
                    .map_err(|message| ExecError::new(ExecErrorKind::Internal, message))?;

            if record.connection.read_only {
                match sql_statement_kind(&cfg.sql) {
                    SqlStatementKind::Write | SqlStatementKind::Unknown => {
                        return Err(ExecError::new(
                            ExecErrorKind::Internal,
                            format!(
                                "read-only connection '{logical}' rejects SQL on node '{}'",
                                node.id
                            ),
                        )
                        .with_detail(json!({
                            "reason": "read_only_violation",
                            "connection": logical,
                        })));
                    }
                    SqlStatementKind::Read => {}
                }
            }

            let sql = cfg.sql.clone();
            let result_mode = cfg.result;
            let node_id = node.id.clone();
            let mapped = tokio::task::spawn_blocking(move || {
                run_query(&record, &sql, &params, record.connection.read_only)
            })
            .await
            .map_err(|e| ExecError::new(ExecErrorKind::Internal, e.to_string()))?
            .map_err(|e| match e {
                QueryRunError::Transport(message) => {
                    ExecError::new(ExecErrorKind::Transport, message)
                }
                QueryRunError::Internal(message) => {
                    ExecError::new(ExecErrorKind::Internal, message)
                }
            })?;

            let output = match result_mode {
                QueryResultMode::Rows => json!({ "rows": mapped }),
                QueryResultMode::FirstRow => {
                    let Some(row) = mapped.into_iter().next() else {
                        return Err(ExecError::new(
                            ExecErrorKind::Extraction,
                            format!("db_query node '{node_id}' expected one row, got none"),
                        ));
                    };
                    json!({ "row": row })
                }
                QueryResultMode::Scalar => {
                    let Some(row) = mapped.into_iter().next() else {
                        return Err(ExecError::new(
                            ExecErrorKind::Extraction,
                            format!("db_query node '{node_id}' expected a scalar, got no rows"),
                        ));
                    };
                    json!({ "value": scalar_from_object(&row)? })
                }
            };

            Ok(output.into())
        })
    }
}

enum QueryRunError {
    Transport(String),
    Internal(String),
}

fn run_query(
    record: &DbConnectionRecord,
    sql: &str,
    params: &[Value],
    read_only: bool,
) -> Result<Vec<Value>, QueryRunError> {
    match record.connection.connection_type {
        ConnectionType::Sqlite => sqlite_query(record, sql, params, read_only),
        ConnectionType::Postgres => {
            #[cfg(feature = "postgres")]
            {
                return postgres_query(record, sql, params, read_only);
            }
            #[cfg(not(feature = "postgres"))]
            {
                let _ = (record, sql, params, read_only);
                Err(QueryRunError::Internal(
                    "postgres support is disabled; rebuild with feature `postgres`".into(),
                ))
            }
        }
        ConnectionType::Mysql => {
            #[cfg(feature = "mysql")]
            {
                return mysql_query(record, sql, params, read_only);
            }
            #[cfg(not(feature = "mysql"))]
            {
                let _ = (record, sql, params, read_only);
                Err(QueryRunError::Internal(
                    "mysql support is disabled; rebuild with feature `mysql`".into(),
                ))
            }
        }
    }
}

fn sqlite_query(
    record: &DbConnectionRecord,
    sql: &str,
    params: &[Value],
    read_only: bool,
) -> Result<Vec<Value>, QueryRunError> {
    let path = sqlite_path(record)?;
    let open_flags = if read_only {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
    };
    let conn = Connection::open_with_flags(&path, open_flags)
        .map_err(|e| QueryRunError::Transport(e.to_string()))?;
    if read_only {
        conn.pragma_update(None, "query_only", "ON")
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
    }
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| QueryRunError::Transport(e.to_string()))?;
    let rusqlite_params = json_params(params);
    let column_names: Vec<String> = stmt.column_names().map(str::to_owned).collect();
    let rows = stmt
        .query_map(rusqlite::params_from_iter(rusqlite_params.iter()), |row| {
            row_to_json_object(row, &column_names)
        })
        .map_err(|e| QueryRunError::Transport(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| QueryRunError::Transport(e.to_string()))?;
    Ok(rows.into_iter().map(Value::Object).collect())
}

#[cfg(feature = "postgres")]
fn postgres_query(
    record: &DbConnectionRecord,
    sql: &str,
    params: &[Value],
    read_only: bool,
) -> Result<Vec<Value>, QueryRunError> {
    use sqlx::postgres::PgPoolOptions;
    use sqlx::Row;

    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let url = connection_url(record).map_err(QueryRunError::Internal)?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        if read_only {
            sqlx::query("BEGIN READ ONLY")
                .execute(&mut *conn)
                .await
                .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        }
        let mut query = sqlx::query::<sqlx::Postgres>(sql);
        for param in params {
            query = bind_sql_value(query, param);
        }
        let rows = query
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        rows.iter().map(sqlx_row_to_json).collect()
    })
}

#[cfg(feature = "mysql")]
fn mysql_query(
    record: &DbConnectionRecord,
    sql: &str,
    params: &[Value],
    read_only: bool,
) -> Result<Vec<Value>, QueryRunError> {
    use sqlx::mysql::MySqlPoolOptions;

    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let url = connection_url(record).map_err(QueryRunError::Internal)?;
        let pool = MySqlPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        if read_only {
            sqlx::query("START TRANSACTION READ ONLY")
                .execute(&mut *conn)
                .await
                .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        }
        let mut query = sqlx::query::<sqlx::MySql>(sql);
        for param in params {
            query = bind_sql_value(query, param);
        }
        let rows = query
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| QueryRunError::Transport(e.to_string()))?;
        rows.iter().map(sqlx_row_to_json).collect()
    })
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn bind_sql_value<'q, DB: sqlx::Database>(
    query: sqlx::query::Query<'q, DB, <DB as sqlx::Database>::Arguments<'q>>,
    value: &Value,
) -> sqlx::query::Query<'q, DB, <DB as sqlx::Database>::Arguments<'q>> {
    match value {
        Value::Null => query.bind(None::<String>),
        Value::Bool(b) => query.bind(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(value.to_string())
            }
        }
        Value::String(s) => query.bind(s.as_str()),
        other => query.bind(other.to_string()),
    }
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn sqlx_row_to_json<R: sqlx::Row>(row: &R) -> Result<Value, QueryRunError> {
    use sqlx::{Column, Row};
    let mut map = Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        map.insert(col.name().to_owned(), decode_sqlx_column(row, i)?);
    }
    Ok(Value::Object(map))
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn decode_sqlx_column<R: sqlx::Row>(row: &R, index: usize) -> Result<Value, QueryRunError> {
    use sqlx::Row;
    if let Ok(v) = row.try_get::<Option<i64>>(index) {
        return Ok(v.map_or(Value::Null, json));
    }
    if let Ok(v) = row.try_get::<Option<f64>>(index) {
        return Ok(v.map_or(Value::Null, json));
    }
    if let Ok(v) = row.try_get::<Option<bool>>(index) {
        return Ok(v.map_or(Value::Null, |b| json!(b)));
    }
    if let Ok(v) = row.try_get::<Option<String>>(index) {
        return Ok(v.map_or(Value::Null, json));
    }
    Ok(Value::Null)
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn connection_url(record: &DbConnectionRecord) -> Result<String, QueryRunError> {
    let c = &record.connection;
    if let Some(auth_ref) = &c.auth_ref {
        if let Ok(secret) = secrets::resolve_credential_ref(auth_ref) {
            let trimmed = secret.trim();
            if looks_like_dsn(trimmed) {
                return Ok(trimmed.to_owned());
            }
            return build_url_from_parts(record, Some(trimmed));
        }
        return Err(QueryRunError::Internal(format!(
            "cannot resolve auth_ref for connection '{}'",
            c.name
        )));
    }
    build_url_from_parts(record, None)
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn looks_like_dsn(s: &str) -> bool {
    s.starts_with("postgres://") || s.starts_with("postgresql://") || s.starts_with("mysql://")
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn build_url_from_parts(
    record: &DbConnectionRecord,
    password: Option<&str>,
) -> Result<String, QueryRunError> {
    let c = &record.connection;
    match c.connection_type {
        ConnectionType::Postgres => {
            let user = pool_string(&c.pool_config, &["username", "user"])
                .unwrap_or_else(|| "postgres".into());
            let host = c.host.as_deref().unwrap_or("localhost");
            let port = c.port.unwrap_or(5432);
            let db = c
                .database_name
                .as_deref()
                .filter(|d| !d.is_empty())
                .unwrap_or("postgres");
            let pass = password.unwrap_or("");
            Ok(format!("postgres://{user}:{pass}@{host}:{port}/{db}"))
        }
        ConnectionType::Mysql => {
            let user =
                pool_string(&c.pool_config, &["username", "user"]).unwrap_or_else(|| "root".into());
            let host = c.host.as_deref().unwrap_or("localhost");
            let port = c.port.unwrap_or(3306);
            let db = c
                .database_name
                .as_deref()
                .filter(|d| !d.is_empty())
                .unwrap_or("mysql");
            let pass = password.unwrap_or("");
            Ok(format!("mysql://{user}:{pass}@{host}:{port}/{db}"))
        }
        ConnectionType::Sqlite => Err(QueryRunError::Internal(
            "sqlite connections use database_name as a file path".into(),
        )),
    }
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn pool_string(pool: &Option<Value>, keys: &[&str]) -> Option<String> {
    let obj = pool.as_ref()?.as_object()?;
    for key in keys {
        if let Some(v) = obj.get(*key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_owned());
            }
        }
    }
    None
}

fn sqlite_path(record: &DbConnectionRecord) -> Result<String, QueryRunError> {
    let c = &record.connection;
    let path = c
        .database_name
        .as_deref()
        .filter(|p| !p.is_empty())
        .unwrap_or(":memory:");
    if path == ":memory:" || path.starts_with("file:") {
        Ok(path.to_owned())
    } else if let Some(stripped) = path.strip_prefix("sqlite://") {
        Ok(stripped.to_owned())
    } else if let Some(stripped) = path.strip_prefix("sqlite:") {
        Ok(stripped.to_owned())
    } else {
        Ok(path.to_owned())
    }
}

fn json_params(params: &[Value]) -> Vec<rusqlite::types::Value> {
    params.iter().map(json_to_sqlite).collect()
}

fn json_to_sqlite(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Text(value.to_string())
            }
        }
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        other => rusqlite::types::Value::Text(other.to_string()),
    }
}

fn row_to_json_object(row: &Row<'_>, columns: &[String]) -> rusqlite::Result<Map<String, Value>> {
    let mut map = Map::new();
    for (i, key) in columns.iter().enumerate() {
        map.insert(key.clone(), value_ref_to_json(row.get_ref(i)?));
    }
    Ok(map)
}

fn value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => json!(n),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(s) => json!(String::from_utf8_lossy(s)),
        ValueRef::Blob(b) => json!(b),
    }
}

fn scalar_from_object(row: &Value) -> Result<Value, ExecError> {
    let obj = row.as_object().ok_or_else(|| {
        ExecError::new(
            ExecErrorKind::Extraction,
            "expected object row for scalar mode",
        )
    })?;
    obj.values()
        .next()
        .cloned()
        .ok_or_else(|| ExecError::new(ExecErrorKind::Extraction, "scalar row has no columns"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlStatementKind {
    Read,
    Write,
    Unknown,
}

fn sql_statement_kind(sql: &str) -> SqlStatementKind {
    match leading_keyword(sql).as_str() {
        "SELECT" | "WITH" | "SHOW" | "EXPLAIN" | "DESCRIBE" | "DESC" | "PRAGMA" => {
            SqlStatementKind::Read
        }
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "REPLACE" | "CREATE" | "DROP" | "ALTER"
        | "TRUNCATE" | "GRANT" | "REVOKE" | "CALL" => SqlStatementKind::Write,
        _ => SqlStatementKind::Unknown,
    }
}

fn leading_keyword(sql: &str) -> String {
    let mut chars = sql.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '-' && chars.clone().nth(1) == Some('-') {
            while chars.next().is_some_and(|ch| ch != '\n') {}
            continue;
        }
        if c == '/' && chars.clone().nth(1) == Some('*') {
            chars.next();
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            continue;
        }
        break;
    }
    let mut word = String::new();
    for c in chars {
        if c.is_ascii_alphabetic() || c == '_' {
            word.push(c.to_ascii_uppercase());
        } else {
            break;
        }
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_statements_are_detected() {
        assert_eq!(
            sql_statement_kind("  -- comment\nUPDATE users SET x = 1"),
            SqlStatementKind::Write
        );
    }
}
