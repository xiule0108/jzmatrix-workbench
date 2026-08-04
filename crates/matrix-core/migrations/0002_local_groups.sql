ALTER TABLE collaboration_groups ADD COLUMN template_id TEXT NOT NULL DEFAULT 'compact';

CREATE TABLE IF NOT EXISTS collaboration_group_templates (
    id TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    label TEXT NOT NULL,
    description TEXT NOT NULL,
    roles_json TEXT NOT NULL CHECK (json_valid(roles_json)),
    data_source TEXT NOT NULL CHECK (data_source IN ('demo', 'fixture')),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collaboration_group_roles (
    group_id TEXT NOT NULL REFERENCES collaboration_groups(id) ON DELETE CASCADE,
    role_id TEXT NOT NULL,
    role_label TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (group_id, role_id)
);

CREATE TABLE IF NOT EXISTS collaboration_group_facts (
    group_id TEXT NOT NULL REFERENCES collaboration_groups(id) ON DELETE CASCADE,
    fact_kind TEXT NOT NULL CHECK (
        fact_kind IN (
            'activity',
            'progress',
            'local_written',
            'sent_not_confirmed',
            'delivered',
            'accepted',
            'completed'
        )
    ),
    state TEXT NOT NULL CHECK (state IN ('unknown', 'observed', 'not_confirmed', 'not_run')),
    evidence_ref TEXT,
    observed_at TEXT NOT NULL,
    PRIMARY KEY (group_id, fact_kind)
);

CREATE TABLE IF NOT EXISTS group_idempotency_keys (
    operation_key TEXT PRIMARY KEY NOT NULL,
    operation_kind TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    group_id TEXT NOT NULL REFERENCES collaboration_groups(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_group_roles_group ON collaboration_group_roles(group_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_group_facts_group ON collaboration_group_facts(group_id);
CREATE INDEX IF NOT EXISTS idx_group_idempotency_group ON group_idempotency_keys(group_id);

INSERT OR IGNORE INTO collaboration_group_templates
    (id, version, label, description, roles_json, data_source, created_at)
VALUES
    ('compact', '1.0.0', '简单完成', '适合先把一件事讲清楚并得到检查', '["统筹","执行","独立检查"]', 'fixture', '2026-08-04T00:00:00Z'),
    ('research', '1.0.0', '查资料并核对', '增加资料与复核分工，适合事实核验', '["统筹","资料","独立检查"]', 'fixture', '2026-08-04T00:00:00Z'),
    ('product', '1.0.0', '从需求做到成品', '覆盖需求、实现和发布前检查', '["统筹","执行","验证","发布前检查"]', 'fixture', '2026-08-04T00:00:00Z'),
    ('content', '1.0.0', '写作与成品', '适合资料、写作、视觉和发布前检查', '["资料","写作","成品整理","检查"]', 'fixture', '2026-08-04T00:00:00Z');
