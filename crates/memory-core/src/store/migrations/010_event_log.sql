-- Replace sessions/access_log with a lightweight event log.
-- sessions and access_log are intentionally LEFT in place to preserve the FK from
-- memories.session_id; they are simply no longer written to by the application.

CREATE TABLE IF NOT EXISTS event_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    action     TEXT NOT NULL,
    key        TEXT NOT NULL DEFAULT '',
    scope      TEXT NOT NULL DEFAULT '/',
    tokens     INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_event_log_created_at ON event_log (created_at DESC);
