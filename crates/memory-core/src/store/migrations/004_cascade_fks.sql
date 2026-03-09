-- Fix foreign keys to cascade on delete so hard-deleting memories doesn't fail
-- when metrics or relations rows reference them.

-- Metrics table: recreate with ON DELETE CASCADE
CREATE TABLE IF NOT EXISTS metrics_new (
    memory_id INTEGER PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    injections INTEGER NOT NULL DEFAULT 0,
    hits INTEGER NOT NULL DEFAULT 0,
    tokens_injected INTEGER NOT NULL DEFAULT 0,
    last_injected_at TEXT,
    last_hit_at TEXT
);
INSERT OR IGNORE INTO metrics_new SELECT * FROM metrics;
DROP TABLE IF EXISTS metrics;
ALTER TABLE metrics_new RENAME TO metrics;

-- Relations table: recreate with ON DELETE CASCADE
CREATE TABLE IF NOT EXISTS relations_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(source_id, target_id, relation_type)
);
INSERT OR IGNORE INTO relations_new SELECT * FROM relations;
DROP TABLE IF EXISTS relations;
ALTER TABLE relations_new RENAME TO relations;

CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id);
CREATE INDEX IF NOT EXISTS idx_relations_type ON relations(relation_type);
