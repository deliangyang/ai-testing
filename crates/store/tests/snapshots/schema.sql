CREATE TABLE api_specs (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    method               TEXT NOT NULL,
    path                 TEXT NOT NULL,
    params_json          TEXT CHECK (params_json IS NULL OR json_valid(params_json)),
    response_schema_json TEXT CHECK (response_schema_json IS NULL OR json_valid(response_schema_json)),
    auth_scheme_ref      TEXT,
    version              INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    status               TEXT NOT NULL DEFAULT 'draft'
                         CHECK (status IN ('draft', 'active', 'deprecated'))
) STRICT;

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
