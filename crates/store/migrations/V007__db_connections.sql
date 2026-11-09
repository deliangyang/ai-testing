-- Named database connections and per-environment bindings (develop.md §3.1, §4.9).

CREATE TABLE db_connections (
    name              TEXT PRIMARY KEY,
    type              TEXT NOT NULL CHECK (type IN ('postgres', 'mysql', 'sqlite')),
    host              TEXT,
    port              INTEGER CHECK (port IS NULL OR (port >= 1 AND port <= 65535)),
    database_name     TEXT,
    schema_name       TEXT,
    auth_ref          TEXT,
    read_only         INTEGER NOT NULL DEFAULT 0 CHECK (read_only IN (0, 1)),
    pool_config_json  TEXT CHECK (pool_config_json IS NULL OR json_valid(pool_config_json)),
    status            TEXT NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft', 'active', 'archived'))
) STRICT;

CREATE INDEX db_connections_by_status ON db_connections (status, name COLLATE NOCASE);

-- Maps DAG-facing connection name to a stored connection per environment.
CREATE TABLE environment_connections (
    env_name         TEXT NOT NULL,
    logical_name     TEXT NOT NULL,
    connection_name  TEXT NOT NULL,
    PRIMARY KEY (env_name, logical_name),
    FOREIGN KEY (connection_name) REFERENCES db_connections (name) ON DELETE RESTRICT
) STRICT;

CREATE INDEX environment_connections_by_connection
    ON environment_connections (connection_name);
