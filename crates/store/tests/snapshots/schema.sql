CREATE TABLE "api_specs" (
    id                   TEXT NOT NULL,
    version              INTEGER NOT NULL CHECK (version >= 1),
    name                 TEXT NOT NULL,
    method               TEXT NOT NULL,
    path                 TEXT NOT NULL,
    params_json          TEXT CHECK (params_json IS NULL OR json_valid(params_json)),
    response_schema_json TEXT CHECK (response_schema_json IS NULL OR json_valid(response_schema_json)),
    auth_scheme_ref      TEXT,
    status               TEXT NOT NULL DEFAULT 'draft'
                         CHECK (status IN ('draft', 'active', 'deprecated')),
    created_by           TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    PRIMARY KEY (id, version)
) STRICT;

CREATE UNIQUE INDEX api_specs_one_active ON api_specs (id) WHERE status = 'active';

CREATE TABLE dag_definitions (
    id           TEXT NOT NULL,
    version      INTEGER NOT NULL CHECK (version >= 1),
    content_json TEXT NOT NULL CHECK (json_valid(content_json)),
    status       TEXT NOT NULL DEFAULT 'pending'
                 CHECK (status IN ('pending', 'active', 'archived')),
    created_by   TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    PRIMARY KEY (id, version)
) STRICT;

CREATE UNIQUE INDEX dag_definitions_one_active ON dag_definitions (id) WHERE status = 'active';

