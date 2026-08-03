CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    schema_contract TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    checksum_sha256 TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('committed', 'failed'))
);

CREATE TABLE IF NOT EXISTS collaboration_groups (
    id TEXT PRIMARY KEY NOT NULL,
    data_source TEXT NOT NULL CHECK (data_source IN ('real', 'demo')),
    goal TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
