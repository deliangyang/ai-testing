-- UI shell layout per workspace scope (develop.md §5.4).

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

INSERT INTO workspace_presets (id, name, layout_json, updated_at) VALUES
(
    'orchestrator',
    '编排者',
    '{"nav_collapsed":false,"right_width_px":400,"drawer_height_px":140,"drawer_collapsed":true}',
    '1970-01-01T00:00:00.000Z'
),
(
    'debugger',
    '执行调试者',
    '{"nav_collapsed":false,"right_width_px":360,"drawer_height_px":220,"drawer_collapsed":false}',
    '1970-01-01T00:00:00.000Z'
),
(
    'reporter',
    '看报告者',
    '{"nav_collapsed":true,"right_width_px":300,"drawer_height_px":140,"drawer_collapsed":true}',
    '1970-01-01T00:00:00.000Z'
);
