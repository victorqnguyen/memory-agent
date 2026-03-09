-- Fix FTS5 update trigger to only fire for content updates on active rows.
-- The original condition (WHEN new.deleted_at IS NULL) would also fire when
-- restoring a soft-deleted row (deleted_at: non-NULL -> NULL), re-indexing it
-- without first removing the stale FTS entry from before the soft-delete.
-- Adding AND old.deleted_at IS NULL restricts the trigger to active->active
-- transitions only, making the intent explicit and safe.
DROP TRIGGER IF EXISTS memories_fts_update;

CREATE TRIGGER memories_fts_update AFTER UPDATE ON memories
WHEN new.deleted_at IS NULL AND old.deleted_at IS NULL BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, value, tags, source_type, scope)
    VALUES ('delete', old.id, old.key, old.value, old.tags, old.source_type, old.scope);
    INSERT INTO memories_fts(rowid, key, value, tags, source_type, scope)
    VALUES (new.id, new.key, new.value, new.tags, new.source_type, new.scope);
END;
