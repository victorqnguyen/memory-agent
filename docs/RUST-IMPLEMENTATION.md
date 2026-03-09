# Rust Implementation Guide

## Why Rust Over Go/TypeScript

| Metric | Rust | Go (Engram) | TypeScript |
|-|-|-|-|
| Binary size | ~3MB | ~12MB | N/A (needs Node) |
| RAM at idle | ~3-5MB | ~10-15MB | ~50-80MB |
| Startup | ~1ms | ~5ms | ~200-500ms |
| WASM target | First-class | Experimental | N/A |
| SQLite bindings | rusqlite (C FFI, fastest) | modernc (pure Go, slower) | better-sqlite3 (native addon) |
| Embeddable | Yes (C ABI, WASM) | No | No |
| Safety | Compile-time memory safety | GC | Runtime errors |

**The WASM angle is the strategic play.** One Rust codebase compiles to:
- Native CLI binary (primary distribution)
- WASM module embeddable in VS Code extensions, browser IDEs, Cursor, edge workers
- C-compatible shared library (.so/.dylib/.dll) for FFI from any language

Nobody else in the agent memory space can do this.

## Crate Selection

### Core (required for L0)

> **Note:** Dependencies are split across two workspace crates (see AD-1 in ARCHITECTURE-DECISIONS.md).

**memory-core** (lib, WASM-compatible — no async, no native-only deps):

```toml
[dependencies]
rusqlite = { version = "0.38", features = ["bundled", "fts5"] }
rusqlite_migration = "2.4"       # schema migrations via user_version pragma
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"                     # content dedup hashing (2-10x faster than SHA-256)
thiserror = "2"
regex-lite = "0.1"               # secret detection (ASCII-only patterns, fast compile)
```

**memory-agent** (bin, native-only — async runtime, MCP, CLI, daemon):

```toml
[dependencies]
memory-core = { path = "../memory-core" }
rmcp = { version = "0.16", features = ["server"] }
schemars = "1"                    # JsonSchema derive for rmcp #[tool] params
tokio = { version = "1", features = ["rt-multi-thread", "macros", "io-std", "signal", "net"] }
clap = { version = "4", features = ["derive"] }
notify = "7"
toml = "0.8"                     # config file parsing (AD-18)
dirs = "6"                       # platform-specific dirs
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
tracing-appender = "0.2"         # log rotation (AD-22)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Level 1 (git integration)

```toml
# Git operations — feature-gated, not available in WASM
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
git2 = "0.20"    # libgit2 bindings for diff detection
```

### Optional (tiered LLM + extras)

```toml
# Tier 2: Local LLM via Ollama
reqwest = { version = "0.12", features = ["json"], optional = true }

# TUI (later)
ratatui = { version = "0.29", optional = true }
crossterm = { version = "0.28", optional = true }

# HTTP server (later)
axum = { version = "0.8", optional = true }

[features]
default = ["native"]
native = ["git2", "tokio/fs", "notify"]
local-llm = ["reqwest"]       # Tier 2: Ollama integration
cloud-llm = ["reqwest"]       # Tier 3: Cloud API
tui = ["ratatui", "crossterm"]
wasm = ["wasm-bindgen", "wasm-bindgen-futures"]
```

### WASM Build

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
# Replace tokio with wasm-compatible runtime
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
# SQLite via sql.js or feature-gate to in-memory only
```

## Module Architecture

Two-crate workspace (see ARCHITECTURE-DECISIONS.md AD-1):

```
memory-agent/                    (workspace root)
  Cargo.toml                     (workspace manifest)
  crates/
    memory-core/                 (lib — WASM-compatible, Stage 1)
      src/
        lib.rs                   # Re-exports, Store struct
        error.rs                 # MemoryError enum (thiserror)
        types.rs                 # Memory, Session, SearchResult, SourceType
        store/
          mod.rs                 # Store: connection, migrations
          schema.rs              # migrations via rusqlite_migration (user_version pragma)
          memory.rs              # CRUD: save (upsert), search (FTS5), detail, delete, list
          session.rs             # Session start/end, recent sessions
          dedup.rs               # normalize_content(), hash_content() (blake3), check_duplicate()
          privacy.rs             # strip_private_tags(), strip_secrets() (regex-lite)
          relations.rs           # Stage 7: relations CRUD, traversal, conflict detection
        search.rs                # FTS5 query building, sanitize_fts_query()

    memory-agent/                (bin — native only, Stage 2+)
      src/
        main.rs                  # CLI + MCP server + daemon startup
        async_store.rs           # AsyncStore wrapper (std::sync::Mutex + spawn_blocking)
        mcp/
          mod.rs                 # MCP server setup, tool registration
          tools.rs               # Handler functions for each MCP tool
          types.rs               # Request/response types (serde structs)
        daemon/
          mod.rs                 # Daemon lifecycle: start, stop, status, PID management
          watcher.rs             # Filesystem watcher — .git/refs, config files (notify crate)
          scheduler.rs           # Background task scheduler (maintenance loop)
          ipc.rs                 # Unix socket for daemon <-> MCP communication
          skills.rs              # Stage 13: skill lifecycle hooks
          llm.rs                 # Stage 13: tiered LLM (None / Ollama / Cloud)
          context_prep.rs        # Stage 13: pre-session context preparation
        source/
          mod.rs                 # SourceType enum, provenance chain
          git.rs                 # git_diff_check(), cache_invalidation() (Stage 5)
          config.rs              # cold_start_extract() — parse project files (Stage 4)
          observer.rs            # track_access(), detect_patterns(), auto_promote() (Stage 6)
        metrics/
          mod.rs                 # Token ROI tracking, hit rate, injection stats

  tests/                         # workspace-level integration tests
    integration/                 # Per-stage integration tests
```

