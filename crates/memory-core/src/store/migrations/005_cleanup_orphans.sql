-- Clean up orphaned rows created before migration 004 added CASCADE foreign keys.
DELETE FROM metrics WHERE memory_id NOT IN (SELECT id FROM memories);
DELETE FROM relations WHERE source_id NOT IN (SELECT id FROM memories) OR target_id NOT IN (SELECT id FROM memories);