CREATE TABLE dag_runs (
    id          TEXT PRIMARY KEY,
    dag_id      TEXT NOT NULL,
    dag_version INTEGER NOT NULL,
    env_name    TEXT NOT NULL,
    mode        TEXT NOT NULL CHECK (mode IN ('manual', 'ci')),
    status      TEXT NOT NULL DEFAULT 'PENDING'
                CHECK (status IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED')),
    started_at  TEXT,
    finished_at TEXT,
    FOREIGN KEY (dag_id, dag_version) REFERENCES dag_definitions (id, version)
) STRICT;

CREATE INDEX dag_runs_by_dag ON dag_runs (dag_id, dag_version);

CREATE INDEX dag_runs_by_status ON dag_runs (status, started_at);

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

CREATE TABLE environment_connections (
    env_name         TEXT NOT NULL,
    logical_name     TEXT NOT NULL,
    connection_name  TEXT NOT NULL,
    PRIMARY KEY (env_name, logical_name),
    FOREIGN KEY (connection_name) REFERENCES db_connections (name) ON DELETE RESTRICT
) STRICT;

CREATE INDEX environment_connections_by_connection
    ON environment_connections (connection_name);

CREATE TABLE failure_records (
    id               TEXT PRIMARY KEY,
    run_id           TEXT NOT NULL REFERENCES dag_runs (id) ON DELETE CASCADE,
    node_id          TEXT,
    failure_category TEXT NOT NULL,
    error_code       TEXT,
    raw_message      TEXT NOT NULL,
    classified_at    TEXT NOT NULL,
    resolution       TEXT
) STRICT;

CREATE INDEX failure_records_by_category ON failure_records (failure_category, classified_at);

CREATE INDEX failure_records_by_run ON failure_records (run_id, node_id);

CREATE TABLE llm_call_logs (
    id          TEXT PRIMARY KEY,
    run_id      TEXT,
    node_id     TEXT,
    purpose     TEXT NOT NULL,
    tokens_in   INTEGER NOT NULL DEFAULT 0 CHECK (tokens_in >= 0),
    tokens_out  INTEGER NOT NULL DEFAULT 0 CHECK (tokens_out >= 0),
    cost        REAL CHECK (cost IS NULL OR cost >= 0),
    latency_ms  INTEGER NOT NULL CHECK (latency_ms >= 0),
    outcome     TEXT NOT NULL CHECK (outcome IN ('success', 'error'))
) STRICT;

CREATE INDEX llm_call_logs_by_purpose ON llm_call_logs (purpose);

CREATE INDEX llm_call_logs_by_run ON llm_call_logs (run_id) WHERE run_id IS NOT NULL;

CREATE TRIGGER node_executions_failed_insert_classified
BEFORE INSERT ON node_executions
WHEN NEW.status = 'FAILED' AND (NEW.failure_category IS NULL OR NEW.error_code IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'failed node execution needs failure_category and error_code');
END;

CREATE TRIGGER node_executions_failed_update_classified
BEFORE UPDATE ON node_executions
WHEN NEW.status = 'FAILED' AND (NEW.failure_category IS NULL OR NEW.error_code IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'failed node execution needs failure_category and error_code');
END;

CREATE TRIGGER node_executions_terminal_immutable
BEFORE UPDATE ON node_executions
WHEN OLD.status IN ('COMPLETED', 'FAILED', 'SKIPPED')
BEGIN
    SELECT RAISE(ABORT, 'terminal node execution is immutable');
END;

CREATE TABLE node_executions (
    run_id              TEXT NOT NULL REFERENCES dag_runs (id) ON DELETE CASCADE,
    node_id             TEXT NOT NULL,
    attempt             INTEGER NOT NULL CHECK (attempt >= 1),
    status              TEXT NOT NULL
                        CHECK (status IN ('PENDING', 'READY', 'RUNNING', 'POLLING',
                                          'COMPLETED', 'FAILED', 'SKIPPED')),
    input_resolved_json TEXT CHECK (input_resolved_json IS NULL OR json_valid(input_resolved_json)),
    output_json         TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
    poll_trace_json     TEXT CHECK (poll_trace_json IS NULL OR json_valid(poll_trace_json)),
    failure_category    TEXT,
    error_code          TEXT,
    started_at          TEXT,
    finished_at         TEXT,
    PRIMARY KEY (run_id, node_id, attempt)
) STRICT;

CREATE TABLE schema_migrations (
    version    INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    checksum   TEXT NOT NULL,
    applied_at TEXT NOT NULL
) STRICT;

CREATE TABLE skill_references (
    skill_id      TEXT NOT NULL,
    skill_version INTEGER NOT NULL,
    dag_id        TEXT NOT NULL,
    dag_version   INTEGER NOT NULL,
    PRIMARY KEY (skill_id, skill_version, dag_id, dag_version),
    FOREIGN KEY (skill_id, skill_version) REFERENCES skills (id, version),
    FOREIGN KEY (dag_id, dag_version) REFERENCES dag_definitions (id, version)
) STRICT;

CREATE INDEX skill_references_by_dag ON skill_references (dag_id, dag_version);

CREATE INDEX skill_references_by_skill ON skill_references (skill_id, skill_version);

CREATE TABLE skills (
    id                    TEXT NOT NULL,
    version               INTEGER NOT NULL CHECK (version >= 1),
    name                  TEXT NOT NULL,
    content_json          TEXT NOT NULL CHECK (json_valid(content_json)),
    input_contract_json   TEXT CHECK (input_contract_json IS NULL OR json_valid(input_contract_json)),
    output_contract_json  TEXT CHECK (output_contract_json IS NULL OR json_valid(output_contract_json)),
    tsx_plugin_ref        TEXT,
    status                TEXT NOT NULL DEFAULT 'pending'
                          CHECK (status IN ('pending', 'active', 'archived')),
    PRIMARY KEY (id, version)
) STRICT;

CREATE UNIQUE INDEX skills_one_active ON skills (id) WHERE status = 'active';

CREATE TABLE ui_layouts (
    scope_key   TEXT PRIMARY KEY,
    layout_json TEXT NOT NULL CHECK (json_valid(layout_json)),
    updated_at  TEXT NOT NULL
) STRICT;

CREATE TABLE workspace_presets (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    layout_json TEXT NOT NULL CHECK (json_valid(layout_json)),
    updated_at  TEXT NOT NULL
) STRICT;