## Core Types

```rust
use serde::{Deserialize, Serialize};
// Timestamps stored as ISO 8601 strings (SQLite strftime), no chrono dependency
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid scope: {0}")]
    InvalidScope(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("duplicate within dedup window")]
    Duplicate { existing_id: i64 },
    #[error("migration error: {0}")]
    Migration(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

/// Normalize a scope path: ensure leading `/`, strip trailing `/`, collapse `//`.
pub fn normalize_scope(scope: &str) -> std::result::Result<String, MemoryError> {
    let trimmed = scope.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Ok("/".to_string());
    }
    let with_slash = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };
    let collapsed = with_slash
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    Ok(format!("/{}", collapsed))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Codebase,    // parsed from source files
    Explicit,    // user said "remember X"
    Observed,    // inferred from repeated behavior
    Derived,     // computed from other memories
    Procedural,  // generated by skill completion
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub key: String,            // semantic identifier (topic_key)
    pub value: String,          // the actual knowledge
    pub scope: String,          // DAG path: /org/project/branch
    pub source_type: SourceType,
    pub source_ref: Option<String>,    // file:lines for codebase
    pub source_commit: Option<String>, // git commit hash
    pub confidence: f64,
    pub session_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub revision_count: i32,
    pub duplicate_count: i32,
    pub normalized_hash: String,
    pub created_at: DateTime<Utc>,
    pub accessed_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: i64,
    pub key: String,
    pub value_preview: String,  // truncated for progressive disclosure
    pub scope: String,
    pub source_type: SourceType,
    pub confidence: f64,
    pub rank: f64,              // FTS5 relevance score
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project: String,
    pub directory: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub status: String,         // active | completed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRequest {
    pub key: String,
    pub value: String,
    pub scope: Option<String>,       // defaults to "/"
    pub source_type: Option<SourceType>, // defaults to Explicit
    pub source_ref: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub scope: Option<String>,
    pub source_type: Option<SourceType>,
    pub limit: Option<i32>,          // default 10
}
```

## Key Implementation Details

### Scope Resolution (Recursive CTE)

```sql
-- Find all memories applicable to scope /org/acme/project/api
-- by walking up the DAG: /org/acme/project/api -> /org/acme/project -> /org/acme -> /org -> /
WITH RECURSIVE scope_chain(s) AS (
    VALUES('/org/acme/project/api')
    UNION ALL
    SELECT
        CASE
            WHEN s LIKE '%/%' THEN substr(s, 1, length(s) - length(replace(rtrim(s, '/'), '/', '')) - 1)
            ELSE '/'
        END
    FROM scope_chain
    WHERE s != '/'
)
SELECT m.* FROM memories m
JOIN scope_chain sc ON m.scope = sc.s
WHERE m.deleted_at IS NULL
ORDER BY length(m.scope) DESC,  -- most specific scope first
         m.accessed_at DESC
LIMIT ?;
```

### Content Deduplication

```rust
fn normalize_content(content: &str) -> String {
    content
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn hash_content(key: &str, scope: &str, content: &str) -> String {
    let normalized = normalize_content(content);
    let input = format!("{}:{}:{}", key, scope, normalized);
    blake3::hash(input.as_bytes()).to_hex().to_string()
}

/// Returns Some(existing_id) if duplicate found within window, None if new
fn check_duplicate(
    conn: &Connection,
    hash: &str,
    window_minutes: i32,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM memories
         WHERE normalized_hash = ?1
         AND deleted_at IS NULL
         AND last_seen_at > datetime('now', ?2)
         LIMIT 1",
        params![hash, format!("-{} minutes", window_minutes)],
        |row| row.get(0),
    ).optional()
}
```

### Privacy Stripping

```rust
use regex::Regex;
use std::sync::LazyLock;

static PRIVATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?si)<private>.*?</private>").unwrap()
});

