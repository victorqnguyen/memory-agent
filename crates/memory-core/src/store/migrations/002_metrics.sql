CREATE TABLE IF NOT EXISTS access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    action TEXT NOT NULL,
    query TEXT,
    memory_ids TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS metrics (
    memory_id INTEGER PRIMARY KEY REFERENCES memories(id),
    injections INTEGER NOT NULL DEFAULT 0,
    hits INTEGER NOT NULL DEFAULT 0,
    tokens_injected INTEGER NOT NULL DEFAULT 0,
    last_injected_at TEXT,
    last_hit_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_access_log_query ON access_log(query, created_at);
CREATE INDEX IF NOT EXISTS idx_access_log_session ON access_log(session_id);
