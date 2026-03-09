use std::collections::HashSet;

use rusqlite::params;

use crate::config::SearchConfig;

/// FTS5 column order — must match the CREATE VIRTUAL TABLE in 001_initial.sql.
/// Update this constant when migrations change FTS5 columns.
pub const FTS_COLUMNS: &[&str] = &["key", "value", "tags", "source_type", "scope"];

/// Build a weighted `bm25(memories_fts, ...)` expression using per-column weights from config.
/// Unknown column names in `config.column_weights` are silently ignored.
/// Columns not present in `config.column_weights` default to weight 1.0.
fn build_bm25_expr(config: &SearchConfig) -> String {
    let weights: Vec<String> = FTS_COLUMNS
        .iter()
        .map(|col| {
            let w = config.column_weights.get(*col).copied().unwrap_or(1.0);
            format!("{w}")
        })
        .collect();
    format!("bm25(memories_fts, {})", weights.join(", "))
}

pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn make_preview(value: &str, max_len: usize) -> String {
    if value.len() > max_len {
        format!("{}...", safe_truncate(value, max_len.saturating_sub(3)))
    } else {
        value.to_string()
    }
}

use crate::error::{Error, Result};
use crate::search::{make_or_fallback, sanitize_fts_query};
use crate::store::dedup::compute_hash;
use crate::store::privacy::{strip_private_tags, strip_secrets};
use crate::store::scope::scope_ancestors;
use crate::store::Store;
use crate::types::*;