fn strip_private(content: &str) -> String {
    PRIVATE_RE.replace_all(content, "[REDACTED]").to_string()
}
```

### FTS5 Search

```rust
fn search(
    conn: &Connection,
    query: &str,
    scope: Option<&str>,
    source_type: Option<&SourceType>,
    limit: i32,
) -> rusqlite::Result<Vec<SearchResult>> {
    let sanitized = sanitize_fts_query(query);

    let mut sql = String::from(
        "SELECT m.id, m.key, substr(m.value, 1, 200) as preview,
                m.scope, m.source_type, m.confidence,
                memories_fts.rank
         FROM memories_fts
         JOIN memories m ON m.id = memories_fts.rowid
         WHERE memories_fts MATCH ?1
         AND m.deleted_at IS NULL"
    );

    if scope.is_some() {
        sql.push_str(" AND m.scope = ?2");
    }
    if source_type.is_some() {
        sql.push_str(" AND m.source_type = ?3");
    }

    sql.push_str(" ORDER BY memories_fts.rank LIMIT ?4");

    // ... execute and map rows to SearchResult
}

fn sanitize_fts_query(query: &str) -> String {
    // Wrap each term in quotes to prevent FTS5 syntax errors
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}
```

### Git Diff Cache Invalidation (Level 1)

```rust
use git2::Repository;

struct CacheEntry {
    memory_id: i64,
    source_file: String,
    source_lines: (usize, usize),  // start, end
    commit_hash: String,
}

fn is_stale(repo: &Repository, entry: &CacheEntry) -> Result<bool> {
    let head = repo.head()?.peel_to_commit()?;
    let head_hash = head.id().to_string();

    if head_hash == entry.commit_hash {
        return Ok(false); // same commit, definitely fresh
    }

    let old_commit = repo.find_commit(
        git2::Oid::from_str(&entry.commit_hash)?
    )?;

    let diff = repo.diff_tree_to_tree(
        Some(&old_commit.tree()?),
        Some(&head.tree()?),
        None,
    )?;

    let mut touches_our_lines = false;

    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path() {
                path.to_str() == Some(&entry.source_file)
            } else {
                true // continue
            }
        },
        None,
        Some(&mut |_delta, _hunk, line| {
            let line_no = line.new_lineno().unwrap_or(0) as usize;
            if line_no >= entry.source_lines.0 && line_no <= entry.source_lines.1 {
                touches_our_lines = true;
            }
            true
        }),
        None,
    )?;

    Ok(touches_our_lines)
}
```

## WASM Strategy

### What works in WASM (via memory-core)
- SQLite (via compiled C to WASM, or sql.js)
- FTS5 search
- Content hashing (blake3 is pure Rust, has WASM target)
- Privacy stripping (regex-lite is pure Rust)
- All memory CRUD operations
- Scope resolution

### Crate boundary handles it

Because the workspace splits into `memory-core` (lib, no async, no native deps) and `memory-agent` (bin, native-only), WASM compilation targets `memory-core` directly. No `#[cfg]` gates are needed in core — the crate boundary is the feature boundary.

A third crate `memory-wasm` will be added at Stage 8 to provide wasm-bindgen bindings over `memory-core`.

### What stays native-only (memory-agent)
- `git2` (libgit2 — C FFI, no WASM target)
- `tokio` filesystem ops
- `notify` filesystem watcher
- Cold start config extraction
- MCP stdio transport
- Persistent daemon

**WASM build = memory-core only.** Git diff, config extraction, and filesystem operations live in memory-agent (native-only). This is fine — WASM targets (VS Code extensions, browser) don't have git repos anyway.

## Distribution

```
Releases:
  memory-agent-x86_64-linux       # Linux
  memory-agent-aarch64-linux      # Linux ARM
  memory-agent-x86_64-darwin      # macOS Intel
  memory-agent-aarch64-darwin     # macOS Apple Silicon
  memory-agent-x86_64-windows.exe # Windows
  memory-agent.wasm               # WASM module

Install methods:
  cargo install memory-agent       # Rust users
  brew install memory-agent        # macOS
  curl -fsSL install.sh | sh      # Universal
  npm install -g memory-agent-bin  # npm wrapper (binary download)
```

## Build Priority (Rust-specific)

### Week 1-2: Level 0 Core
1. `store/schema.rs` — migrations, table creation
2. `store/memory.rs` — save (upsert), search (FTS5), detail, delete, list
3. `store/dedup.rs` — content hashing, duplicate detection
4. `store/privacy.rs` — private tag stripping
5. `store/session.rs` — session start/end
6. `mcp/tools.rs` — MCP tool handlers
7. `main.rs` — CLI + MCP server startup

### Week 2-3: Level 0 Differentiators
8. Scope resolution — DAG scope (recursive CTE, Stage 3)
9. `source/config.rs` — cold start extraction
10. `metrics/mod.rs` — token ROI tracking

### Week 3-4: Level 1
11. `source/git.rs` — git diff cache invalidation
12. `source/observer.rs` — behavioral observation
13. Relations table + queries

### Month 2+: Level 2 + Distribution
14. SimpleMem-style compression
15. WASM build target
16. Homebrew/npm distribution
17. Ratatui TUI
