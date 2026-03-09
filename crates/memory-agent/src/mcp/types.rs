use memory_core::{make_preview, Memory, SearchResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct SaveRequest {
    /// Semantic identifier (e.g., "architecture/auth-model")
    pub key: String,
    /// The knowledge to store
    pub value: String,
    /// Scope path, default "/"
    pub scope: Option<String>,
    /// explicit|codebase|observed|derived, default "explicit"
    pub source_type: Option<String>,
    /// file:lines for codebase type
    pub source_ref: Option<String>,
    /// Searchable tags
    pub tags: Option<Vec<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct SaveResponse {
    pub id: i64,
    pub action: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchRequest {
    /// Search terms
    pub query: String,
    /// Filter to scope
    pub scope: Option<String>,
    /// Filter to source type
    pub source_type: Option<String>,
    /// Default 10, max 50
    pub limit: Option<i32>,
}

#[derive(Serialize, JsonSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchResultItem {
    pub id: i64,
    pub key: String,
    pub value_preview: String,
    pub scope: String,
    pub source_type: String,
    pub confidence: f64,
    pub rank: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct DetailRequest {
    /// Memory id from search results
    pub id: i64,
}

#[derive(Serialize, JsonSchema)]
pub struct DetailResponse {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub scope: String,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub confidence: f64,
    pub tags: Option<Vec<String>>,
    pub revision_count: i32,
    pub duplicate_count: i32,
    pub created_at: String,
    pub accessed_at: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DeleteRequest {
    /// Delete by ID (preferred). If provided, key/scope are ignored.
    pub id: Option<i64>,
    /// Delete by key (requires scope or defaults to "/")
    pub key: Option<String>,
    /// Default "/"
    pub scope: Option<String>,
    /// Default false (soft delete)
    pub hard: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct DeleteResponse {
    pub deleted: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListRequest {
    pub scope: Option<String>,
    pub source_type: Option<String>,
    /// Default 20
    pub limit: Option<i32>,
}

#[derive(Serialize, JsonSchema)]
pub struct ListResponse {
    pub memories: Vec<ListItem>,
    pub total: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListItem {
    pub id: i64,
    pub key: String,
    pub value_preview: String,
    pub scope: String,
    pub source_type: String,
    pub confidence: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct ContextRequest {
    pub scope: Option<String>,
    /// Default 10
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExtractRequest {
    /// Source type: "config" (scans project config files)
    pub source: String,
    /// Project root directory
    pub directory: String,
    /// Override auto-detected scope
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExtractResponse {
    pub extracted: i32,
    pub updated: i32,
    pub skipped: i32,
    pub files_scanned: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StaleRequest {
    /// Filter to scope
    pub scope: Option<String>,
    /// Working directory for git discovery (defaults to current dir)
    pub directory: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StaleResponse {
    pub stale: Vec<StaleItem>,
    pub checked: i32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StaleItem {
    pub memory_id: i64,
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MetricsRequest {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MetricsResponse {
    pub aggregate_hit_rate: f64,
    pub total_injections: i32,
    pub total_hits: i32,
    pub top_memories: Vec<MetricsItem>,
    /// LLM-generated actionable recommendations (when LLM is available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insights: Option<Vec<String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MetricsItem {
    pub id: i64,
    pub key: String,
    pub scope: String,
    pub injections: i32,
    pub hits: i32,
    pub hit_rate: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelateRequest {
    pub source_id: i64,
    pub target_id: i64,
    /// derived_from|supersedes|conflicts_with|related_to
    pub relation: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RelateResponse {
    pub id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelationsRequest {
    pub id: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RelationsResponse {
    pub relations: Vec<RelationItem>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RelationItem {
    pub id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub relation_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConsolidateRequest {
    /// Filter to scope
    pub scope: Option<String>,
    /// Preview without merging (default true)
    pub dry_run: Option<bool>,
    /// Similarity threshold 0.0-1.0 (default 0.85)
    pub threshold: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ConsolidateResponse {
    pub groups: Vec<ConsolidationGroupItem>,
    pub consolidated: i32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ConsolidationGroupItem {
    pub key: String,
    pub memory_ids: Vec<i64>,
    pub similarity: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BudgetRequest {
    /// Maximum token budget
    pub max_tokens: i32,
    /// Filter to scope
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BudgetResponse {
    pub memories: Vec<ListItem>,
    pub tokens_used: i32,
    pub tokens_remaining: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillStartRequest {
    /// Skill name (e.g., "implementing", "debugging")
    pub skill_name: String,
    /// Optional context hint (reserved for future LLM tier, unused in Tier 1)
    #[allow(dead_code)]
    pub context: Option<String>,
    /// Filter to scope
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SkillStartResponse {
    pub procedural_memories: Vec<SearchResultItem>,
    pub overrides: Vec<SearchResultItem>,
    pub context_memories: Vec<ListItem>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillEndRequest {
    /// Skill name matching the skill_start call
    pub skill_name: String,
    /// Outcome description; may contain Pattern:/Learned:/Takeaway: headers
    pub outcome: String,
    /// Files modified during the skill execution
    pub files_changed: Option<Vec<String>>,
    /// Filter to scope
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SkillEndResponse {
    /// ID of the created procedural memory, or null if outcome was trivial
    pub procedural_memory_id: Option<i64>,
}

impl From<&SearchResult> for SearchResultItem {
    fn from(r: &SearchResult) -> Self {
        Self {
            id: r.id,
            key: r.key.clone(),
            value_preview: r.value_preview.clone(),
            scope: r.scope.clone(),
            source_type: r.source_type.to_string(),
            confidence: r.confidence,
            rank: r.rank,
        }
    }
}

impl From<&Memory> for ListItem {
    fn from(m: &Memory) -> Self {
        Self {
            id: m.id,
            key: m.key.clone(),
            value_preview: make_preview(&m.value, 200),
            scope: m.scope.clone(),
            source_type: m.source_type.to_string(),
            confidence: m.confidence,
        }
    }
}
