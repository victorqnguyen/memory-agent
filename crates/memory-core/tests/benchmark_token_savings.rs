use memory_core::test_utils::create_test_store;
use memory_core::{SaveParams, SearchParams, SourceType};
use tiktoken_rs::cl100k_base;

struct InfoNeed {
    id: &'static str,
    query: &'static str,
    scope: Option<&'static str>,
    fallback_tokens: u32,
    description: &'static str,
}

struct NeedResult {
    #[allow(dead_code)]
    need_id: &'static str,
    description: &'static str,
    hit: bool,
    tokens_with: u32,
    tokens_without: u32,
}

struct BenchmarkResult {
    scenario: &'static str,
    needs: Vec<NeedResult>,
}

fn seed_codebase_memories(store: &memory_core::Store) {
    let memories: &[(&str, &str, &str)] = &[
        ("db/config", "Database uses SQLite with WAL mode. Config at ~/.memory-agent/config.toml. Connection pragmas centralized in store/mod.rs configure_connection.", "/project"),
        ("error/pattern", "All errors use thiserror in memory-core, anyhow in memory-agent. Match on Error variants, never use string matching.", "/project"),
        ("auth/approach", "No auth in single-user mode. MCP transport handles auth. Unix socket permissions for local security.", "/project"),
        ("api/mcp-tools", "MCP tools use rmcp 0.16 with tool_router macro and tool attribute macros. Tool names are prefixed memory_ and locked in protocol.", "/project"),
        ("crate/boundary", "memory-core is sync and WASM-compatible, no tokio. memory-agent adds async via AsyncStore wrapper with spawn_blocking.", "/project"),
        ("search/fts5", "FTS5 full-text search with 4 triggers: insert, update, soft-delete, hard-delete. Queries sanitized via sanitize_fts_query whitelist.", "/project"),
        ("dedup/hashing", "Content deduplication uses blake3 hashing, 2-10x faster than SHA-256. Duplicate entries increment duplicate_count rather than storing again.", "/project"),
        ("config/defaults", "Config struct has Default impl. TOML parsed in memory-agent only. Precedence: CLI flags > env vars > config file > compiled defaults.", "/project"),
        ("schema/migrations", "Schema migrations use rusqlite_migration crate with user_version pragma. Each stage adds its own M::up() entry. Migrations are transactional.", "/project"),
        ("privacy/secrets", "strip_secrets runs on all content before storage. Patterns in privacy.rs. Built-in defaults plus user extra_patterns.", "/project"),
        ("logging/policy", "Never log memory values. Keys and IDs only. Internal SQL and file paths never exposed in MCP error messages.", "/project"),
        ("ipc/transport", "IpcTransport enum dispatch: Unix socket on macOS/Linux, TCP localhost on Windows. No async-trait crate, enum with 2 variants.", "/project"),
        ("retention/policy", "Soft-deleted rows purged after retention_days (default 90). VACUUM runs on vacuum_interval_secs (default weekly).", "/project"),
        ("versioning/scheme", "Binary semver 0.x.y. Protocol starts at 1.0, additive-only changes. Schema is integer forward-only.", "/project"),
        ("testing/approach", "TDD for memory-core: write failing test first. No mocks for SQLite, use Store::open_in_memory(). Security tests are mandatory.", "/project"),
        ("scope/validation", "Scope normalization: strip trailing slash, ensure leading slash, empty becomes /. Validate: no .., no null bytes.", "/project"),
        ("daemon/lifecycle", "Daemon manages PID file at 0644 permissions. DB/socket/log files at 0600. Stale PID files detected and cleaned automatically.", "/project"),
        ("dependency/blake3", "Use blake3 for hashing, not sha2. Use regex-lite for secret patterns, not full regex. No chrono, SQLite handles timestamps.", "/project"),
        ("error/codes", "MCP error codes: -32602 invalid params, -32001 not found, -32000 server error. Codes are API contract, never change existing codes.", "/project"),
        ("confidence/constraint", "Confidence column has CHECK constraint: 0.0 <= confidence <= 1.0. Default confidence is 1.0 on insert.", "/project"),
    ];
    for (key, value, scope) in memories {
        store
            .save(SaveParams {
                key: key.to_string(),
                value: value.to_string(),
                scope: Some(scope.to_string()),
                source_type: Some(SourceType::Codebase),
                ..Default::default()
            })
            .unwrap();
    }
}

