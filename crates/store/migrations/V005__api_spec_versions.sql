-- Versioned api_specs: composite primary key (id, version), at most one active per id.

CREATE TABLE api_specs_v2 (
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

INSERT INTO api_specs_v2 (
    id,
    version,
    name,
    method,
    path,
    params_json,
    response_schema_json,
    auth_scheme_ref,
    status,
    created_by,
    created_at
)
SELECT
    id,
    version,
    name,
    method,
    path,
    params_json,
    response_schema_json,
    auth_scheme_ref,
    status,
    'migration',
    '1970-01-01T00:00:00.000Z'
FROM api_specs;

DROP TABLE api_specs;

ALTER TABLE api_specs_v2 RENAME TO api_specs;

CREATE UNIQUE INDEX api_specs_one_active ON api_specs (id) WHERE status = 'active';
