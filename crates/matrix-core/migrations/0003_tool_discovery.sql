CREATE TABLE IF NOT EXISTS tool_discovery_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    observed_at TEXT NOT NULL,
    data_source TEXT NOT NULL CHECK (data_source = 'real'),
    invocation TEXT NOT NULL CHECK (invocation = 'user_triggered'),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json))
);

CREATE INDEX IF NOT EXISTS idx_tool_discovery_observed
    ON tool_discovery_snapshots(observed_at DESC);
