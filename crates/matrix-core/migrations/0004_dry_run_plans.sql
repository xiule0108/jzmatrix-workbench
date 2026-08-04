CREATE TABLE IF NOT EXISTS dry_run_plans (
    id TEXT PRIMARY KEY NOT NULL,
    group_id TEXT NOT NULL REFERENCES collaboration_groups(id) ON DELETE CASCADE,
    observed_at TEXT NOT NULL,
    data_source TEXT NOT NULL CHECK (data_source IN ('demo', 'fixture', 'real')),
    execution TEXT NOT NULL CHECK (execution = 'not_authorized'),
    plan_json TEXT NOT NULL CHECK (json_valid(plan_json))
);

CREATE INDEX IF NOT EXISTS idx_dry_run_plans_group
    ON dry_run_plans(group_id, observed_at DESC);
