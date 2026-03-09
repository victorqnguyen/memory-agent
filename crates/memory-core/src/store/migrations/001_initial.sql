CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    directory TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ended_at TEXT,
    summary TEXT,
    status TEXT NOT NULL DEFAULT 'active'
);

CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT '/',
    source_type TEXT NOT NULL DEFAULT 'explicit',
    source_ref TEXT,
    source_commit TEXT,
    confidence REAL NOT NULL DEFAULT 1.0 CHECK (confidence >= 0.0 AND confidence <= 1.0),
    session_id TEXT REFERENCES sessions(id),
    tags TEXT,
    revision_count INTEGER NOT NULL DEFAULT 0,
    duplicate_count INTEGER NOT NULL DEFAULT 0,
    normalized_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    accessed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    key, value, tags, source_type, scope,
    content=memories, content_rowid=id
);

CREATE TRIGGER IF NOT EXISTS memories_fts_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, key, value, tags, source_type, scope)
    VALUES (new.id, new.key, new.value, new.tags, new.source_type, new.scope);
END;

CREATE TRIGGER IF NOT EXISTS memories_fts_delete AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, value, tags, source_type, scope)
    VALUES ('delete', old.id, old.key, old.value, old.tags, old.source_type, old.scope);
END;

CREATE TRIGGER IF NOT EXISTS memories_fts_update AFTER UPDATE ON memories
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, value, tags, source_type, scope)
    VALUES ('delete', old.id, old.key, old.value, old.tags, old.source_type, old.scope);
    INSERT INTO memories_fts(rowid, key, value, tags, source_type, scope)
    VALUES (new.id, new.key, new.value, new.tags, new.source_type, new.scope);
END;

CREATE TRIGGER IF NOT EXISTS memories_fts_softdelete AFTER UPDATE ON memories
WHEN new.deleted_at IS NOT NULL AND old.deleted_at IS NULL BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, value, tags, source_type, scope)
    VALUES ('delete', old.id, old.key, old.value, old.tags, old.source_type, old.scope);
END;

CREATE INDEX IF NOT EXISTS idx_memories_key_scope
    ON memories(key, scope) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_memories_source
    ON memories(source_type, source_ref) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_memories_session
    ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_hash
    ON memories(normalized_hash, scope) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_memories_accessed
    ON memories(accessed_at DESC) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS _metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO _metadata (key, value)
VALUES ('db_created_at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
