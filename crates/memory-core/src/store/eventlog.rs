use rusqlite::params;

use crate::types::EventLogEntry;
use crate::Result;

use super::Store;

impl Store {
    /// Write an event to the live activity feed.
    /// `action`: "save" | "search" | "inject" | "hit"
    /// `key`:    memory key, query string, or count description
    /// `scope`:  scope path
    /// `tokens`: tokens injected (non-zero only for "inject" action)
    pub fn write_event(&self, action: &str, key: &str, scope: &str, tokens: i32) -> Result<()> {
        self.conn().execute(
            "INSERT INTO event_log (action, key, scope, tokens) VALUES (?1, ?2, ?3, ?4)",
            params![action, key, scope, tokens],
        )?;
        Ok(())
    }

    /// Return the N most recent events, newest first.
    pub fn recent_events(&self, limit: i32) -> Result<Vec<EventLogEntry>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, action, key, scope, tokens, created_at
             FROM event_log
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let results = stmt
            .query_map(params![limit], |row| {
                Ok(EventLogEntry {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    key: row.get(2)?,
                    scope: row.get(3)?,
                    tokens: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Counts for today: (saves, injections, searches, tokens_injected).
    pub fn events_today_summary(&self) -> Result<(i64, i64, i64, i64)> {
        Ok(self.conn().query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN action = 'save'   THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN action = 'inject' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN action = 'search' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN action = 'inject' THEN tokens ELSE 0 END), 0)
             FROM event_log
             WHERE date(created_at) = date('now')",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?)
    }

    /// Purge events older than `days` days. Called by the maintenance scheduler.
    pub fn purge_old_events(&self, days: u32) -> Result<u64> {
        let deleted = self.conn().execute(
            "DELETE FROM event_log
             WHERE created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ?1 || ' days')",
            params![days],
        )?;
        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_read_events() {
        let store = Store::open_in_memory().unwrap();
        store
            .write_event("save", "arch/decision", "/myproject", 0)
            .unwrap();
        store
            .write_event("inject", "3 memories", "/myproject", 450)
            .unwrap();

        let events = store.recent_events(10).unwrap();
        assert_eq!(events.len(), 2);
        // newest first
        assert_eq!(events[0].action, "inject");
        assert_eq!(events[0].tokens, 450);
        assert_eq!(events[1].action, "save");
        assert_eq!(events[1].key, "arch/decision");
    }

    #[test]
    fn test_events_today_summary() {
        let store = Store::open_in_memory().unwrap();
        store.write_event("save", "k1", "/", 0).unwrap();
        store.write_event("save", "k2", "/", 0).unwrap();
        store.write_event("search", "rust async", "/", 0).unwrap();
        store.write_event("inject", "2 memories", "/", 300).unwrap();

        let (saves, injections, searches, tokens) = store.events_today_summary().unwrap();
        assert_eq!(saves, 2);
        assert_eq!(injections, 1);
        assert_eq!(searches, 1);
        assert_eq!(tokens, 300);
    }

    #[test]
    fn test_purge_old_events() {
        let store = Store::open_in_memory().unwrap();
        // Insert an old event directly
        store
            .conn()
            .execute(
                "INSERT INTO event_log (action, key, scope, tokens, created_at)
             VALUES ('save', 'old', '/', 0, '2020-01-01T00:00:00.000Z')",
                [],
            )
            .unwrap();
        store.write_event("save", "new", "/", 0).unwrap();

        let deleted = store.purge_old_events(30).unwrap();
        assert_eq!(deleted, 1);
        let events = store.recent_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].key, "new");
    }
}