fn seed_session_memories(store: &memory_core::Store) {
    // Session 1: initial project setup decisions
    let session1: &[(&str, &str, &str)] = &[
        ("session1/auth-decision", "Decided: no authentication layer for single-user local tool. Security via Unix socket file permissions 0600.", "/project"),
        ("session1/db-engine", "Chose SQLite over PostgreSQL: zero infrastructure, single file, WASM-compatible. WAL mode for concurrent reads.", "/project"),
        ("session1/crate-split", "Split into memory-core (sync, no_std-friendly) and memory-agent (async, CLI). Enables WASM target for core.", "/project"),
        ("session1/mcp-protocol", "Adopted MCP protocol over custom RPC. Uses rmcp 0.16 crate. Tool names locked after first release.", "/project"),
        ("session1/error-strategy", "thiserror for typed errors in core, anyhow for convenience in agent binary. Never expose SQL in error messages.", "/project"),
    ];
    // Session 2: bug fixes and refactoring
    let session2: &[(&str, &str, &str)] = &[
        ("session2/fts5-injection-bug", "Fixed FTS5 injection vulnerability. Raw user input was passed directly to FTS5 query. Added sanitize_fts_query whitelist.", "/project"),
        ("session2/dedup-refactor", "Refactored dedup to use blake3 instead of sha256. 3x speed improvement on large values. Updated all tests.", "/project"),
        ("session2/connection-pragma", "Centralized all SQLite pragma settings in configure_connection(). Previously scattered across codebase causing inconsistency.", "/project"),
        ("session2/scope-normalization", "Added normalize_scope() to handle trailing slashes and missing leading slashes consistently across all store operations.", "/project"),
        ("session2/migration-transaction", "Wrapped schema migrations in transactions. Previously partial migration left DB in broken state on crash.", "/project"),
    ];
    // Session 3: new features
    let session3: &[(&str, &str, &str)] = &[
        ("session3/retention-impl", "Implemented retention policy: soft-delete sets deleted_at, purge job removes after 90 days, weekly VACUUM.", "/project"),
        ("session3/metrics-table", "Added memory_metrics table to track injection count, hit count, tokens_injected per key. Enables value reporting.", "/project"),
        ("session3/privacy-patterns", "Added strip_secrets() with regex-lite patterns for API keys, tokens, passwords. Runs on all saves.", "/project"),
        ("session3/wasm-plan", "Planned WASM build for Stage 8. memory-core already WASM-compatible. Need wasm-bindgen wrappers and JS glue.", "/project"),
        ("session3/doctor-command", "Planned memory-agent doctor subcommand for Stage 9: DB health, config status, daemon status, version info.", "/project"),
    ];

    for (key, value, scope) in session1.iter().chain(session2).chain(session3) {
        store
            .save(SaveParams {
                key: key.to_string(),
                value: value.to_string(),
                scope: Some(scope.to_string()),
                source_type: Some(SourceType::Explicit),
                ..Default::default()
            })
            .unwrap();
    }
}

fn count_tokens(text: &str) -> u32 {
    // cl100k_base is the encoding used by Claude and GPT-4.
    // Initialisation is expensive; callers should avoid calling this in a hot loop.
    let bpe = cl100k_base().expect("failed to load cl100k_base tokenizer");
    bpe.encode_with_special_tokens(text).len() as u32
}

