-- Node executions are evidence: once a row reaches a terminal status it never changes.
CREATE TRIGGER node_executions_terminal_immutable
BEFORE UPDATE ON node_executions
WHEN OLD.status IN ('COMPLETED', 'FAILED', 'SKIPPED')
BEGIN
    SELECT RAISE(ABORT, 'terminal node execution is immutable');
END;

-- Every failed node is classified (AGENTS.md red line 6).
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
