use std::sync::{Arc, Mutex};

use memory_core::{
    Memory, SaveAction, SaveParams, SearchParams, SearchResult, Store,
    types::{EventLogEntry, SourceType},
};

fn acquire_lock(
    store: &Mutex<Store>,
) -> anyhow::Result<std::sync::MutexGuard<'_, Store>> {
    store
        .lock()
        .map_err(|_| anyhow::anyhow!("store mutex poisoned"))
}

#[derive(Clone)]
pub struct AsyncStore {
    inner: Arc<Mutex<Store>>,
}

impl AsyncStore {
    pub fn new(store: Store) -> Self {
        Self {
            inner: Arc::new(Mutex::new(store)),
        }
    }

    pub async fn save(&self, params: SaveParams) -> anyhow::Result<SaveAction> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.save(params)?)
        })
        .await?
    }

    pub async fn get(&self, id: i64) -> anyhow::Result<Memory> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.get(id)?)
        })
        .await?
    }

    pub async fn search(&self, params: SearchParams) -> anyhow::Result<Vec<SearchResult>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.search(params)?)
        })
        .await?
    }

    pub async fn delete(
        &self,
        key: String,
        scope: Option<String>,
        hard: bool,
    ) -> anyhow::Result<bool> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.delete(&key, scope.as_deref(), hard)?)
        })
        .await?
    }

    pub async fn delete_by_id(
        &self,
        id: i64,
        hard: bool,
    ) -> anyhow::Result<bool> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.delete_by_id(id, hard)?)
        })
        .await?
    }

    pub async fn list(
        &self,
        scope: Option<String>,
        source_type: Option<SourceType>,
        limit: Option<i32>,
    ) -> anyhow::Result<Vec<Memory>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.list(scope.as_deref(), source_type.as_ref(), limit)?)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn list_all(
        &self,
        scope: Option<String>,
        source_type: Option<SourceType>,
    ) -> anyhow::Result<Vec<Memory>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.list_all(scope.as_deref(), source_type.as_ref())?)
        })
        .await?
    }

    pub async fn context(
        &self,
        scope: Option<String>,
        limit: Option<i32>,
    ) -> anyhow::Result<Vec<Memory>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.context(scope.as_deref(), limit)?)
        })
        .await?
    }

    pub async fn write_event(&self, action: String, key: String, scope: String, tokens: i32) -> anyhow::Result<()> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.write_event(&action, &key, &scope, tokens)?)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn recent_events(&self, limit: i32) -> anyhow::Result<Vec<EventLogEntry>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.recent_events(limit)?)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn events_today_summary(&self) -> anyhow::Result<(i64, i64, i64, i64)> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.events_today_summary()?)
        })
        .await?
    }

    pub async fn list_by_source_commit(&self) -> anyhow::Result<Vec<Memory>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.list_by_source_commit()?)
        })
        .await?
    }

    pub async fn record_injection(&self, memory_ids: Vec<i64>, tokens_per_memory: i32) -> anyhow::Result<()> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.record_injection(&memory_ids, tokens_per_memory)?)
        })
        .await?
    }

    pub async fn record_hit(&self, memory_id: i64) -> anyhow::Result<()> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.record_hit(memory_id)?)
        })
        .await?
    }

    pub async fn record_hit_batch(&self, memory_ids: Vec<i64>) -> anyhow::Result<()> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.record_hit_batch(&memory_ids)?)
        })
        .await?
    }

    pub async fn apply_confidence_decay(&self) -> anyhow::Result<u32> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.apply_confidence_decay()?)
        })
        .await?
    }

    pub async fn extract(
        &self,
        directory: String,
        scope_override: Option<String>,
    ) -> anyhow::Result<crate::source::extract::ExtractResult> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&directory);
            let scope = match scope_override.as_deref() {
                Some(s) => s.to_string(),
                None => crate::source::extract::scope_from_directory(dir),
            };
            let guard = acquire_lock(&store)?;
            crate::source::extract::extract_from_directory(dir, &guard, &scope)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn update(
        &self,
        id: i64,
        key: Option<String>,
        value: Option<String>,
        tags: Option<Vec<String>>,
    ) -> anyhow::Result<Memory> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.update(id, key.as_deref(), value.as_deref(), tags)?)
        })
        .await?
    }

    pub async fn get_metrics(
        &self,
    ) -> anyhow::Result<Vec<memory_core::types::MemoryMetric>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.get_metrics()?)
        })
        .await?
    }

    pub async fn add_relation(
        &self,
        source_id: i64,
        target_id: i64,
        rel_type: memory_core::store::relations::RelationType,
    ) -> anyhow::Result<i64> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.add_relation(source_id, target_id, rel_type)?)
        })
        .await?
    }

    pub async fn get_relations(
        &self,
        memory_id: i64,
    ) -> anyhow::Result<Vec<memory_core::store::relations::Relation>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.get_relations(memory_id)?)
        })
        .await?
    }

    pub async fn skill_start(
        &self,
        skill_name: String,
        scope: Option<String>,
    ) -> anyhow::Result<crate::skills::SkillStartResult> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = acquire_lock(&store)?;
            crate::skills::on_skill_start(&guard, &skill_name, scope.as_deref())
        })
        .await?
    }

    pub async fn purge_soft_deleted(&self, retention_days: u32) -> anyhow::Result<u32> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.purge_soft_deleted(retention_days)?)
        })
        .await?
    }

    pub async fn vacuum(&self) -> anyhow::Result<()> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.vacuum()?)
        })
        .await?
    }

    pub async fn maintenance_status(&self) -> anyhow::Result<memory_core::types::MaintenanceStatus> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.maintenance_status()?)
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn distinct_scopes(&self) -> anyhow::Result<Vec<String>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            Ok(store.distinct_scopes()?)
        })
        .await?
    }

    pub async fn retention_days(&self) -> u32 {
        self.inner
            .lock()
            .map(|s| s.config().storage.retention_days)
            .unwrap_or(90)
    }

    pub async fn skill_end(
        &self,
        skill_name: String,
        scope: Option<String>,
        outcome: String,
        files_changed: Vec<String>,
    ) -> anyhow::Result<Option<i64>> {
        let store = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = acquire_lock(&store)?;
            crate::skills::on_skill_end(
                &guard,
                &skill_name,
                scope.as_deref(),
                &outcome,
                &files_changed,
            )
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn consolidate(
        &self,
        scope: Option<String>,
        threshold: f64,
        dry_run: bool,
    ) -> anyhow::Result<(Vec<memory_core::autonomous::consolidation::ConsolidationGroup>, i32)>
    {
        self.consolidate_with_llm(scope, threshold, dry_run, crate::llm::LlmTier::None).await
    }

    pub async fn consolidate_with_llm(
        &self,
        scope: Option<String>,
        threshold: f64,
        dry_run: bool,
        llm: crate::llm::LlmTier,
    ) -> anyhow::Result<(Vec<memory_core::autonomous::consolidation::ConsolidationGroup>, i32)>
    {
        let store = self.inner.clone();
        // Find candidates synchronously
        let (groups, group_values) = tokio::task::spawn_blocking({
            let store = store.clone();
            move || {
                let store = acquire_lock(&store)?;
                let memories = store.list_all(scope.as_deref(), None)?;
                let tuples: Vec<(i64, String, String, String)> = memories
                    .iter()
                    .map(|m| (m.id, m.key.clone(), m.value.clone(), m.scope.clone()))
                    .collect();
                let groups =
                    memory_core::autonomous::consolidation::find_candidates(&tuples, threshold);

                let group_values: Vec<Vec<String>> = groups.iter().map(|group| {
                    group.memory_ids.iter()
                        .filter_map(|id| store.get(*id).ok().map(|m| m.value))
                        .collect()
                }).collect();

                Ok::<_, anyhow::Error>((groups, group_values))
            }
        }).await??;

        if dry_run || groups.is_empty() {
            return Ok((groups, 0));
        }

        // Merge values (using LLM if available)
        let mut merged_values = Vec::new();
        for values in &group_values {
            let refs: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
            let merged = crate::llm::compress_memories(&llm, &refs).await?;
            merged_values.push(merged);
        }

        // Save merged values synchronously
        let groups_clone = groups.clone();
        let consolidated = tokio::task::spawn_blocking(move || {
            let store = acquire_lock(&store)?;
            let mut consolidated = 0i32;
            for (group, merged) in groups_clone.iter().zip(merged_values) {
                let action = store.save(SaveParams {
                    key: group.key.clone(),
                    value: merged,
                    scope: Some(group.scope.clone()),
                    source_type: Some(memory_core::types::SourceType::Derived),
                    source_ref: None,
                    source_commit: None,
                    tags: Some(vec!["consolidated".to_string()]),
                })?;

                let new_id = action.id();
                for &old_id in &group.memory_ids {
                    if old_id != new_id {
                        let _ = store.add_relation(
                            new_id,
                            old_id,
                            memory_core::store::relations::RelationType::Supersedes,
                        );
                    }
                }
                consolidated += 1;
            }
            Ok::<_, anyhow::Error>(consolidated)
        }).await??;

        Ok((groups, consolidated))
    }
}
