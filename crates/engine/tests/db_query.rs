#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{FixedClock, Timestamp, TokioSleeper};
use engine::{
    ConnectionResolver, DbQueryExecutor, Environment, ExecContext, ExecErrorKind, NodeExecutor,
};
use model::Node;
use serde_json::{json, Value};
use store::{
    ConnectionRepository, ConnectionStatus, ConnectionType, Db, DbConnectionRecord,
    DbConnectionSaveInput,
};
use tempfile::NamedTempFile;

struct MapResolver {
    env: String,
    bindings: BTreeMap<String, DbConnectionRecord>,
}

impl MapResolver {
    fn new(env: &str, bindings: BTreeMap<String, DbConnectionRecord>) -> Self {
        Self {
            env: env.to_owned(),
            bindings,
        }
    }
}

impl ConnectionResolver for MapResolver {
    fn resolve(&self, env_name: &str, logical_name: &str) -> Result<DbConnectionRecord, String> {
        if env_name != self.env {
            return Err(format!("unexpected environment '{env_name}'"));
        }
        self.bindings
            .get(logical_name)
            .cloned()
            .ok_or_else(|| format!("missing '{logical_name}'"))
    }
}

fn sqlite_record(name: &str, path: &str, read_only: bool) -> DbConnectionRecord {
    DbConnectionRecord {
        connection: DbConnectionSaveInput {
            name: name.into(),
            connection_type: ConnectionType::Sqlite,
            host: None,
            port: None,
            database_name: Some(path.into()),
            schema_name: None,
            auth_ref: None,
            read_only,
            pool_config: None,
            status: ConnectionStatus::Active,
        },
    }
}

fn prepare_users_db(path: &str) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, level TEXT NOT NULL);
         INSERT INTO users (name, level) VALUES ('Ada', 'VIP');
         INSERT INTO users (name, level) VALUES ('Bob', 'STD');",
    )
    .unwrap();
}

async fn exec_query(
    node: Value,
    resolver: Arc<dyn ConnectionResolver>,
) -> Result<Value, engine::ExecError> {
    let node: Node = serde_json::from_value(node).unwrap();
    let exec = DbQueryExecutor::new(resolver);
    let ctx = ExecContext {
        run_id: "run-1".into(),
        outputs: BTreeMap::new(),
        env: Arc::new(Environment::default()),
        ..ExecContext::default()
    };
    Ok(exec.execute(&node, ctx).await?.output)
}

#[tokio::test]
async fn parameterized_query_and_read_only_rules() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap().to_string();
    prepare_users_db(&path);

    let resolver: Arc<dyn ConnectionResolver> = Arc::new(MapResolver::new(
        "default",
        BTreeMap::from([
            ("app".into(), sqlite_record("app", &path, false)),
            ("ro".into(), sqlite_record("ro", &path, true)),
        ]),
    ));

    let rows = exec_query(
        json!({
            "id": "all",
            "type": "db_query",
            "connection": "app",
            "sql": "SELECT id, name FROM users WHERE level = ?",
            "params": ["VIP"],
        }),
        Arc::clone(&resolver),
    )
    .await
    .unwrap();
    assert_eq!(rows, json!({"rows": [{"id": 1, "name": "Ada"}]}));

    let scalar = exec_query(
        json!({
            "id": "c",
            "type": "db_query",
            "connection": "app",
            "sql": "SELECT COUNT(*) FROM users",
            "result": "scalar",
        }),
        Arc::clone(&resolver),
    )
    .await
    .unwrap();
    assert_eq!(scalar, json!({"value": 2}));

    let err = exec_query(
        json!({
            "id": "bad",
            "type": "db_query",
            "connection": "ro",
            "sql": "UPDATE users SET level = 'X' WHERE id = 1",
        }),
        Arc::clone(&resolver),
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind, ExecErrorKind::Internal);
    assert!(err.message.contains("read-only"));

    let ok = exec_query(
        json!({
            "id": "q",
            "type": "db_query",
            "connection": "ro",
            "sql": "SELECT COUNT(*) FROM users",
            "result": "scalar",
        }),
        resolver,
    )
    .await
    .unwrap();
    assert_eq!(ok, json!({"value": 2}));
}

#[tokio::test]
async fn store_resolver_binding() {
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(0)));
    let db = Db::open_in_memory(clock.as_ref()).unwrap();
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap().to_string();
    prepare_users_db(&path);
    let repo = ConnectionRepository::new(db.conn());
    repo.save(&DbConnectionSaveInput {
        name: "sqlite-main".into(),
        connection_type: ConnectionType::Sqlite,
        host: None,
        port: None,
        database_name: Some(path),
        schema_name: None,
        auth_ref: None,
        read_only: false,
        pool_config: None,
        status: ConnectionStatus::Active,
    })
    .unwrap();
    repo.bind_environment("local", "orders-ro", "sqlite-main")
        .unwrap();

    let resolver: Arc<dyn ConnectionResolver> = Arc::new(engine::StoreConnectionResolver::new(
        Arc::new(std::sync::Mutex::new(db)),
    ));
    let env = Arc::new(Environment::new("local", "http://example", BTreeMap::new()).unwrap());
    let node: Node = serde_json::from_value(json!({
        "id": "lookup",
        "type": "db_query",
        "connection": "orders-ro",
        "sql": "SELECT name FROM users WHERE level = ?",
        "params": ["VIP"],
        "result": "first_row",
    }))
    .unwrap();
    let ctx = ExecContext {
        run_id: "r".into(),
        outputs: BTreeMap::new(),
        env,
        ..ExecContext::default()
    };
    let out = DbQueryExecutor::new(resolver)
        .execute(&node, ctx)
        .await
        .unwrap()
        .output;
    assert_eq!(out, json!({"row": {"name": "Ada"}}));
}

#[tokio::test]
async fn router_dispatches_db_query() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap().to_string();
    prepare_users_db(&path);
    let resolver: Arc<dyn ConnectionResolver> = Arc::new(MapResolver::new(
        "default",
        BTreeMap::from([("app".into(), sqlite_record("app", &path, false))]),
    ));
    let clock = Arc::new(FixedClock::new(Timestamp::from_unix_millis(0)));
    let router = engine::NodeRouter::new(
        Arc::new(engine::HttpExecutor::new(clock.clone()).unwrap()),
        clock,
        Arc::new(TokioSleeper),
    )
    .with_db_query(DbQueryExecutor::new(resolver));
    let node: Node = serde_json::from_value(json!({
        "id": "q",
        "type": "db_query",
        "connection": "app",
        "sql": "SELECT 1 AS one",
        "result": "scalar",
    }))
    .unwrap();
    let ctx = ExecContext {
        run_id: "run".into(),
        outputs: BTreeMap::new(),
        env: Arc::new(Environment::default()),
        ..ExecContext::default()
    };
    assert_eq!(
        router.execute(&node, ctx).await.unwrap().output,
        json!({"value": 1})
    );
}
