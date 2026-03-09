use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::*,
    tool, tool_handler, tool_router, ErrorData as McpError, handler::server::ServerHandler,
};

use memory_core::types::parse_source_ref;

use crate::async_store::AsyncStore;
use crate::llm::LlmTier;
use crate::mcp::errors::to_mcp_error;
use crate::mcp::types::*;
use crate::source::extract::scope_from_directory;
use crate::source::git::GitContext;

fn serialization_error(e: impl std::fmt::Display) -> McpError {
    McpError::new(
        ErrorCode(-32000),
        format!("serialization error: {e}"),
        None::<serde_json::Value>,
    )
}

fn invalid_params(msg: impl Into<String>) -> McpError {
    McpError::new(ErrorCode(-32602), msg.into(), None::<serde_json::Value>)
}

fn validate_string_param(value: &str, name: &str, max_len: usize) -> Result<(), McpError> {
    if value.len() > max_len {
        return Err(invalid_params(format!("{name} exceeds maximum length of {max_len}")));
    }
    if value.contains('\0') {
        return Err(invalid_params(format!("{name} contains null bytes")));
    }
    if value.contains("..") {
        return Err(invalid_params(format!("{name} contains path traversal sequence")));
    }
    Ok(())
}

#[derive(Clone)]
pub struct MemoryServer {
    store: AsyncStore,
    llm: LlmTier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MemoryServer {
    pub fn new(store: AsyncStore, llm: LlmTier) -> Self {
        Self {
            store,
            llm,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "memory_save",
        description = "Save or update a memory. If a memory with the same key and scope exists, it will be updated (revision tracked)."
    )]
    async fn save(
        &self,
        params: Parameters<SaveRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let source_type = req
            .source_type
            .map(|s| s.parse::<memory_core::types::SourceType>())
            .transpose()
            .map_err(|e| to_mcp_error(e.into()))?;

        let scope = req.scope.clone().unwrap_or_else(|| "/".to_string());

        let result = self
            .store
            .save(memory_core::SaveParams {
                key: req.key.clone(),
                value: req.value,
                scope: req.scope,
                source_type,
                source_ref: req.source_ref,
                source_commit: None,
                tags: req.tags,
            })
            .await
            .map_err(to_mcp_error)?;

        // Write to live event feed (fire-and-forget)
        let store = self.store.clone();
        let key = req.key.clone();
        let ev_scope = scope.clone();
        tokio::spawn(async move {
            if let Err(e) = store.write_event("save".to_string(), key, ev_scope, 0).await {
                tracing::warn!("event_log write failed: {e}");
            }
        });

        let resp = SaveResponse {
            id: result.id(),
            action: result.action_str().to_string(),
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_search",
        description = "Search memories by keyword. Returns compact results -- use memory_detail for full content."
    )]
    async fn search(
        &self,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let source_type = req
            .source_type
            .map(|s| s.parse::<memory_core::types::SourceType>())
            .transpose()
            .map_err(|e| to_mcp_error(e.into()))?;

        let query_clone = req.query.clone();
        let scope_clone = req.scope.clone().unwrap_or_else(|| "/".to_string());
        let adaptive_limit = {
            let base = req.limit.unwrap_or(10);
            Some(memory_core::autonomous::adaptive::adaptive_limit(&req.query, base))
        };
        let results = self
            .store
            .search(memory_core::SearchParams {
                query: req.query,
                scope: req.scope,
                source_type,
                limit: adaptive_limit,
            })
            .await
            .map_err(to_mcp_error)?;

        let ids: Vec<i64> = results.iter().map(|r| r.id).collect();
        if !ids.is_empty() {
            let store = self.store.clone();
            let hit_ids = ids.clone();
            let query_ev = query_clone.clone();
            let scope_ev = scope_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = store.record_hit_batch(hit_ids).await {
                    tracing::warn!("record_hit_batch failed: {e}");
                }
                if let Err(e) = store.write_event("search".to_string(), query_ev, scope_ev, 0).await {
                    tracing::warn!("event_log write failed: {e}");
                }
            });
        }

        let items: Vec<SearchResultItem> = results.iter().map(SearchResultItem::from).collect();