impl Store {
    pub fn save(&self, params: SaveParams) -> Result<SaveAction> {
        let config = &self.config;
        validate_save_params(&params, &config.validation)?;

        let scope = params
            .scope
            .as_deref()
            .map(normalize_scope)
            .unwrap_or_else(|| "/".to_string());
        validate_scope(&scope)?;

        let source_type = params.source_type.unwrap_or(SourceType::Explicit);

        // Privacy: strip tags and secrets
        let value = strip_private_tags(&params.value);
        let value = strip_secrets(&value, &config.privacy);

        // Entropy filter: reject low-information content for non-explicit sources
        if source_type != SourceType::Explicit
            && source_type != SourceType::Procedural
            && config.storage.entropy_threshold > 0.0
        {
            let score = crate::autonomous::compression::information_score(&value);
            if score < config.storage.entropy_threshold {
                return Err(Error::LowInformation(score));
            }
        }

        // Truncate if needed
        let value = if value.len() > config.validation.max_value_length {
            let mut truncated =
                safe_truncate(&value, config.validation.max_value_length - 3).to_string();
            truncated.push_str("...");
            truncated
        } else {
            value
        };

        let hash = compute_hash(&params.key, &scope, &value);
        let tags_json = params
            .tags
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        // Check for existing memory with same key+scope (upsert)
        let existing = self.conn().query_row(
            "SELECT id, revision_count FROM memories WHERE key = ?1 AND scope = ?2 AND deleted_at IS NULL",
            params![params.key, scope],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)),
        );

        if let Ok((id, rev)) = existing {
            self.conn().execute(
                "UPDATE memories SET value = ?1, source_type = ?2, source_ref = ?3, source_commit = ?4,
                 tags = ?5, normalized_hash = ?6, revision_count = ?7,
                 accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?8",
                params![
                    value,
                    source_type.to_string(),
                    params.source_ref,
                    params.source_commit,
                    tags_json,
                    hash,
                    rev + 1,
                    id,
                ],
            )?;
            return Ok(SaveAction::Updated(id));
        }

        // Check for duplicate hash within dedup window (active rows only).
        // Intentional: soft-deleted rows are excluded so that re-saving previously
        // deleted content creates a fresh entry rather than bumping a deleted one.
        // This allows soft-delete to act as a "clear and re-learn" signal.
        let window_secs = config.storage.dedup_window_secs as i64;
        let dup = self.conn().query_row(
            "SELECT id, duplicate_count FROM memories
             WHERE normalized_hash = ?1 AND scope = ?2 AND deleted_at IS NULL
             AND (julianday('now') - julianday(last_seen_at)) * 86400 < ?3",
            params![hash, scope, window_secs],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)),
        );

        if let Ok((id, dup_count)) = dup {
            self.conn().execute(
                "UPDATE memories SET duplicate_count = ?1,
                 last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2",
                params![dup_count + 1, id],
            )?;
            return Ok(SaveAction::Deduplicated(id));
        }

        // Insert new
        self.conn().execute(
            "INSERT INTO memories (key, value, scope, source_type, source_ref, source_commit,
             tags, normalized_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                params.key,
                value,
                scope,
                source_type.to_string(),
                params.source_ref,
                params.source_commit,
                tags_json,
                hash,
            ],
        )?;

        let id = self.conn().last_insert_rowid();
        Ok(SaveAction::Created(id))
    }

    pub fn get(&self, id: i64) -> Result<Memory> {
        let mem = self
            .conn()
            .query_row(
                "SELECT id, key, value, scope, source_type, source_ref, source_commit,
                 confidence, tags, revision_count, duplicate_count,
                 created_at, accessed_at, last_seen_at
                 FROM memories WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                row_to_memory,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => crate::Error::NotFound(id),
                other => crate::Error::Database(other),
            })?;

        let _ = self.conn().execute(
            "UPDATE memories SET accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            params![id],
        );

        Ok(mem)
    }

    pub fn search(&self, params: SearchParams) -> Result<Vec<SearchResult>> {
        let sanitized = sanitize_fts_query(&params.query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let limit = params
            .limit
            .unwrap_or(self.config.search.default_limit as i32)
            .min(self.config.search.max_limit as i32);

        let bm25 = build_bm25_expr(&self.config.search);

        // Try AND query first (all terms must match); fall back to OR if no results.
        let results = self.run_fts_search(&sanitized, &params, limit, &bm25)?;
        if results.is_empty() {
            if let Some(or_query) = make_or_fallback(&sanitized) {
                return self.run_fts_search(&or_query, &params, limit, &bm25);
            }
        }
        Ok(results)
    }

    fn run_fts_search(
        &self,
        fts_query: &str,
        params: &SearchParams,
        limit: i32,
        bm25: &str,
    ) -> Result<Vec<SearchResult>> {
        let mut sql = format!(
            "SELECT m.id, m.key, m.value, m.scope, m.source_type, m.confidence, {bm25} \
             FROM memories_fts f \
             JOIN memories m ON f.rowid = m.id \
             WHERE memories_fts MATCH ?1 AND m.deleted_at IS NULL \
             AND m.id NOT IN (SELECT target_id FROM relations WHERE relation_type = 'supersedes')"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(fts_query.to_string())];

        // Optional relevance threshold — only applied when configured
        if let Some(threshold) = self.config.search.min_relevance_score {
            sql.push_str(&format!(" AND {bm25} < ?{}", param_values.len() + 1));
            param_values.push(Box::new(threshold));
        }

        if let Some(ref scope) = params.scope {
            let ancestors = scope_ancestors(scope);
            let placeholders: Vec<String> = ancestors
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + 1 + i))
                .collect();
            sql.push_str(&format!(" AND m.scope IN ({})", placeholders.join(",")));
            for a in ancestors {
                param_values.push(Box::new(a));
            }
        }

        if let Some(ref st) = params.source_type {
            sql.push_str(&format!(" AND m.source_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(st.to_string()));
        }

        sql.push_str(&format!(
            " ORDER BY length(m.scope) DESC, {bm25} LIMIT ?{}",
            param_values.len() + 1
        ));
        param_values.push(Box::new(limit));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn().prepare(&sql)?;
        let preview_max = self.config.search.preview_max_chars;
        let preview_min = self.config.search.preview_min_chars;
        let mut row_idx: usize = 0;
        let results = stmt
            .query_map(param_refs.as_slice(), |row| {
                let value: String = row.get(2)?;
                let preview_len = match row_idx {
                    0 => preview_max,
                    1..=2 => 200,
                    _ => preview_min,
                };
                row_idx += 1;
                let preview = make_preview(&value, preview_len);
                let source_str: String = row.get(4)?;
                Ok(SearchResult {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value_preview: preview,
                    scope: row.get(3)?,
                    source_type: source_str.parse().unwrap_or(SourceType::Explicit),
                    confidence: row.get(5)?,
                    rank: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    pub fn delete_scope(&self, scope: &str, hard: bool) -> Result<usize> {
        let scope = normalize_scope(scope);
        let count = if hard {
            self.conn()
                .execute("DELETE FROM memories WHERE scope = ?1", params![scope])?
        } else {
            self.conn().execute(
                "UPDATE memories SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE scope = ?1 AND deleted_at IS NULL",
                params![scope],
            )?
        };
        Ok(count)
    }

    pub fn delete(&self, key: &str, scope: Option<&str>, hard: bool) -> Result<bool> {
        if key.len() > self.config.validation.max_key_length {
            return Err(Error::KeyTooLong(
                key.len(),
                self.config.validation.max_key_length,
            ));
        }

        let scope = scope
            .map(normalize_scope)
            .unwrap_or_else(|| "/".to_string());

        if hard {
            let count = self.conn().execute(
                "DELETE FROM memories WHERE key = ?1 AND scope = ?2",
                params![key, scope],
            )?;
            Ok(count > 0)
        } else {
            let count = self.conn().execute(
                "UPDATE memories SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE key = ?1 AND scope = ?2 AND deleted_at IS NULL",
                params![key, scope],
            )?;
            Ok(count > 0)
        }
    }

    pub fn delete_by_id(&self, id: i64, hard: bool) -> Result<bool> {
        if hard {
            let count = self
                .conn()
                .execute("DELETE FROM memories WHERE id = ?1", params![id])?;
            Ok(count > 0)
        } else {
            let count = self.conn().execute(
                "UPDATE memories SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
            )?;
            Ok(count > 0)
        }
    }

    pub fn update(
        &self,
        id: i64,
        key: Option<&str>,
        value: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<Memory> {
        if let Some(k) = key {
            if k.is_empty() || k.trim().is_empty() {
                return Err(Error::EmptyKey);
            }
            if k.len() > self.config.validation.max_key_length {
                return Err(Error::KeyTooLong(
                    k.len(),
                    self.config.validation.max_key_length,
                ));
            }
            if k.contains('\0') {
                return Err(Error::InvalidInput("null byte in key".to_string()));
            }
        }

        let existing = self.get(id)?;

        let final_key = key.unwrap_or(&existing.key);
        let tags_json = tags
            .as_ref()
            .or(existing.tags.as_ref())
            .map(serde_json::to_string)
            .transpose()?;

        if let Some(new_value) = value {
            if new_value.is_empty() {
                return Err(Error::EmptyValue);
            }
            let clean = strip_private_tags(new_value);
            let clean = strip_secrets(&clean, &self.config.privacy);
            let clean = if clean.len() > self.config.validation.max_value_length {
                let mut truncated =
                    safe_truncate(&clean, self.config.validation.max_value_length - 3).to_string();
                truncated.push_str("...");
                truncated
            } else {
                clean
            };
            let hash = compute_hash(final_key, &existing.scope, &clean);

            // Dedup check: if another active memory already has this hash+scope, reject.
            let dup = self.conn().query_row(
                "SELECT id FROM memories WHERE normalized_hash = ?1 AND scope = ?2 AND deleted_at IS NULL AND id != ?3",
                params![hash, existing.scope, id],
                |row| row.get::<_, i64>(0),
            );
            if let Ok(dup_id) = dup {
                return Err(Error::Duplicate(dup_id));
            }

            self.conn().execute(
                "UPDATE memories SET key = ?1, value = ?2, tags = ?3, normalized_hash = ?4,
                 revision_count = revision_count + 1,
                 accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?5",
                params![final_key, clean, tags_json, hash, id],
            )?;
        } else {
            // Update key and/or tags without changing value
            let needs_update = key.is_some() || tags.is_some();
            if needs_update {
                // Recompute hash if key changed (hash includes key)
                let new_hash = compute_hash(final_key, &existing.scope, &existing.value);

                // Dedup check: if another active memory already has this hash+scope, reject.
                if key.is_some() {
                    let dup = self.conn().query_row(
                        "SELECT id FROM memories WHERE normalized_hash = ?1 AND scope = ?2 AND deleted_at IS NULL AND id != ?3",
                        params![new_hash, existing.scope, id],
                        |row| row.get::<_, i64>(0),
                    );
                    if let Ok(dup_id) = dup {
                        return Err(Error::Duplicate(dup_id));
                    }
                }

                self.conn().execute(
                    "UPDATE memories SET key = ?1, tags = ?2, normalized_hash = ?3,
                     revision_count = revision_count + 1,
                     accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?4",
                    params![final_key, tags_json, new_hash, id],
                )?;
            }
        }

        self.get(id)
    }

    pub fn list(
        &self,
        scope: Option<&str>,
        source_type: Option<&SourceType>,
        limit: Option<i32>,
    ) -> Result<Vec<Memory>> {
        let limit = limit
            .unwrap_or(self.config.search.default_limit as i32)
            .min(self.config.search.max_limit as i32);

        let mut sql = String::from(
            "SELECT id, key, value, scope, source_type, source_ref, source_commit,
             confidence, tags, revision_count, duplicate_count,
             created_at, accessed_at, last_seen_at
             FROM memories WHERE deleted_at IS NULL \
             AND id NOT IN (SELECT target_id FROM relations WHERE relation_type = 'supersedes')",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = scope {
            let normalized = normalize_scope(s);
            let ancestors = scope_ancestors(&normalized);
            let placeholders: Vec<String> = ancestors
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + 1 + i))
                .collect();
            sql.push_str(&format!(" AND scope IN ({})", placeholders.join(",")));
            for a in ancestors {
                param_values.push(Box::new(a));
            }
        }

        if let Some(st) = source_type {
            sql.push_str(&format!(" AND source_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(st.to_string()));
        }

        sql.push_str(&format!(
            " ORDER BY length(scope) DESC, accessed_at DESC LIMIT ?{}",
            param_values.len() + 1
        ));
        param_values.push(Box::new(limit));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn().prepare(&sql)?;
        let results = stmt
            .query_map(param_refs.as_slice(), row_to_memory)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    pub fn list_all(
        &self,
        scope: Option<&str>,
        source_type: Option<&SourceType>,
    ) -> Result<Vec<Memory>> {
        let mut sql = String::from(
            "SELECT m.id, m.key, m.value, m.scope, m.source_type, m.source_ref, m.source_commit,
             m.confidence, m.tags, m.revision_count, m.duplicate_count,
             m.created_at, m.accessed_at, m.last_seen_at
             FROM memories m
             LEFT JOIN metrics mt ON mt.memory_id = m.id
             WHERE m.deleted_at IS NULL \
             AND m.id NOT IN (SELECT target_id FROM relations WHERE relation_type = 'supersedes')",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = scope {
            let normalized = normalize_scope(s);
            let ancestors = scope_ancestors(&normalized);
            let placeholders: Vec<String> = ancestors
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + 1 + i))
                .collect();
            sql.push_str(&format!(" AND m.scope IN ({})", placeholders.join(",")));
            for a in ancestors {
                param_values.push(Box::new(a));
            }
        }

        if let Some(st) = source_type {
            sql.push_str(&format!(" AND m.source_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(st.to_string()));
        }

        // Composite ranking: scope specificity + hit rate boost + recency
        // hit_rate_score: 0.0-1.0 based on hits/injections (0 if no data)
        // recency_score: 1.0 for today, decays over 30 days
        sql.push_str(
            " ORDER BY length(m.scope) DESC,
             (CASE WHEN COALESCE(mt.injections, 0) > 0
                   THEN CAST(COALESCE(mt.hits, 0) AS REAL) / mt.injections
                   ELSE 0.5 END) * 0.3
             + (1.0 / (1.0 + julianday('now') - julianday(m.accessed_at))) * 0.7
             DESC",
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn().prepare(&sql)?;
        let results = stmt
            .query_map(param_refs.as_slice(), row_to_memory)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    pub fn context(&self, scope: Option<&str>, limit: Option<i32>) -> Result<Vec<Memory>> {
        let limit = limit.unwrap_or(10).min(self.config.search.max_limit as i32);

        let memories = self.list_all(scope, None)?;

        if scope.is_none() {
            // No scope hierarchy to dedup — just truncate
            return Ok(memories.into_iter().take(limit as usize).collect());
        }

        // Nearest-match dedup: for same key at multiple scopes, keep most specific
        // list() already orders by length(scope) DESC, so first occurrence is most specific
        let mut seen_keys: HashSet<String> = HashSet::new();
        let deduped: Vec<Memory> = memories
            .into_iter()
            .filter(|m| seen_keys.insert(m.key.clone()))
            .take(limit as usize)
            .collect();

        Ok(deduped)
    }

    pub fn apply_confidence_decay(&self) -> Result<u32> {
        let updated = self.conn().execute(
            "UPDATE memories SET confidence = MAX(0.0, confidence - (
                CASE source_type
                    WHEN 'observed' THEN 0.1
                    WHEN 'derived' THEN 0.05
                    ELSE 0.0
                END * (julianday('now') - julianday(last_seen_at)) / 7.0
            ))
            WHERE source_type IN ('observed', 'derived')
            AND deleted_at IS NULL
            AND confidence > 0.0",
            [],
        )?;
        Ok(updated as u32)
    }

    pub fn search_by_tags(
        &self,
        tags: &[&str],
        scope: Option<&str>,
        limit: i32,
    ) -> Result<Vec<SearchResult>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.min(self.config.search.max_limit as i32);

        // Build a query that checks all tags are present using json_each
        // For each tag, require that it exists in the json array
        let mut sql = String::from(
            "SELECT m.id, m.key, m.value, m.scope, m.source_type, m.confidence
             FROM memories m
             WHERE m.deleted_at IS NULL AND m.tags IS NOT NULL \
             AND m.id NOT IN (SELECT target_id FROM relations WHERE relation_type = 'supersedes')",
        );

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        for tag in tags {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM json_each(m.tags) WHERE value = ?{})",
                param_values.len() + 1
            ));
            param_values.push(Box::new(tag.to_string()));
        }

        if let Some(s) = scope {
            let normalized = normalize_scope(s);
            let ancestors = scope_ancestors(&normalized);
            let placeholders: Vec<String> = ancestors
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + 1 + i))
                .collect();
            sql.push_str(&format!(" AND m.scope IN ({})", placeholders.join(",")));
            for a in ancestors {
                param_values.push(Box::new(a));
            }
        }

        sql.push_str(&format!(
            " ORDER BY length(m.scope) DESC, m.accessed_at DESC LIMIT ?{}",
            param_values.len() + 1
        ));
        param_values.push(Box::new(limit));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn().prepare(&sql)?;
        let results = stmt
            .query_map(param_refs.as_slice(), |row| {
                let value: String = row.get(2)?;
                let preview = make_preview(&value, 200);
                let source_str: String = row.get(4)?;
                Ok(SearchResult {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value_preview: preview,
                    scope: row.get(3)?,
                    source_type: source_str.parse().unwrap_or(SourceType::Explicit),
                    confidence: row.get(5)?,
                    rank: 0.0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Hard-delete soft-deleted memories older than `retention_days` days.
    /// Returns the number of rows deleted.
    pub fn purge_soft_deleted(&self, retention_days: u32) -> Result<u32> {
        let count = self.conn().execute(
            "DELETE FROM memories WHERE deleted_at IS NOT NULL
             AND julianday('now') - julianday(deleted_at) > ?1",
            params![retention_days as f64],
        )?;
        Ok(count as u32)
    }

    /// Run VACUUM and record the timestamp in `_metadata`.
    pub fn vacuum(&self) -> Result<()> {
        self.conn().execute_batch("VACUUM")?;
        let now =
            self.conn()
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
                    row.get::<_, String>(0)
                })?;
        self.set_metadata("last_vacuum_at", &now)?;
        Ok(())
    }

    /// Return sorted distinct scopes of non-deleted memories.
    pub fn distinct_scopes(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT scope FROM memories WHERE deleted_at IS NULL ORDER BY scope",
        )?;
        let results = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Return a snapshot of maintenance-relevant statistics.
    pub fn maintenance_status(&self) -> Result<crate::types::MaintenanceStatus> {
        let last_vacuum_at = self
            .get_metadata("last_vacuum_at")?
            .unwrap_or_else(|| "never".to_string());

        let vacuum_overdue = if last_vacuum_at == "never" {
            true
        } else {
            self.conn()
                .query_row(
                    "SELECT julianday('now') - julianday(?1) > ?2",
                    params![
                        last_vacuum_at,
                        self.config.storage.vacuum_interval_secs as f64 / 86400.0,
                    ],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(true)
        };

        let purge_candidates = self
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE deleted_at IS NOT NULL
             AND julianday('now') - julianday(deleted_at) > ?1",
                params![self.config.storage.retention_days as f64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as u32;

        Ok(crate::types::MaintenanceStatus {
            vacuum_overdue,
            last_vacuum_at,
            purge_candidates,
        })
    }

    pub fn list_by_source_commit(&self) -> Result<Vec<Memory>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, key, value, scope, source_type, source_ref, source_commit,
             confidence, tags, revision_count, duplicate_count,
             created_at, accessed_at, last_seen_at
             FROM memories WHERE source_commit IS NOT NULL AND deleted_at IS NULL \
             AND id NOT IN (SELECT target_id FROM relations WHERE relation_type = 'supersedes')",
        )?;
        let results = stmt
            .query_map([], row_to_memory)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }
}

fn validate_save_params(
    params: &SaveParams,
    config: &crate::config::ValidationConfig,
) -> Result<()> {
    if params.key.is_empty() || params.key.trim().is_empty() {
        return Err(Error::EmptyKey);
    }
    if params.key.len() > config.max_key_length {
        return Err(Error::KeyTooLong(params.key.len(), config.max_key_length));
    }
    if params.key.contains('\0') {
        return Err(Error::InvalidInput("null byte in key".to_string()));
    }
    if params.value.is_empty() || params.value.trim().is_empty() {
        return Err(Error::EmptyValue);
    }
    if let Some(ref tags) = params.tags {
        if tags.len() > config.max_tags {
            return Err(Error::TooManyTags(tags.len(), config.max_tags));
        }
        for tag in tags {
            if tag.len() > config.max_tag_length {
                return Err(Error::TagTooLong(tag.len(), config.max_tag_length));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::store::relations::RelationType;
    use crate::store::Store;
    use crate::types::{SaveParams, SourceType};

    fn make_memory(store: &Store, key: &str) -> i64 {
        store
            .save(SaveParams {
                key: key.to_string(),
                value: format!("value for {key}"),
                ..Default::default()
            })
            .unwrap()
            .id()
    }

    // --- purge_soft_deleted tests ---

    #[test]
    fn purge_soft_deleted_removes_past_retention() {
        let store = Store::open_in_memory().unwrap();
        let id = store
            .save(SaveParams {
                key: "purge-me".to_string(),
                value: "some value here to pass entropy".to_string(),
                ..Default::default()
            })
            .unwrap()
            .id();
        store.delete_by_id(id, false).unwrap();
        // With retention_days=0, anything deleted should be purged
        let removed = store.purge_soft_deleted(0).unwrap();
        assert_eq!(removed, 1);
    }

    #[test]
    fn purge_soft_deleted_keeps_within_retention() {
        let store = Store::open_in_memory().unwrap();
        let id = store
            .save(SaveParams {
                key: "keep-me".to_string(),
                value: "some value here to pass entropy".to_string(),
                ..Default::default()
            })
            .unwrap()
            .id();
        store.delete_by_id(id, false).unwrap();
        // With retention_days=999, nothing should be purged yet
        let removed = store.purge_soft_deleted(999).unwrap();
        assert_eq!(removed, 0);
    }

    // --- vacuum tests ---

    #[test]
    fn vacuum_sets_last_vacuum_at_metadata() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.get_metadata("last_vacuum_at").unwrap().is_none());
        store.vacuum().unwrap();
        let ts = store.get_metadata("last_vacuum_at").unwrap();
        assert!(ts.is_some(), "last_vacuum_at should be set after vacuum");
    }

    // --- distinct_scopes tests ---

    #[test]
    fn distinct_scopes_returns_sorted_unique_scopes() {
        let store = Store::open_in_memory().unwrap();
        for (key, scope) in &[("a", "/a"), ("b", "/b"), ("c", "/")] {
            store
                .save(SaveParams {
                    key: key.to_string(),
                    value: "value that passes entropy filter with enough content".to_string(),
                    scope: Some(scope.to_string()),
                    ..Default::default()
                })
                .unwrap();
        }
        let scopes = store.distinct_scopes().unwrap();
        assert_eq!(scopes, vec!["/", "/a", "/b"]);
    }

    // --- maintenance_status tests ---

    #[test]
    fn maintenance_status_fresh_db_vacuum_overdue() {
        let store = Store::open_in_memory().unwrap();
        let status = store.maintenance_status().unwrap();
        assert!(
            status.vacuum_overdue,
            "fresh DB has never been vacuumed, should be overdue"
        );
        assert_eq!(status.purge_candidates, 0);
    }

    // --- entropy filter tests ---

    #[test]
    fn entropy_filter_rejects_low_info_observed() {
        let store = Store::open_in_memory().unwrap();
        let result = store.save(SaveParams {
            key: "low-info".to_string(),
            value: "hello".to_string(),
            source_type: Some(SourceType::Observed),
            ..Default::default()
        });
        assert!(
            matches!(result, Err(crate::Error::LowInformation(_))),
            "low-info observed save should be rejected, got: {:?}",
            result
        );
    }

    #[test]
    fn entropy_filter_allows_explicit_source() {
        let store = Store::open_in_memory().unwrap();
        // "hello" is low-info but Explicit bypasses the filter
        let result = store.save(SaveParams {
            key: "explicit-low-info".to_string(),
            value: "hello".to_string(),
            source_type: Some(SourceType::Explicit),
            ..Default::default()
        });
        assert!(
            result.is_ok(),
            "explicit source should bypass entropy filter"
        );
    }

    #[test]
    fn entropy_filter_allows_high_info_observed() {
        let store = Store::open_in_memory().unwrap();
        let result = store.save(SaveParams {
            key: "high-info".to_string(),
            value: "The authentication flow uses JWT tokens with RSA-256 signing. Tokens expire after 1h, refreshed via /api/auth/refresh endpoint. Signing key rotated every 90 days.".to_string(),
            source_type: Some(SourceType::Observed),
            ..Default::default()
        });
        assert!(result.is_ok(), "high-info observed save should succeed");
    }

    #[test]
    fn entropy_filter_disabled_when_threshold_zero() {
        let mut config = Config::default();
        config.storage.entropy_threshold = 0.0;
        let store = Store::open_in_memory_with_config(config).unwrap();
        let result = store.save(SaveParams {
            key: "low-info-no-filter".to_string(),
            value: "hello".to_string(),
            source_type: Some(SourceType::Observed),
            ..Default::default()
        });
        assert!(
            result.is_ok(),
            "with threshold=0, entropy filter is disabled"
        );
    }

    #[test]
    fn test_superseded_excluded_from_list() {
        let store = Store::open_in_memory().unwrap();
        let id1 = make_memory(&store, "key/one");
        let id2 = make_memory(&store, "key/two");
        let _id3 = make_memory(&store, "key/three");

        // mem1 supersedes mem2
        store
            .add_relation(id1, id2, RelationType::Supersedes)
            .unwrap();

        let listed: Vec<i64> = store
            .list(None, None, None)
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        assert!(listed.contains(&id1), "superseder should appear in list");
        assert!(
            !listed.contains(&id2),
            "superseded memory must be excluded from list"
        );
        assert!(
            listed.contains(&_id3),
            "unrelated memory should appear in list"
        );
    }

    #[test]
    fn test_superseded_excluded_from_search() {
        let store = Store::open_in_memory().unwrap();
        let id1 = store
            .save(SaveParams {
                key: "alpha/concept".to_string(),
                value: "superseder knowledge".to_string(),
                ..Default::default()
            })
            .unwrap()
            .id();
        let id2 = store
            .save(SaveParams {
                key: "alpha/old".to_string(),
                value: "superseded knowledge".to_string(),
                ..Default::default()
            })
            .unwrap()
            .id();

        store
            .add_relation(id1, id2, RelationType::Supersedes)
            .unwrap();

        let results = store
            .search(crate::types::SearchParams {
                query: "knowledge".to_string(),
                scope: None,
                source_type: None,
                limit: Some(10),
            })
            .unwrap();

        let result_ids: Vec<i64> = results.iter().map(|r| r.id).collect();
        assert!(
            result_ids.contains(&id1),
            "superseder should appear in search"
        );
        assert!(
            !result_ids.contains(&id2),
            "superseded memory must be excluded from search"
        );
    }

    #[test]
    fn test_superseded_still_accessible_via_get() {
        let store = Store::open_in_memory().unwrap();
        let id1 = make_memory(&store, "key/new");
        let id2 = make_memory(&store, "key/old");

        store
            .add_relation(id1, id2, RelationType::Supersedes)
            .unwrap();

        // Direct ID lookup must still work
        let mem = store.get(id2).unwrap();
        assert_eq!(mem.id, id2);
    }
}

fn row_to_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    let source_str: String = row.get(4).unwrap_or_default();
    // Column order: id, key, value, scope, source_type, source_ref, source_commit,
    //               confidence, tags, revision_count, duplicate_count,
    //               created_at, accessed_at, last_seen_at
    let tags_str: Option<String> = row.get(8).unwrap_or(None);
    let tags = tags_str.and_then(|s| serde_json::from_str(&s).ok());
    let id: i64 = row.get(0)?;

    let source_type = source_str.parse().unwrap_or_else(|_| {
        log::warn!(
            "unknown source_type {:?} for memory id {id}, defaulting to Explicit",
            source_str
        );
        SourceType::Explicit
    });

    Ok(Memory {
        id,
        key: row.get(1)?,
        value: row.get(2)?,
        scope: row.get(3)?,
        source_type,
        source_ref: row.get(5).unwrap_or(None),
        source_commit: row.get(6).unwrap_or(None),
        confidence: row.get(7)?,
        tags,
        revision_count: row.get(9)?,
        duplicate_count: row.get(10)?,
        created_at: row.get(11)?,
        accessed_at: row.get(12)?,
        last_seen_at: row.get(13)?,
    })
}