fn run_scenario(
    store: &memory_core::Store,
    scenario: &'static str,
    needs: &[InfoNeed],
) -> BenchmarkResult {
    // Initialise tokenizer once per scenario to amortise the load cost.
    let bpe = cl100k_base().expect("failed to load cl100k_base tokenizer");
    let tok = |s: &str| bpe.encode_with_special_tokens(s).len() as u32;

    let mut results = Vec::new();
    for need in needs {
        let search_results = store
            .search(SearchParams {
                query: need.query.to_string(),
                scope: need.scope.map(|s| s.to_string()),
                source_type: None,
                limit: Some(3),
            })
            .unwrap();

        let hit = !search_results.is_empty();
        // Per-result overhead: key label + formatting ~10 tokens; count actual preview tokens.
        let tokens_with = if hit {
            search_results
                .iter()
                .map(|r| tok(&r.value_preview) + 10)
                .sum()
        } else {
            need.fallback_tokens
        };

        results.push(NeedResult {
            need_id: need.id,
            description: need.description,
            hit,
            tokens_with,
            tokens_without: need.fallback_tokens,
        });
    }
    BenchmarkResult { scenario, needs: results }
}

fn print_results(result: &BenchmarkResult) {
    let hits = result.needs.iter().filter(|n| n.hit).count();
    let misses = result.needs.len() - hits;
    let tokens_with: u32 = result.needs.iter().map(|n| n.tokens_with).sum();
    let tokens_without: u32 = result.needs.iter().map(|n| n.tokens_without).sum();
    let reduction = if tokens_without > 0 {
        (1.0 - tokens_with as f64 / tokens_without as f64) * 100.0
    } else {
        0.0
    };

    println!("Scenario: {}", result.scenario);
    println!(
        "  Needs: {} | Hits: {} | Misses: {}",
        result.needs.len(),
        hits,
        misses
    );
    println!("  Tokens with memory:    {:>6}", tokens_with);
    println!("  Tokens without memory: {:>6}", tokens_without);
    println!("  Reduction: {:.1}%", reduction);
    println!();
    println!("  Per-need breakdown:");
    for need in &result.needs {
        let label = if need.hit { "[HIT] " } else { "[MISS]" };
        println!(
            "  {} {:<35} {:>4} vs {:>4} tokens",
            label, need.description, need.tokens_with, need.tokens_without
        );
    }
    println!();
}

