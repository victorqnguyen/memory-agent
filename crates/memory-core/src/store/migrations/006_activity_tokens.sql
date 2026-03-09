ALTER TABLE access_log ADD COLUMN tokens_injected INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN tokens_used_input INTEGER;
ALTER TABLE sessions ADD COLUMN tokens_used_output INTEGER;