        let resp = SearchResponse {
            total: items.len(),
            results: items,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_detail",
        description = "Get the full content of a memory by ID."
    )]
    async fn detail(
        &self,
        params: Parameters<DetailRequest>,
    ) -> Result<CallToolResult, McpError> {
        let mem = self
            .store
            .get(params.0.id)
            .await
            .map_err(to_mcp_error)?;

        let store = self.store.clone();
        let hit_id = mem.id;
        let key_ev = mem.key.clone();
        let scope_ev = mem.scope.clone();
        tokio::spawn(async move {
            if let Err(e) = store.record_hit(hit_id).await {
                tracing::warn!("record_hit failed: {e}");
            }
            if let Err(e) = store.write_event("hit".to_string(), key_ev, scope_ev, 0).await {
                tracing::warn!("event_log write failed: {e}");
            }
        });

        let resp = DetailResponse {
            id: mem.id,
            key: mem.key,
            value: mem.value,
            scope: mem.scope,
            source_type: mem.source_type.to_string(),
            source_ref: mem.source_ref,
            confidence: mem.confidence,
            tags: mem.tags,
            revision_count: mem.revision_count,
            duplicate_count: mem.duplicate_count,
            created_at: mem.created_at,
            accessed_at: mem.accessed_at,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_delete",
        description = "Delete a memory by ID or by key+scope. Soft-deletes by default."
    )]
    async fn delete(
        &self,
        params: Parameters<DeleteRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let hard = req.hard.unwrap_or(false);

        let deleted = match (req.id, req.key) {
            (Some(_), Some(_)) => {
                return Err(McpError::new(
                    ErrorCode(-32602),
                    "provide either 'id' or 'key', not both".to_string(),
                    None::<serde_json::Value>,
                ));
            }
            (Some(id), None) => self.store.delete_by_id(id, hard).await.map_err(to_mcp_error)?,
            (None, Some(key)) => self
                .store
                .delete(key, req.scope, hard)
                .await
                .map_err(to_mcp_error)?,
            (None, None) => {
                return Err(McpError::new(
                    ErrorCode(-32602),
                    "provide either 'id' or 'key'".to_string(),
                    None::<serde_json::Value>,
                ));
            }
        };

        let resp = DeleteResponse { deleted };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_list",
        description = "List memories with optional scope and source type filters."
    )]
    async fn list(
        &self,
        params: Parameters<ListRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let source_type = req
            .source_type
            .map(|s| s.parse::<memory_core::types::SourceType>())
            .transpose()
            .map_err(|e| to_mcp_error(e.into()))?;

        let memories = self
            .store
            .list(req.scope, source_type, req.limit)
            .await
            .map_err(to_mcp_error)?;

        let items: Vec<ListItem> = memories.iter().map(ListItem::from).collect();

        let resp = ListResponse {
            total: items.len(),
            memories: items,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_context",
        description = "Get recent relevant memories for context injection. Returns scope-aware results with nearest-match dedup."
    )]
    async fn context(
        &self,
        params: Parameters<ContextRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let scope_ev = req.scope.clone().unwrap_or_else(|| "/".to_string());

        let memories = self
            .store
            .context(req.scope, req.limit)
            .await
            .map_err(to_mcp_error)?;

        let ids: Vec<i64> = memories.iter().map(|m| m.id).collect();

        let items: Vec<ListItem> = memories.iter().map(ListItem::from).collect();
        let resp = ListResponse {
            total: items.len(),
            memories: items,
        };

        let tokens_injected = if !ids.is_empty() {
            serde_json::to_string(&resp)
                .map(|s| (s.len() / 4) as i32)
                .unwrap_or(0)
        } else {
            0
        };

        if !ids.is_empty() {
            let store = self.store.clone();
            let hit_ids = ids.clone();
            let tokens_per_memory = tokens_injected / ids.len() as i32;
            let count_key = format!("{} memories", ids.len());
            tokio::spawn(async move {
                if let Err(e) = store.record_injection(hit_ids, tokens_per_memory).await {
                    tracing::warn!("record_injection failed: {e}");
                }
                if let Err(e) = store.write_event("inject".to_string(), count_key, scope_ev, tokens_injected).await {
                    tracing::warn!("event_log write failed: {e}");
                }
            });
        }

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_stale",
        description = "Check which codebase memories have become stale because their source files changed in git since they were recorded."
    )]
    async fn stale(
        &self,
        params: Parameters<StaleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;

        if let Some(ref dir) = req.directory {
            validate_string_param(dir, "directory", 512)?;
        }

        let dir = req
            .directory
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));

        let memories = self
            .store
            .list_by_source_commit()
            .await
            .map_err(to_mcp_error)?;

        let checked = memories.len() as i32;

        let git = GitContext::open(&dir);
        let stale_items = if let Some(git) = git {
            let mut by_commit: std::collections::HashMap<String, Vec<&memory_core::types::Memory>> =
                std::collections::HashMap::new();
            for m in &memories {
                if let Some(scope_filter) = &req.scope {
                    if !m.scope.starts_with(scope_filter.as_str()) {
                        continue;
                    }
                }
                if let Some(ref commit) = m.source_commit {
                    by_commit.entry(commit.clone()).or_default().push(m);
                }
            }

            let mut items = Vec::new();
            for (commit, mems) in &by_commit {
                let changed = match git.changed_files(commit) {
                    Ok(files) => files,
                    Err(_) => continue,
                };
                for m in mems {
                    if let Some(ref source_ref) = m.source_ref {
                        let (file, _, _) = parse_source_ref(source_ref);
                        if changed.contains(&file) {
                            items.push(StaleItem {
                                memory_id: m.id,
                                key: m.key.clone(),
                                reason: format!("file '{file}' changed since commit {commit}"),
                            });
                        }
                    }
                }
            }
            items
        } else {
            Vec::new()
        };

        let resp = StaleResponse {
            stale: stale_items,
            checked,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_extract",
        description = "Extract memories from project configuration files (package.json, Cargo.toml, CLAUDE.md, etc). Run at the start of a project to auto-populate the memory system."
    )]
    async fn extract(
        &self,
        params: Parameters<ExtractRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        if req.source != "config" {
            return Err(McpError::new(
                ErrorCode(-32602),
                format!("unsupported source: {}. Only 'config' is supported.", req.source),
                None::<serde_json::Value>,
            ));
        }

        validate_string_param(&req.directory, "directory", 512)?;

        let dir = std::path::Path::new(&req.directory);
        let scope = req.scope.unwrap_or_else(|| scope_from_directory(dir));

        let result = self
            .store
            .extract(req.directory, Some(scope))
            .await
            .map_err(to_mcp_error)?;

        let resp = ExtractResponse {
            extracted: result.extracted,
            updated: result.updated,
            skipped: result.skipped,
            files_scanned: result.files_scanned,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_metrics",
        description = "Get token efficiency metrics. Shows which memories are earning their tokens and which should be removed."
    )]
    async fn metrics(
        &self,
        _params: Parameters<MetricsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let metrics = self
            .store
            .get_metrics()
            .await
            .map_err(to_mcp_error)?;

        let total_injections: i32 = metrics.iter().map(|m| m.injections).sum();
        let total_hits: i32 = metrics.iter().map(|m| m.hits).sum();
        let aggregate_hit_rate = if total_injections > 0 {
            total_hits as f64 / total_injections as f64
        } else {
            0.0
        };

        let top: Vec<MetricsItem> = metrics
            .iter()
            .take(20)
            .map(|m| MetricsItem {
                id: m.id,
                key: m.key.clone(),
                scope: m.scope.clone(),
                injections: m.injections,
                hits: m.hits,
                hit_rate: m.hit_rate,
            })
            .collect();

        let insights = if self.llm.is_available() {
            let summary = format_metrics_for_llm(&top, aggregate_hit_rate);
            crate::llm::analyze_metrics(&self.llm, &summary)
                .await
                .ok()
                .flatten()
                .map(|s| s.lines()
                    .map(|l| l.trim().trim_start_matches("- ").to_string())
                    .filter(|l| !l.is_empty())
                    .collect())
        } else {
            Some(generate_rule_based_insights(&top, aggregate_hit_rate))
                .filter(|v: &Vec<String>| !v.is_empty())
        };

        let resp = MetricsResponse {
            aggregate_hit_rate,
            total_injections,
            total_hits,
            top_memories: top,
            insights,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_relate",
        description = "Create a relationship between two memories (derived_from, supersedes, conflicts_with, related_to)."
    )]
    async fn relate(
        &self,
        params: Parameters<RelateRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let rel_type: memory_core::store::relations::RelationType = req
            .relation
            .parse()
            .map_err(|e: memory_core::Error| to_mcp_error(e.into()))?;

        let id = self
            .store
            .add_relation(req.source_id, req.target_id, rel_type)
            .await
            .map_err(to_mcp_error)?;

        let resp = RelateResponse { id };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_relations",
        description = "Get all relationships for a memory."
    )]
    async fn relations(
        &self,
        params: Parameters<RelationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let relations = self
            .store
            .get_relations(params.0.id)
            .await
            .map_err(to_mcp_error)?;

        let items: Vec<RelationItem> = relations
            .iter()
            .map(|r| RelationItem {
                id: r.id,
                source_id: r.source_id,
                target_id: r.target_id,
                relation_type: r.relation_type.to_string(),
                created_at: r.created_at.clone(),
            })
            .collect();

        let resp = RelationsResponse { relations: items };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_consolidate",
        description = "Find and merge similar memories to reduce redundancy. Memories with high term overlap at the same scope are merged. Uses LLM for semantic compression when available. Default is dry_run (preview only)."
    )]
    async fn consolidate(
        &self,
        params: Parameters<ConsolidateRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let threshold = req.threshold.unwrap_or(0.85);
        if !(0.0..=1.0).contains(&threshold) {
            return Err(McpError::new(
                ErrorCode(-32602),
                "threshold must be between 0.0 and 1.0".to_string(),
                None::<serde_json::Value>,
            ));
        }
        let dry_run = req.dry_run.unwrap_or(true);

        let llm = self.llm.clone();
        let (groups, consolidated) = self
            .store
            .consolidate_with_llm(req.scope, threshold, dry_run, llm)
            .await
            .map_err(to_mcp_error)?;

        let items: Vec<ConsolidationGroupItem> = groups
            .iter()
            .map(|g| ConsolidationGroupItem {
                key: g.key.clone(),
                memory_ids: g.memory_ids.clone(),
                similarity: g.similarity,
            })
            .collect();

        let resp = ConsolidateResponse {
            groups: items,
            consolidated,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_skill_start",
        description = "Retrieve procedural memories, preference overrides, and context for a named skill before execution."
    )]
    async fn skill_start(
        &self,
        params: Parameters<SkillStartRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        if req.skill_name.trim().is_empty() {
            return Err(McpError::new(
                ErrorCode(-32602),
                "skill_name must not be empty".to_string(),
                None::<serde_json::Value>,
            ));
        }
        validate_string_param(&req.skill_name, "skill_name", 256)?;
        let scope_ev = req.scope.clone().unwrap_or_else(|| "/".to_string());
        let result = self
            .store
            .skill_start(req.skill_name.clone(), req.scope)
            .await
            .map_err(to_mcp_error)?;

        let procedural_memories: Vec<SearchResultItem> =
            result.procedural_memories.iter().map(SearchResultItem::from).collect();
        let overrides: Vec<SearchResultItem> =
            result.overrides.iter().map(SearchResultItem::from).collect();
        let context_memories: Vec<ListItem> =
            result.context_memories.iter().map(ListItem::from).collect();

        let mut hit_ids: Vec<i64> = context_memories.iter().map(|m| m.id).collect();
        hit_ids.extend(procedural_memories.iter().map(|m| m.id));
        if !hit_ids.is_empty() {
            let store = self.store.clone();
            let tokens_per_memory = {
                let total_bytes = context_memories.iter().map(|m| m.value_preview.len()).sum::<usize>()
                    + procedural_memories.iter().map(|m| m.value_preview.len()).sum::<usize>();
                let n = hit_ids.len();
                if n > 0 { (total_bytes / 4 / n) as i32 } else { 0 }
            };
            let total_tokens = tokens_per_memory * hit_ids.len() as i32;
            let count_key = format!("{} memories", hit_ids.len());
            let skill_scope = scope_ev.clone();
            tokio::spawn(async move {
                if let Err(e) = store.record_injection(hit_ids, tokens_per_memory).await {
                    tracing::warn!("record_injection failed: {e}");
                }
                if let Err(e) = store.write_event("inject".to_string(), count_key, skill_scope, total_tokens).await {
                    tracing::warn!("event_log write failed: {e}");
                }
            });
        }

        let resp = SkillStartResponse {
            procedural_memories,
            overrides,
            context_memories,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_skill_end",
        description = "Record the outcome of a skill execution. Extracts and stores procedural memories from outcome text. Uses LLM for enhanced extraction when available."
    )]
    async fn skill_end(
        &self,
        params: Parameters<SkillEndRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        if req.skill_name.trim().is_empty() {
            return Err(McpError::new(
                ErrorCode(-32602),
                "skill_name must not be empty".to_string(),
                None::<serde_json::Value>,
            ));
        }
        validate_string_param(&req.skill_name, "skill_name", 256)?;
        let files = req.files_changed.unwrap_or_default();

        let enhanced = crate::llm::enhance_procedural(&self.llm, &req.skill_name, &req.outcome)
            .await
            .unwrap_or(None);

        let outcome = enhanced.unwrap_or(req.outcome);
        let id = self
            .store
            .skill_end(req.skill_name, req.scope, outcome, files)
            .await
            .map_err(to_mcp_error)?;

        let resp = SkillEndResponse {
            procedural_memory_id: id,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }

    #[tool(
        name = "memory_budget",
        description = "Get the most valuable memories that fit within a token budget. Use at conversation start to inject optimal context."
    )]
    async fn budget(
        &self,
        params: Parameters<BudgetRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        if req.max_tokens <= 0 {
            return Err(McpError::new(
                ErrorCode(-32602),
                "max_tokens must be positive".to_string(),
                None::<serde_json::Value>,
            ));
        }
        let scope_ev = req.scope.clone().unwrap_or_else(|| "/".to_string());

        let memories = self
            .store
            .context(req.scope, Some(100))
            .await
            .map_err(to_mcp_error)?;

        let mut selected = Vec::new();
        let mut tokens_used = 0i32;

        for m in &memories {
            let estimated = memory_core::autonomous::adaptive::estimate_tokens(&m.value);
            if tokens_used + estimated > req.max_tokens && !selected.is_empty() {
                break;
            }
            tokens_used += estimated;
            selected.push(ListItem::from(m));
        }

        let hit_ids: Vec<i64> = selected.iter().map(|s| s.id).collect();
        if !hit_ids.is_empty() {
            let store = self.store.clone();
            let tokens_per_memory = tokens_used / hit_ids.len() as i32;
            let count_key = format!("{} memories", hit_ids.len());
            let budget_tokens = tokens_used;
            tokio::spawn(async move {
                if let Err(e) = store.record_injection(hit_ids, tokens_per_memory).await {
                    tracing::warn!("record_injection failed: {e}");
                }
                if let Err(e) = store.write_event("inject".to_string(), count_key, scope_ev, budget_tokens).await {
                    tracing::warn!("event_log write failed: {e}");
                }
            });
        }

        let resp = BudgetResponse {
            memories: selected,
            tokens_used,
            tokens_remaining: req.max_tokens - tokens_used,
        };

        Ok(CallToolResult::success(vec![Content::json(resp).map_err(
            serialization_error,
        )?]))
    }
}

