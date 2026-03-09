CREATE INDEX IF NOT EXISTS idx_relations_supersedes ON relations(relation_type, target_id) WHERE relation_type = 'supersedes';
