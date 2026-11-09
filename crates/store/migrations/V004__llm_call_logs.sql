-- LLM invocation audit trail (develop.md §3.1). Metadata only; no prompts or credentials.

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

CREATE INDEX llm_call_logs_by_run ON llm_call_logs (run_id) WHERE run_id IS NOT NULL;

CREATE INDEX llm_call_logs_by_purpose ON llm_call_logs (purpose);
