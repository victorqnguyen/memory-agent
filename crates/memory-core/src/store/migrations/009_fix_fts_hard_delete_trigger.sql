-- Fix FTS5 hard-delete trigger to skip rows already removed by soft-delete.
-- When purge_soft_deleted() hard-DELETEs a soft-deleted row, the FTS5 entry was
-- already removed by memories_fts_softdelete. Double-deleting from FTS5 causes
-- SQLITE_CORRUPT_VTAB (error 267). Guard with WHEN old.deleted_at IS NULL so the
-- trigger only runs for active rows being hard-deleted directly.
DROP TRIGGER IF EXISTS memories_fts_delete;

CREATE TRIGGER memories_fts_delete AFTER DELETE ON memories
WHEN old.deleted_at IS NULL BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, value, tags, source_type, scope)
    VALUES ('delete', old.id, old.key, old.value, old.tags, old.source_type, old.scope);
END;
