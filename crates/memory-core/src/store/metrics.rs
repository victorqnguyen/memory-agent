use rusqlite::params;

use crate::types::MemoryMetric;
use crate::Result;

use super::Store;

impl Store {
    pub fn record_injection(&self, memory_ids: &[i64], tokens_per_memory: i32) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        for id in memory_ids {
            tx.execute(
                "INSERT INTO metrics (memory_id, injections, tokens_injected, last_injected_at)
                 VALUES (?1, 1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(memory_id) DO UPDATE SET
                    injections = injections + 1,
                    tokens_injected = tokens_injected + ?2,
                    last_injected_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![id, tokens_per_memory],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn record_hit(&self, memory_id: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO metrics (memory_id, hits, last_hit_at)
             VALUES (?1, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(memory_id) DO UPDATE SET
                hits = hits + 1,
                last_hit_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![memory_id],
        )?;
        Ok(())
    }

    pub fn record_hit_batch(&self, memory_ids: &[i64]) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        for id in memory_ids {
            tx.execute(
                "INSERT INTO metrics (memory_id, hits, last_hit_at)
                 VALUES (?1, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(memory_id) DO UPDATE SET
                    hits = hits + 1,
                    last_hit_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Cumulative token stats across all time (from the metrics table).
    pub fn cumulative_stats(&self) -> Result<crate::types::TokenStats> {
        let (injections, hits, tokens_injected) = self.conn().query_row(
            "SELECT COALESCE(SUM(injections), 0), COALESCE(SUM(hits), 0), COALESCE(SUM(tokens_injected), 0) FROM metrics",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        )?;
        let unique = self.conn().query_row(
            "SELECT COUNT(*) FROM metrics WHERE injections > 0",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(crate::types::TokenStats {
            injections,
            hits,
            unique_memories_injected: unique,
            tokens_injected,
        })
    }

    pub fn dedup_total(&self) -> Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COALESCE(SUM(duplicate_count), 0) FROM memories WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn revision_total(&self) -> Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COALESCE(SUM(revision_count), 0) FROM memories WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn low_roi_count(&self) -> Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM metrics WHERE injections > 10 AND CAST(hits AS REAL) / injections < 0.1",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn get_metrics(&self) -> Result<Vec<MemoryMetric>> {
        let mut stmt = self.conn().prepare(
            "SELECT m.id, m.key, m.scope,
                    COALESCE(mt.injections, 0),
                    COALESCE(mt.hits, 0),
                    COALESCE(mt.tokens_injected, 0),
                    CASE WHEN COALESCE(mt.injections, 0) > 0
                         THEN CAST(COALESCE(mt.hits, 0) AS REAL) / mt.injections
                         ELSE 0.0 END
             FROM memories m
             LEFT JOIN metrics mt ON mt.memory_id = m.id
             WHERE m.deleted_at IS NULL
             ORDER BY COALESCE(mt.injections, 0) DESC
             LIMIT 100",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(MemoryMetric {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    scope: row.get(2)?,
                    injections: row.get(3)?,
                    hits: row.get(4)?,
                    tokens_injected: row.get(5)?,
                    hit_rate: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;
    use crate::types::SaveParams;

    fn make_memory(store: &Store, key: &str) -> i64 {
        store
            .save(SaveParams {
                key: key.to_string(),
                value: "test value".to_string(),
                ..Default::default()
            })
            .unwrap()
            .id()
    }

    #[test]
    fn test_record_injection_increments() {
        let store = Store::open_in_memory().unwrap();
        let id = make_memory(&store, "test/key");
        store.record_injection(&[id], 0).unwrap();
        store.record_injection(&[id], 0).unwrap();
        let metrics = store.get_metrics().unwrap();
        let m = metrics.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.injections, 2);
    }

    #[test]
    fn test_record_hit_increments() {
        let store = Store::open_in_memory().unwrap();
        let id = make_memory(&store, "test/key");
        store.record_hit(id).unwrap();
        store.record_hit(id).unwrap();
        store.record_hit(id).unwrap();
        let metrics = store.get_metrics().unwrap();
        let m = metrics.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.hits, 3);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let store = Store::open_in_memory().unwrap();
        let id = make_memory(&store, "test/key");
        store.record_injection(&[id], 0).unwrap();
        store.record_injection(&[id], 0).unwrap();
        store.record_hit(id).unwrap();
        let metrics = store.get_metrics().unwrap();
        let m = metrics.iter().find(|m| m.id == id).unwrap();
        assert!((m.hit_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_injection_accumulates_tokens() {
        let store = Store::open_in_memory().unwrap();
        let id = make_memory(&store, "key/one");
        store.record_injection(&[id], 50).unwrap();
        store.record_injection(&[id], 50).unwrap();
        let metrics = store.get_metrics().unwrap();
        let m = metrics.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.tokens_injected, 100);
    }
}