fn format_metrics_for_llm(metrics: &[MetricsItem], aggregate_hit_rate: f64) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "Aggregate hit rate: {:.1}%", aggregate_hit_rate * 100.0);
    let _ = writeln!(s, "Top memories by injection count:");
    for m in metrics.iter().take(15) {
        let _ = writeln!(
            s,
            "  [{}] {} (scope: {}) — injections: {}, hits: {}, hit_rate: {:.0}%",
            m.id, m.key, m.scope, m.injections, m.hits, m.hit_rate * 100.0
        );
    }
    s
}

fn generate_rule_based_insights(metrics: &[MetricsItem], aggregate_hit_rate: f64) -> Vec<String> {
    let mut insights = Vec::new();

    let zero_hit: Vec<&MetricsItem> = metrics
        .iter()
        .filter(|m| m.injections >= 3 && m.hits == 0)
        .collect();
    if !zero_hit.is_empty() {
        let keys: Vec<&str> = zero_hit.iter().take(3).map(|m| m.key.as_str()).collect();
        insights.push(format!(
            "DELETE: {} memories injected 3+ times with zero hits (wasting tokens): {}",
            zero_hit.len(),
            keys.join(", ")
        ));
    }

    if aggregate_hit_rate < 0.1 && metrics.iter().any(|m| m.injections > 0) {
        insights.push(
            "REWRITE: Overall hit rate below 10%. Memories may be too generic or poorly keyed."
                .to_string(),
        );
    }

    let high_value: Vec<&MetricsItem> = metrics
        .iter()
        .filter(|m| m.hit_rate > 0.5 && m.hits >= 2)
        .collect();
    if !high_value.is_empty() {
        let keys: Vec<&str> = high_value.iter().take(3).map(|m| m.key.as_str()).collect();
        insights.push(format!(
            "KEEP: {} high-value memories (>50% hit rate): {}",
            high_value.len(),
            keys.join(", ")
        ));
    }

    insights
}

#[tool_handler]
impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Memory agent: persistent memory system for AI coding agents. \
                 Use memory_save to store knowledge, memory_search to find it, \
                 memory_detail for full content.\n\n\
                 PROACTIVE USAGE — call these tools without being asked:\n\
                 - memory_search: BEFORE modifying unfamiliar code, search for relevant memories\n\
                 - memory_save: AFTER solving a bug, making an architecture decision, or discovering a pattern, save it\n\
                 - memory_context: at conversation start, load project context for the current scope\n\
                 - memory_budget: use instead of memory_context when token budget is tight\n\n\
                 SUBAGENTS: Hooks do not fire inside subagents — they start with no memory context. \
                 When dispatching a subagent (Agent or Task tool), call memory_context for the current \
                 project scope first, then include the results in the subagent prompt.\n\n\
                 The hook system injects memories into your context automatically, but those are previews. \
                 Call memory_detail to get full content when a preview is relevant to your task. \
                 Call memory_search when you need to find something the hooks didn't surface."
                    .to_string(),
            ),
        }
    }
}