#[test]
fn benchmark_token_savings() {
    let store = create_test_store();
    seed_codebase_memories(&store);

    let codebase_needs = vec![
        InfoNeed {
            id: "db-config-location",
            query: "database config SQLite WAL",
            scope: Some("/project"),
            fallback_tokens: 500,
            description: "Where is the database config?",
        },
        InfoNeed {
            id: "error-handling-pattern",
            query: "error handling thiserror anyhow",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "What error handling pattern do we use?",
        },
        InfoNeed {
            id: "auth-approach",
            query: "authentication auth security",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "What is the auth approach?",
        },
        InfoNeed {
            id: "mcp-tool-names",
            query: "MCP tools rmcp tool_router",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "How are MCP tools defined?",
        },
        InfoNeed {
            id: "crate-async-boundary",
            query: "async tokio spawn_blocking WASM",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "Where does async code live?",
        },
        InfoNeed {
            id: "fts5-search",
            query: "FTS5 full text search sanitize",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "How does FTS5 search work?",
        },
        InfoNeed {
            id: "dedup-algorithm",
            query: "deduplication blake3 hashing",
            scope: Some("/project"),
            fallback_tokens: 1000,
            description: "What dedup algorithm is used?",
        },
        InfoNeed {
            id: "schema-migration",
            query: "schema migration rusqlite_migration user_version",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "How are schema migrations handled?",
        },
        InfoNeed {
            id: "config-precedence",
            query: "config TOML env vars CLI flags precedence",
            scope: Some("/project"),
            fallback_tokens: 1000,
            description: "What is the config precedence order?",
        },
        InfoNeed {
            id: "secret-detection",
            query: "secrets privacy strip patterns",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "How are secrets stripped from content?",
        },
    ];

    let scenario1 = run_scenario(&store, "Codebase Q&A", &codebase_needs);

    // Fresh store for scenario 2
    let store2 = create_test_store();
    seed_session_memories(&store2);

    let session_needs = vec![
        InfoNeed {
            id: "auth-decision",
            query: "authentication decision single-user",
            scope: Some("/project"),
            fallback_tokens: 3000,
            description: "What did I decide about auth?",
        },
        InfoNeed {
            id: "db-choice",
            query: "SQLite PostgreSQL database choice",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "Why did we choose SQLite?",
        },
        InfoNeed {
            id: "fts5-bug-fix",
            query: "FTS5 injection bug fix sanitize",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "What FTS5 bug did I fix?",
        },
        InfoNeed {
            id: "dedup-change",
            query: "dedup blake3 sha256 refactor",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "What changed in dedup implementation?",
        },
        InfoNeed {
            id: "migration-crash-fix",
            query: "migration transaction crash rollback",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "What migration crash was fixed?",
        },
        InfoNeed {
            id: "retention-impl",
            query: "retention soft-delete purge VACUUM",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "How is retention implemented?",
        },
        InfoNeed {
            id: "privacy-feature",
            query: "privacy secrets regex-lite strip",
            scope: Some("/project"),
            fallback_tokens: 1500,
            description: "How does the privacy feature work?",
        },
        InfoNeed {
            id: "future-wasm",
            query: "WASM wasm-bindgen plan stage",
            scope: Some("/project"),
            fallback_tokens: 2000,
            description: "What is the WASM plan?",
        },
    ];

    let scenario2 = run_scenario(&store2, "Multi-Session Continuity", &session_needs);

    // Print formatted table
    println!();
    println!("=== Token Savings Benchmark ===");
    println!();
    print_results(&scenario1);
    print_results(&scenario2);

    // Summary table
    println!("Summary:");
    println!(
        "{:<28} {:>6} {:>6} {:>7} {:>14} {:>16} {:>12}",
        "Scenario", "Needs", "Hits", "Misses", "Tokens (with)", "Tokens (without)", "Reduction %"
    );
    println!("{}", "-".repeat(95));

    for result in [&scenario1, &scenario2] {
        let hits = result.needs.iter().filter(|n| n.hit).count();
        let misses = result.needs.len() - hits;
        let tokens_with: u32 = result.needs.iter().map(|n| n.tokens_with).sum();
        let tokens_without: u32 = result.needs.iter().map(|n| n.tokens_without).sum();
        let reduction = if tokens_without > 0 {
            (1.0 - tokens_with as f64 / tokens_without as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{:<28} {:>6} {:>6} {:>7} {:>14} {:>16} {:>11.1}%",
            result.scenario,
            result.needs.len(),
            hits,
            misses,
            tokens_with,
            tokens_without,
            reduction
        );
    }

    // Assertions: >50% token reduction per scenario
    let tokens_with_s1: u32 = scenario1.needs.iter().map(|n| n.tokens_with).sum();
    let tokens_without_s1: u32 = scenario1.needs.iter().map(|n| n.tokens_without).sum();
    let reduction_s1 =
        (1.0 - tokens_with_s1 as f64 / tokens_without_s1 as f64) * 100.0;

    let tokens_with_s2: u32 = scenario2.needs.iter().map(|n| n.tokens_with).sum();
    let tokens_without_s2: u32 = scenario2.needs.iter().map(|n| n.tokens_without).sum();
    let reduction_s2 =
        (1.0 - tokens_with_s2 as f64 / tokens_without_s2 as f64) * 100.0;

    assert!(
        reduction_s1 > 50.0,
        "Scenario 1 token reduction {:.1}% must be >50%",
        reduction_s1
    );
    assert!(
        reduction_s2 > 50.0,
        "Scenario 2 token reduction {:.1}% must be >50%",
        reduction_s2
    );
}
