-- Skill library and DAG pins (develop.md §3.1).

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

CREATE TABLE skill_references (
    skill_id      TEXT NOT NULL,
    skill_version INTEGER NOT NULL,
    dag_id        TEXT NOT NULL,
    dag_version   INTEGER NOT NULL,
    PRIMARY KEY (skill_id, skill_version, dag_id, dag_version),
    FOREIGN KEY (skill_id, skill_version) REFERENCES skills (id, version),
    FOREIGN KEY (dag_id, dag_version) REFERENCES dag_definitions (id, version)
) STRICT;

CREATE INDEX skill_references_by_skill ON skill_references (skill_id, skill_version);

CREATE INDEX skill_references_by_dag ON skill_references (dag_id, dag_version);
