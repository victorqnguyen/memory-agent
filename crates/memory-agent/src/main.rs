mod async_store;
mod config_loader;
pub(crate) mod llm;
mod mcp;
mod setup;
pub(crate) mod skills;
pub(crate) mod source;
#[cfg(feature = "tui")]
mod tui;
mod update_check;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use memory_core::Store;

use async_store::AsyncStore;
use config_loader::default_data_dir;


#[derive(Parser)]
#[command(name = "memory-agent", about = "Persistent memory for AI coding agents")]
enum Cli {
    /// Start MCP server on stdio
    Mcp,
    /// Create default config file
    Init,
    /// Show effective configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show version info (binary, protocol, schema)
    Version,
    /// Check crates.io for a newer version
    Update,
    /// Search memories
    Search {
        query: String,
        #[arg(short, long)]
        scope: Option<String>,
        #[arg(short, long, default_value = "10")]
        limit: i32,
    },
    /// Save a memory
    Save {
        #[arg(short, long)]
        key: String,
        #[arg(short, long)]
        value: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        source_type: Option<String>,
        #[arg(short, long)]
        tags: Option<Vec<String>>,
    },
    /// List memories
    List {
        #[arg(short, long)]
        scope: Option<String>,
        #[arg(long)]
        source_type: Option<String>,
        #[arg(short, long, default_value = "20")]
        limit: i32,
    },
    /// Show full memory detail
    Detail {
        id: i64,
    },
    /// Delete memories (by id, key, or all in a scope with --all)
    Delete {
        /// Delete by memory ID
        #[arg(long)]
        id: Option<i64>,
        #[arg(short, long)]
        key: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        hard: bool,
        /// Delete all memories in the given scope
        #[arg(long)]
        all: bool,
    },
    /// Show statistics
    Stats,
    /// Health check — show what's working and what's not
    Doctor,
    /// Extract memories from project config files
    Extract {
        #[arg(short, long, default_value = ".")]
        dir: std::path::PathBuf,
        #[arg(long)]
        scope: Option<String>,
    },
    /// Show stale memories (git-tracked source changes)
    Stale {
        #[arg(short, long)]
        scope: Option<String>,
        #[arg(short, long)]
        directory: Option<String>,
    },
    /// Show token efficiency metrics
    Metrics,
    /// Launch interactive terminal UI
    #[cfg(feature = "tui")]
    Tui,
    /// Export all data as JSON
    Export,
    /// Import data from JSON
    Import,
    /// Run background maintenance (confidence decay, purge, vacuum)
    Maintenance {
        /// Show status only, do not execute
        #[arg(long)]
        dry_run: bool,
    },
    /// Run VACUUM on the database to reclaim space
    Vacuum,
    /// Search memories relevant to a prompt and output formatted context (for hooks)
    AutoContext {
        /// The user's prompt text
        prompt: String,
        /// Project scope
        #[arg(short, long)]
        scope: Option<String>,
        /// Max results
        #[arg(short, long, default_value = "5")]
        limit: i32,
    },
    /// Record a file edit observation (for hooks, fire-and-forget)
    ObserveEdit {
        /// Path of the edited file
        file_path: String,
        /// Project scope
        #[arg(short, long)]
        scope: Option<String>,
    },
    /// Extract learnings from transcript before compaction (for hooks)
    PreCompact {
        /// Path to the conversation transcript JSONL
        transcript: String,
        /// Project scope
        #[arg(short, long)]
        scope: Option<String>,
    },
    /// Handle instructions file load — re-extract if changed, inject delta context (for hooks)
    InstructionsLoaded {
        /// Path to the instructions file that was loaded
        file_path: String,
        /// Project scope (auto-detected from CWD if omitted)
        #[arg(short, long)]
        scope: Option<String>,
    },
    /// Emit static injection_prompt verbatim (for hooks)
    HookInject {
        /// Hook event name (ignored, kept for backward compatibility)
        #[arg(hide = true)]
        event: Option<String>,
        /// Emit the static injection_prompt from config
        #[arg(long)]
        static_only: bool,
    },
    /// Check if a hook feature is enabled. Exits 0 if enabled, 1 if disabled.
    /// Used by hook scripts: `memory-agent hook-gate agent_review_gate || exit 0`
    #[command(hide = true)]
    HookGate {
        feature: String,
    },
    /// Install memory-agent into an editor/tool (e.g. `memory-agent install claude`)
    Install {
        /// Target to install into
        target: InstallTarget,
        /// Skip confirmation prompts
        #[arg(long)]
        yes: bool,
    },
    /// Uninstall memory-agent from an editor/tool (e.g. `memory-agent uninstall claude`)
    Uninstall {
        /// Target to uninstall from
        target: InstallTarget,
        /// Also remove all data (~/.memory-agent/)
        #[arg(long)]
        purge: bool,
        /// Skip confirmation prompts
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Clone, clap::ValueEnum)]
enum InstallTarget {
    /// Claude Code (MCP server, CLAUDE.md, hooks, slash commands)
    Claude,
    /// Gemini CLI (~/.gemini/settings.json)
    Gemini,
    /// Cursor (~/.cursor/mcp.json)
    Cursor,
    /// OpenAI Codex (~/.codex/config.json)
    Codex,
    /// OpenCode (~/.config/opencode/opencode.json) — MCP + TypeScript plugin hooks
    Opencode,
    /// Other MCP client (prints config to copy-paste)
    Other,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print effective config (merged from all sources)
    Show,
    /// Print config file path
    Path,
    /// Configure LLM settings (Ollama model, URL)
    Llm,
    /// Manage database encryption (SQLCipher)
    Encryption {
        #[command(subcommand)]
        action: EncryptionAction,
    },
}

#[derive(Subcommand)]
enum EncryptionAction {
    /// Enable encryption (generates passphrase, encrypts DB, stores in keychain)
    Enable,
    /// Disable encryption (decrypts DB, removes passphrase from keychain)
    Disable,
    /// Show encryption status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data_dir = default_data_dir();

    // First-run detection: no args + no config = run setup walkthrough
    if std::env::args().len() == 1 && !data_dir.join("config.toml").exists() {
        println!("Welcome to memory-agent!\n");
        return setup::install(&data_dir, false);
    }

    let cli = Cli::parse();


    let agent_config = config_loader::load(&data_dir)?;
    let config = agent_config.core;
    let llm_config = agent_config.llm;
    let hooks_config = agent_config.hooks;

    match cli {
        Cli::Mcp => {
            init_tracing();
            let store = open_store(&data_dir, config)?;
            let async_store = AsyncStore::new(store);
            let llm_tier = llm::detect_tier(&llm_config).await;
            tracing::info!("LLM tier: {}", llm_tier.tier_name());
            mcp::run_mcp_server(async_store, llm_tier).await
        }
        Cli::Init => {
            config_loader::create_default_config(&data_dir)?;
            println!("Created {}/config.toml", data_dir.display());
            Ok(())
        }
        Cli::Config { action } => match action {
            ConfigAction::Show => {
                println!("{}", toml::to_string_pretty(&config)?);
                Ok(())
            }
            ConfigAction::Path => {
                println!("{}", config_path(&data_dir).display());
                Ok(())
            }
            ConfigAction::Llm => {
                // Ensure config exists first
                if !data_dir.join("config.toml").exists() {
                    config_loader::create_default_config(&data_dir)?;
                }
                let choice = setup::configure_llm(false);
                setup::write_llm_config(&data_dir, &choice)?;
                match &choice {
                    setup::LlmChoice::Ollama { model, .. } => {
                        println!("LLM configured: Ollama with {}", model);
                    }
                    setup::LlmChoice::None => {
                        println!("LLM disabled (template-based processing).");
                    }
                }
                println!("Restart any running MCP server for changes to take effect.");
                Ok(())
            }
            ConfigAction::Encryption { action: enc_action } => {
                let db_path = data_dir.join("memory.db");
                let db_path_str = db_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {:?}", db_path))?;
                match enc_action {
                    EncryptionAction::Enable => {
                        if config.storage.encryption_enabled {
                            println!("Encryption is already enabled in config.");
                            return Ok(());
                        }
                        let passphrase = config_loader::generate_passphrase();
                        config_loader::store_passphrase(&passphrase)?;
                        println!("Passphrase stored in system keychain.");

                        if db_path.exists() {
                            println!("Encrypting existing database...");
                            Store::encrypt(db_path_str, &passphrase, config.clone())?;
                            println!("Database encrypted successfully.");
                        }

                        set_config_value(&data_dir, "encryption_enabled", "true")?;
                        println!("Encryption enabled. Restart any running MCP server.");
                        Ok(())
                    }
                    EncryptionAction::Disable => {
                        if !config.storage.encryption_enabled {
                            println!("Encryption is already disabled in config.");
                            return Ok(());
                        }
                        let passphrase = config_loader::retrieve_passphrase()?
                            .ok_or_else(|| anyhow::anyhow!("no passphrase found — cannot decrypt"))?;

                        if db_path.exists() {
                            println!("Decrypting database...");
                            Store::decrypt(db_path_str, &passphrase, config.clone())?;
                            println!("Database decrypted successfully.");
                        }

                        config_loader::delete_passphrase().ok();
                        set_config_value(&data_dir, "encryption_enabled", "false")?;
                        println!("Encryption disabled. Restart any running MCP server.");
                        Ok(())
                    }
                    EncryptionAction::Status => {
                        let config_enabled = config.storage.encryption_enabled;
                        let db_encrypted = db_path.exists() && Store::is_encrypted(db_path_str);
                        let source = config_loader::passphrase_source();

                        println!("Config:      encryption_enabled = {}", config_enabled);
                        println!("DB actual:   {}", if db_encrypted { "encrypted" } else { "plaintext" });
                        println!("Passphrase:  {}", source.unwrap_or("none"));

                        if config_enabled && !db_encrypted && db_path.exists() {
                            println!("\nWARNING: config says enabled but DB is plaintext");
                        } else if !config_enabled && db_encrypted {
                            println!("\nWARNING: config says disabled but DB is encrypted");
                        }
                        Ok(())
                    }
                }
            }
        },
        Cli::Version => {
            println!("memory-agent {}", env!("CARGO_PKG_VERSION"));
            println!("protocol: 1.0");
            println!("schema: {}", memory_core::SCHEMA_VERSION);
            println!("data: {}", data_dir.display());
            Ok(())
        }
        Cli::Update => {
            match update_check::check_for_update().await {
                Some(info) => println!("{}", info.full()),
                None => println!("memory-agent {} is up to date.", env!("CARGO_PKG_VERSION")),
            }
            Ok(())
        }
        Cli::Search { query, scope, limit } => {
            let store = open_store(&data_dir, config)?;
            let results = store.search(memory_core::SearchParams {
                query: query.clone(),
                scope: scope.clone(),
                source_type: None,
                limit: Some(limit),
            })?;
            let _ = store.write_event("search", &query, scope.as_deref().unwrap_or("/"), 0);
            if results.is_empty() {
                println!("No results found.");
            } else {
                println!("{:<6} {:<30} {:<15} {:<6} PREVIEW", "ID", "KEY", "SCOPE", "CONF");
                println!("{}", "-".repeat(80));
                for r in &results {
                    println!(
                        "{:<6} {:<30} {:<15} {:.2}   {}",
                        r.id,
                        truncate(&r.key, 28),
                        truncate(&r.scope, 13),
                        r.confidence,
                        truncate(&r.value_preview, 40),
                    );
                }
            }
            Ok(())
        }
        Cli::Save { key, value, scope, source_type, tags } => {
            let store = open_store(&data_dir, config)?;
            let st = source_type
                .map(|s| s.parse::<memory_core::types::SourceType>())
                .transpose()?;
            let ev_scope = scope.clone().unwrap_or_else(|| "/".to_string());
            let action = store.save(memory_core::SaveParams {
                key: key.clone(),
                value,
                scope,
                source_type: st,
                source_ref: None,
                source_commit: None,
                tags,
            })?;
            let _ = store.write_event("save", &key, &ev_scope, 0);
            match action {
                memory_core::SaveAction::Created(id) => println!("Created memory {} (key: {})", id, key),
                memory_core::SaveAction::Updated(id) => println!("Updated memory {} (key: {})", id, key),
                memory_core::SaveAction::Deduplicated(id) => println!("Deduplicated with memory {} (key: {})", id, key),
            }
            Ok(())
        }
        Cli::List { scope, source_type, limit } => {
            let store = open_store(&data_dir, config)?;
            let st = source_type
                .map(|s| s.parse::<memory_core::types::SourceType>())
                .transpose()?;
            let memories = store.list(scope.as_deref(), st.as_ref(), Some(limit))?;
            let _ = store.write_event("search", "list", scope.as_deref().unwrap_or("/"), 0);
            if memories.is_empty() {
                println!("No memories found.");
            } else {
                println!("{:<6} {:<30} {:<15} {:<6} PREVIEW", "ID", "KEY", "SCOPE", "CONF");
                println!("{}", "-".repeat(80));
                for m in &memories {
                    let preview = memory_core::make_preview(&m.value, 40);
                    println!(
                        "{:<6} {:<30} {:<15} {:.2}   {}",
                        m.id,
                        truncate(&m.key, 28),
                        truncate(&m.scope, 13),
                        m.confidence,
                        preview,
                    );
                }
            }
            Ok(())
        }
        Cli::Detail { id } => {
            let store = open_store(&data_dir, config)?;
            let _ = store.record_hit(id);
            let _ = store.write_event("search", &id.to_string(), "/", 0);
            match store.get(id) {
                Ok(m) => {
                    println!("ID:         {}", m.id);
                    println!("Key:        {}", m.key);
                    println!("Scope:      {}", m.scope);
                    println!("Source:     {}", m.source_type);
                    println!("Confidence: {:.2}", m.confidence);
                    println!("Revisions:  {}", m.revision_count);
                    println!("Duplicates: {}", m.duplicate_count);
                    println!("Created:    {}", m.created_at);
                    println!("Accessed:   {}", m.accessed_at);
                    if let Some(ref sr) = m.source_ref {
                        println!("Source Ref: {}", sr);
                    }
                    if let Some(ref tags) = m.tags {
                        println!("Tags:       {}", tags.join(", "));
                    }
                    println!();
                    println!("{}", m.value);
                }
                Err(memory_core::Error::NotFound(id)) => {
                    eprintln!("Memory {} not found.", id);
                    std::process::exit(1);
                }
                Err(e) => return Err(e.into()),
            }
            Ok(())
        }
        Cli::Delete { id, key, scope, hard, all } => {
            let store = open_store(&data_dir, config)?;
            if let Some(id) = id {
                let deleted = store.delete_by_id(id, hard)?;
                if deleted {
                    println!("Deleted memory {}", id);
                } else {
                    println!("No memory found with id: {}", id);
                }
            } else if all {
                let scope = scope.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("--all requires --scope")
                })?;
                let count = store.delete_scope(scope, hard)?;
                println!("Deleted {} memories in scope: {}", count, scope);
            } else {
                let key = key.ok_or_else(|| {
                    anyhow::anyhow!("provide --id, --key, or --all --scope <scope>")
                })?;
                let deleted = store.delete(&key, scope.as_deref(), hard)?;
                if deleted {
                    println!("Deleted memory with key: {}", key);
                } else {
                    println!("No memory found with key: {}", key);
                }
            }
            Ok(())
        }
        Cli::Doctor => {
            let store = open_store(&data_dir, config.clone())?;
            let all = store.list(None, None, Some(100000))?;
            let total = all.len();
            let by_source = |st: memory_core::types::SourceType| {
                all.iter().filter(|m| m.source_type == st).count()
            };
            let scopes: std::collections::HashSet<&str> = all.iter().map(|m| m.scope.as_str()).collect();

            println!("=== Memory Agent Health ===");
            println!();

            // Memories
            println!("Memories:    {} total", total);
            println!("  explicit:  {}", by_source(memory_core::types::SourceType::Explicit));
            println!("  codebase:  {}", by_source(memory_core::types::SourceType::Codebase));
            println!("  observed:  {}", by_source(memory_core::types::SourceType::Observed));
            println!("  derived:   {}", by_source(memory_core::types::SourceType::Derived));
            println!("  procedural:{}", by_source(memory_core::types::SourceType::Procedural));
            println!("Scopes:      {}", scopes.len());

            // Dedup & revisions
            let dedup_total = store.dedup_total().unwrap_or(0);
            let revision_total = store.revision_total().unwrap_or(0);
            println!("Dedup saves: {} (duplicate writes prevented)", dedup_total);
            println!("Revisions:   {} (upsert updates)", revision_total);
            println!();

            // Metrics
            let metrics = store.get_metrics()?;
            let total_inj: i32 = metrics.iter().map(|m| m.injections).sum();
            let total_hits: i32 = metrics.iter().map(|m| m.hits).sum();
            let agg_rate = if total_inj > 0 { total_hits as f64 / total_inj as f64 } else { 0.0 };
            let low_roi = store.low_roi_count().unwrap_or(0);
            println!("Injections:  {}", total_inj);
            println!("Hits:        {}", total_hits);
            println!("Hit rate:    {:.1}%", agg_rate * 100.0);
            println!("Low ROI:     {} (>10 injections, <10% hit rate)", low_roi);
            println!();

            // Stale check
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let source_tracked = store.list_by_source_commit().unwrap_or_default();
            let git = source::git::GitContext::open(&dir);
            let mut stale_count = 0;
            if let Some(git) = git {
                let mut by_commit: std::collections::HashMap<String, Vec<&memory_core::types::Memory>> =
                    std::collections::HashMap::new();
                for m in &source_tracked {
                    if let Some(ref commit) = m.source_commit {
                        by_commit.entry(commit.clone()).or_default().push(m);
                    }
                }
                for (commit, mems) in &by_commit {
                    let changed = match git.changed_files(commit) {
                        Ok(files) => files,
                        Err(_) => continue,
                    };
                    for m in mems {
                        if let Some(ref source_ref) = m.source_ref {
                            let (file, _, _) = memory_core::types::parse_source_ref(source_ref);
                            if changed.contains(&file) {
                                stale_count += 1;
                            }
                        }
                    }
                }
            }
            println!("Stale:       {} memories need refresh", stale_count);

            // Database
            let db_path = data_dir.join("memory.db");
            let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
            let created = store.get_metadata("db_created_at").ok().flatten().unwrap_or_else(|| "unknown".into());
            let last_vacuum = store.get_metadata("last_vacuum_at").ok().flatten().unwrap_or_else(|| "never".into());
            println!("DB size:     {} KB", db_size / 1024);
            println!("DB created:  {}", truncate(&created, 19));
            println!("Last vacuum: {}", last_vacuum);

            // Encryption
            let enc_enabled = config.storage.encryption_enabled;
            let db_encrypted = db_path.exists()
                && Store::is_encrypted(
                    db_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {:?}", db_path))?,
                );
            let enc_source = config_loader::passphrase_source();
            if enc_enabled {
                println!("Encryption:  enabled (passphrase: {})", enc_source.unwrap_or("missing!"));
            } else {
                println!("Encryption:  disabled");
            }
            println!();

            // Maintenance
            let maint = store.maintenance_status()?;
            if maint.vacuum_overdue || maint.purge_candidates > 0 {
                println!("Maintenance:");
                if maint.vacuum_overdue {
                    println!("  Vacuum overdue:  yes (last: {})", maint.last_vacuum_at);
                }
                if maint.purge_candidates > 0 {
                    println!("  Purge pending:   {} soft-deleted memories past retention", maint.purge_candidates);
                }

                // Dead worktree scopes
                let all_scopes = store.distinct_scopes().unwrap_or_default();
                let dead_scopes: Vec<&str> = all_scopes
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| {
                        let path = s.trim_start_matches('/');
                        !path.is_empty() && !std::path::Path::new(path).exists()
                    })
                    .collect();
                if !dead_scopes.is_empty() {
                    println!("  Dead scopes:     {} (worktree directories no longer exist)", dead_scopes.len());
                    for s in dead_scopes.iter().take(5) {
                        println!("    {}", s);
                    }
                }
                println!();
            }

            // Verdict
            let mut issues: Vec<String> = Vec::new();
            if total == 0 {
                issues.push("No memories stored. Run `memory-agent extract -d .` to bootstrap.".to_string());
            }
            if total_inj == 0 && total > 0 {
                issues.push("Zero injections. Claude isn't loading memories via memory_context/memory_budget.".to_string());
            }
            if stale_count > 0 {
                issues.push("Stale memories detected. Run `memory-agent stale` for details.".to_string());
            }
            if low_roi > 0 {
                issues.push("Low-ROI memories found. Run `memory-agent metrics` to review.".to_string());
            }
            if enc_enabled && !db_encrypted && db_path.exists() {
                issues.push("Config says encryption enabled but DB is plaintext. Run `memory-agent config encryption status`.".to_string());
            }
            if !enc_enabled && db_encrypted {
                issues.push("Config says encryption disabled but DB is encrypted. Run `memory-agent config encryption status`.".to_string());
            }
            if enc_enabled && enc_source.is_none() {
                issues.push("Encryption enabled but no passphrase found. Run `memory-agent config encryption enable` to fix.".to_string());
            }
            if maint.vacuum_overdue {
                issues.push("VACUUM overdue. Run 'memory-agent vacuum'.".to_string());
            }
            if maint.purge_candidates > 0 {
                issues.push(format!("{} soft-deleted memories past retention window. Run 'memory-agent maintenance'.", maint.purge_candidates));
            }

            if issues.is_empty() {
                println!("Status: HEALTHY");
            } else {
                println!("Issues found:");
                for issue in &issues {
                    println!("  - {}", issue);
                }
            }

            Ok(())
        }
        Cli::Stats => {
            let update_task = tokio::spawn(update_check::check_for_update());
            let store = open_store(&data_dir, config)?;

            // Storage size
            let db_path = data_dir.join("memory.db");
            if let Ok(meta) = std::fs::metadata(&db_path) {
                println!("Storage:   {}", format_bytes(meta.len()));
            }

            let all = store.list(None, None, Some(10000))?;
            let total = all.len();
            let total_bytes: usize = all.iter().map(|m| m.value.len() + m.key.len()).sum();
            let by_source = |st: memory_core::types::SourceType| {
                all.iter().filter(|m| m.source_type == st).count()
            };

            // Scope breakdown
            let mut scope_counts: std::collections::BTreeMap<&str, (usize, usize)> = std::collections::BTreeMap::new();
            for m in &all {
                let entry = scope_counts.entry(m.scope.as_str()).or_default();
                entry.0 += 1;
                entry.1 += m.value.len() + m.key.len();
            }

            println!("Memories:  {} ({})", total, format_bytes(total_bytes as u64));
            println!("Sources:   explicit={}, codebase={}, observed={}, derived={}",
                by_source(memory_core::types::SourceType::Explicit),
                by_source(memory_core::types::SourceType::Codebase),
                by_source(memory_core::types::SourceType::Observed),
                by_source(memory_core::types::SourceType::Derived),
            );

            // Cumulative (all-time, from metrics table — counts all injection paths incl. CLI hooks)
            let cumulative = store.cumulative_stats().unwrap_or_default();
            if cumulative.injections > 0 {
                // Use real token data when available; fall back to 25-token estimate for CLI
                // injections where tokens_per_memory was not measured (e.g. list output).
                let actual_tokens = if cumulative.tokens_injected > 0 {
                    cumulative.tokens_injected
                } else {
                    cumulative.injections * 25
                };

                let tool_call_overhead: i64 = 110;
                let mut without_tokens: i64 = 0;
                let metrics = store.get_metrics()?;
                for metric in &metrics {
                    if metric.hits > 0 || metric.injections > 0 {
                        if let Some(mem) = all.iter().find(|m| m.id == metric.id) {
                            // Active memory: use actual content size.
                            let content_tokens = memory_core::autonomous::adaptive::estimate_tokens(&mem.value) as i64;
                            without_tokens += metric.injections as i64 * (content_tokens + tool_call_overhead);
                        } else {
                            // Soft-deleted memory: content is gone, use stored tokens_injected
                            // as a proxy for content size (it was derived from the actual content).
                            without_tokens += metric.tokens_injected as i64 + metric.injections as i64 * tool_call_overhead;
                        }
                    }
                }

                let saved = without_tokens - actual_tokens;
                let hit_rate = if cumulative.injections > 0 {
                    cumulative.hits as f64 / cumulative.injections as f64
                } else {
                    0.0
                };

                println!();
                println!("Cumulative (all-time)");
                println!("{}", "-".repeat(50));
                println!("Injections:    {}", cumulative.injections);
                println!("Hits:          {}", cumulative.hits);
                println!("Hit rate:      {:.1}%", hit_rate * 100.0);
                println!("Unique mem:    {}", cumulative.unique_memories_injected);
                println!("With agent:    ~{} tokens (compact injection)", format_tokens(actual_tokens));
                println!("Without agent: ~{} tokens (tool calls to rediscover)", format_tokens(without_tokens));
                if saved > 0 {
                    let pct = saved as f64 / without_tokens as f64 * 100.0;
                    println!("Saved:         ~{} tokens ({:.0}% reduction)", format_tokens(saved), pct);
                }
            }

            if !scope_counts.is_empty() {
                println!();
                println!("{:<30} {:<8} SIZE", "SCOPE", "COUNT");
                println!("{}", "-".repeat(50));
                for (scope, (count, bytes)) in &scope_counts {
                    println!("{:<30} {:<8} {}", truncate(scope, 28), count, format_bytes(*bytes as u64));
                }
            }

            if let Some(info) = update_task.await.ok().flatten() {
                // ANSI: bold yellow text
                println!("\n\x1b[1;33m{}\x1b[0m", info.full());
            }

            Ok(())
        }
        Cli::Extract { dir, scope } => {
            let store = open_store(&data_dir, config)?;
            // Create default .memory-agentignore if it doesn't exist
            let ignore_path = dir.join(".memory-agentignore");
            if !ignore_path.exists() {
                std::fs::write(&ignore_path, source::extract::DEFAULT_IGNORE)?;
                println!("Created {} (excludes .env* by default)", ignore_path.display());
            }
            let scope_val = scope.unwrap_or_else(|| source::extract::scope_from_directory(&dir));
            let result = source::extract::extract_from_directory(&dir, &store, &scope_val)?;
            println!("Extracted: {}, Updated: {}, Skipped: {}", result.extracted, result.updated, result.skipped);
            if !result.files_scanned.is_empty() {
                println!("Files scanned: {}", result.files_scanned.join(", "));
            }
            Ok(())
        }
        Cli::Stale { scope, directory } => {
            let store = open_store(&data_dir, config)?;
            let dir = directory
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            let memories = store.list_by_source_commit()?;
            let git = source::git::GitContext::open(&dir);
            if let Some(git) = git {
                let mut found = 0;
                let mut by_commit: std::collections::HashMap<String, Vec<&memory_core::types::Memory>> =
                    std::collections::HashMap::new();
                for m in &memories {
                    if let Some(scope_filter) = &scope {
                        if !m.scope.starts_with(scope_filter.as_str()) {
                            continue;
                        }
                    }
                    if let Some(ref commit) = m.source_commit {
                        by_commit.entry(commit.clone()).or_default().push(m);
                    }
                }
                for (commit, mems) in &by_commit {
                    let changed = match git.changed_files(commit) {
                        Ok(files) => files,
                        Err(_) => continue,
                    };
                    for m in mems {
                        if let Some(ref source_ref) = m.source_ref {
                            let (file, _, _) = memory_core::types::parse_source_ref(source_ref);
                            if changed.contains(&file) {
                                println!("STALE [{}] {}: file '{}' changed since {}", m.id, m.key, file, commit);
                                found += 1;
                            }
                        }
                    }
                }
                if found == 0 {
                    println!("No stale memories found ({} checked).", memories.len());
                }
            } else {
                println!("No git repository found. Cannot check staleness.");
            }
            Ok(())
        }
        Cli::Metrics => {
            let store = open_store(&data_dir, config)?;
            let metrics = store.get_metrics()?;
            if metrics.is_empty() {
                println!("No metrics data yet.");
            } else {
                let total_injections: i32 = metrics.iter().map(|m| m.injections).sum();
                let total_hits: i32 = metrics.iter().map(|m| m.hits).sum();
                let agg_rate = if total_injections > 0 {
                    total_hits as f64 / total_injections as f64
                } else {
                    0.0
                };
                println!("Aggregate hit rate: {:.1}%", agg_rate * 100.0);
                println!("Total injections: {}, Total hits: {}", total_injections, total_hits);
                println!();
                println!("{:<6} {:<30} {:<8} {:<8} {:<8}", "ID", "KEY", "INJ", "HITS", "RATE");
                println!("{}", "-".repeat(65));
                for m in metrics.iter().take(20) {
                    println!("{:<6} {:<30} {:<8} {:<8} {:.1}%",
                        m.id,
                        truncate(&m.key, 28),
                        m.injections,
                        m.hits,
                        m.hit_rate * 100.0,
                    );
                }
            }
            Ok(())
        }
        #[cfg(feature = "tui")]
        Cli::Tui => {
            let update_task = tokio::spawn(update_check::check_for_update());
            let store = open_store(&data_dir, config)?;
            let update_notice = update_task.await.ok().flatten().map(|info| info.short());
            tui::run(store, data_dir, hooks_config, update_notice)
        }
        Cli::Maintenance { dry_run } => {
            let retention_days = config.storage.retention_days;
            let store = open_store(&data_dir, config)?;
            let status = store.maintenance_status()?;
            println!("Maintenance status:");
            println!("  Vacuum overdue:         {}", status.vacuum_overdue);
            println!("  Last vacuum:            {}", status.last_vacuum_at);
            println!("  Purge candidates:       {}", status.purge_candidates);
            if dry_run {
                println!("\n(dry-run: no changes made)");
                return Ok(());
            }
            println!();
            let decayed = store.apply_confidence_decay()?;
            if decayed > 0 { println!("Confidence decay applied to {} memory/memories.", decayed); }
            let purged = store.purge_soft_deleted(retention_days)?;
            if purged > 0 { println!("Purged {} soft-deleted memory/memories.", purged); }
            if status.vacuum_overdue {
                store.vacuum()?;
                println!("VACUUM complete.");
            }
            println!("Maintenance done.");
            Ok(())
        }
        Cli::Vacuum => {
            let db_path = data_dir.join("memory.db");
            let before_kb = std::fs::metadata(&db_path).map(|m| m.len() / 1024).unwrap_or(0);
            let store = open_store(&data_dir, config)?;
            store.vacuum()?;
            let after_kb = std::fs::metadata(&db_path).map(|m| m.len() / 1024).unwrap_or(0);
            println!("VACUUM complete. DB size: {} KB -> {} KB", before_kb, after_kb);
            Ok(())
        }
        Cli::AutoContext { prompt, scope, limit } => {
            let store = open_store(&data_dir, config)?;
            let llm_tier = llm::detect_tier(&llm_config).await;
            let keywords = llm::extract_keywords(&llm_tier, &prompt).await;
            let query = keywords.join(" ");

            if query.is_empty() {
                return Ok(());
            }

            let tracker_key = scope.as_deref().unwrap_or("/");
            let mut tracker = InjectionTracker::load(&data_dir, tracker_key);

            let results = store.search(memory_core::SearchParams {
                query: query.clone(),
                scope: scope.clone(),
                source_type: None,
                limit: Some(limit),
            })?;

            // Filter out already-injected memories
            let new_results: Vec<_> = results.iter()
                .filter(|r| !tracker.seen(r.id))
                .collect();

            if new_results.is_empty() {
                // Try context-based fallback, also deduped
                if let Some(ref s) = scope {
                    let ctx = store.context(Some(s), Some(limit))?;
                    let new_ctx: Vec<_> = ctx.iter()
                        .filter(|m| !tracker.seen(m.id))
                        .take(limit as usize)
                        .collect();
                    if !new_ctx.is_empty() {
                        let ctx_ids: Vec<i64> = new_ctx.iter().map(|m| m.id).collect();
                        let total_tokens: i32 = new_ctx.iter()
                            .map(|m| memory_core::autonomous::adaptive::estimate_tokens(&m.value))
                            .sum();
                        let tokens_per = if ctx_ids.is_empty() { 0 } else { total_tokens / ctx_ids.len() as i32 };
                        let _ = store.record_injection(&ctx_ids, tokens_per);
                        println!("[memory-agent] Relevant project context:");
                        for m in &new_ctx {
                            println!("  [{}] {}: {}", m.key, m.scope, memory_core::make_preview(&m.value, 80));
                            tracker.mark(m.id);
                        }
                    }
                }
                tracker.save(&data_dir);
                return Ok(());
            }

            let ids: Vec<i64> = new_results.iter().map(|r| r.id).collect();
            let total_tokens: i32 = new_results.iter()
                .map(|r| memory_core::autonomous::adaptive::estimate_tokens(&r.value_preview))
                .sum();
            let tokens_per = if ids.is_empty() { 0 } else { total_tokens / ids.len() as i32 };
            let _ = store.record_injection(&ids, tokens_per);

            println!("[memory-agent] Memories relevant to your prompt:");
            for r in &new_results {
                println!("  [{}] {}: {}", r.key, r.scope, r.value_preview);
                tracker.mark(r.id);
            }
            tracker.save(&data_dir);
            Ok(())
        }
        Cli::ObserveEdit { file_path, scope } => {
            let store = open_store(&data_dir, config)?;
            let filename = std::path::Path::new(&file_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.clone());

            let key = format!("observed/edit/{}", filename);
            let scope_val = scope.unwrap_or_else(|| "/".to_string());
            let value = format!("File edited: {}", file_path);

            // Use save (upsert) — if same file edited again, it bumps the revision
            let _ = store.save(memory_core::SaveParams {
                key,
                value,
                scope: Some(scope_val.clone()),
                source_type: Some(memory_core::types::SourceType::Observed),
                source_ref: Some(file_path),
                source_commit: None,
                tags: Some(vec!["observed".to_string(), "edit".to_string()]),
            });

            // Implicit hit tracking: if any injected memories share the scope of
            // the edited file, the agent likely used them — record as hits.
            let tracker = InjectionTracker::load(&data_dir, &scope_val);
            let injected_ids: Vec<i64> = tracker.ids.iter().copied().collect();
            if !injected_ids.is_empty() {
                let mut hit_ids = Vec::new();
                for id in &injected_ids {
                    if let Ok(mem) = store.get(*id) {
                        // Hit if the memory's scope is an ancestor of the edited file's scope
                        // or matches exactly
                        if scope_val.starts_with(&mem.scope) || mem.scope == "/" {
                            hit_ids.push(*id);
                        }
                    }
                }
                if !hit_ids.is_empty() {
                    let _ = store.record_hit_batch(&hit_ids);
                }
            }

            Ok(())
        }
        Cli::PreCompact { transcript, scope } => {
            let llm_tier = llm::detect_tier(&llm_config).await;

            // Read last portion of transcript
            let content = std::fs::read_to_string(&transcript)?;
            let lines: Vec<&str> = content.lines().collect();
            let recent = lines.iter().rev().take(50).rev().cloned().collect::<Vec<_>>();

            // Extract user and assistant messages
            let mut excerpt = String::new();
            for line in &recent {
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) {
                    let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                    if role == "user" || role == "assistant" {
                        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                            let owned: String;
                            let preview = if content.len() > 300 {
                                owned = content.chars().take(300).collect();
                                owned.as_str()
                            } else {
                                content
                            };
                            excerpt.push_str(&format!("{}: {}\n", role, preview));
                        }
                    }
                }
            }

            if excerpt.len() < 50 {
                return Ok(()); // Not enough content to extract from
            }

            let learnings = llm::extract_learnings(&llm_tier, &excerpt).await;

            if learnings.is_empty() {
                return Ok(());
            }

            let store = open_store(&data_dir, config)?;
            let scope_val = scope.unwrap_or_else(|| "/".to_string());
            let timestamp = chrono_now();

            let mut saved = 0;
            for (i, learning) in learnings.iter().enumerate() {
                let key = format!("learned/compact-{}-{}", timestamp, i);
                let result = store.save(memory_core::SaveParams {
                    key,
                    value: learning.clone(),
                    scope: Some(scope_val.clone()),
                    source_type: Some(memory_core::types::SourceType::Derived),
                    source_ref: None,
                    source_commit: None,
                    tags: Some(vec!["learned".to_string(), "pre-compact".to_string()]),
                });
                if let Ok(memory_core::SaveAction::Created(_)) = result {
                    saved += 1;
                }
            }

            if saved > 0 {
                eprintln!("[memory-agent] Saved {} learnings before compaction.", saved);
            }
            Ok(())
        }
        Cli::InstructionsLoaded { file_path, scope } => {
            let file = std::path::Path::new(&file_path);
            if !file.exists() {
                return Ok(());
            }

            let content = std::fs::read_to_string(file)?;
            let hash = blake3_hash(content.as_bytes()).to_hex().to_string();
            let store = open_store(&data_dir, config)?;

            // Detect project scope from the file's parent directory
            let scope_val = scope.unwrap_or_else(|| {
                file.parent()
                    .map(source::extract::scope_from_directory)
                    .unwrap_or_else(|| "/".to_string())
            });

            let meta_key = format!("instructions_hash:{}", file_path);
            let stored_hash = store.get_metadata(&meta_key).ok().flatten();

            let changed = stored_hash.as_deref() != Some(&hash);

            if changed {
                // Re-extract memories from this file
                let filename = file.file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("unknown");

                let extractor = match filename {
                    "CLAUDE.md" => Some(source::config::extract_claude_md as fn(&str, &str) -> Vec<memory_core::types::ExtractedMemory>),
                    ".cursorrules" | ".windsurfrules" => Some(source::config::extract_rules_file as fn(&str, &str) -> Vec<memory_core::types::ExtractedMemory>),
                    _ => None,
                };

                if let Some(extract_fn) = extractor {
                    let memories = extract_fn(&content, &file_path);
                    let mut extracted = 0;
                    let mut updated = 0;
                    for mem in memories {
                        let action = store.save(memory_core::SaveParams {
                            key: mem.key,
                            value: mem.value,
                            scope: Some(scope_val.clone()),
                            source_type: Some(mem.source_type),
                            source_ref: Some(mem.source_ref),
                            source_commit: None,
                            tags: Some(mem.tags),
                        })?;
                        match action {
                            memory_core::SaveAction::Created(_) => extracted += 1,
                            memory_core::SaveAction::Updated(_) => updated += 1,
                            memory_core::SaveAction::Deduplicated(_) => {}
                        }
                    }

                    if extracted > 0 || updated > 0 {
                        eprintln!("[memory-agent] Instructions changed: {} new, {} updated from {}", extracted, updated, filename);
                    }
                }

                // Store the new hash
                store.set_metadata(&meta_key, &hash)?;
            }

            // Inject delta: memories in this scope that AREN'T from this file
            let mut tracker = InjectionTracker::load(&data_dir, &scope_val);
            let all = store.list(Some(&scope_val), None, Some(50))?;
            let delta: Vec<_> = all.iter()
                .filter(|m| {
                    m.source_ref.as_deref() != Some(&*file_path)
                        && m.source_type != memory_core::types::SourceType::Observed
                        && !tracker.seen(m.id)
                })
                .take(10)
                .collect();

            if !delta.is_empty() {
                let delta_ids: Vec<i64> = delta.iter().map(|m| m.id).collect();
                let _ = store.record_injection(&delta_ids, 0);
                let mut context = String::new();
                context.push_str("[memory-agent] Additional context not in instructions:\n");
                for m in &delta {
                    context.push_str(&format!(
                        "  [{}] {}: {}\n",
                        m.key,
                        m.source_type,
                        memory_core::make_preview(&m.value, 80),
                    ));
                    tracker.mark(m.id);
                }
                let json = serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "InstructionsLoaded",
                        "additionalContext": context
                    }
                });
                println!("{}", json);
            }
            tracker.save(&data_dir);

            Ok(())
        }
        Cli::HookInject { static_only, .. } => {
            if static_only {
                if let Some(ip) = &hooks_config.injection_prompt {
                    let trimmed = ip.as_str().trim();
                    if !trimmed.is_empty() {
                        println!("{}", trimmed);
                    }
                }
            }
            Ok(())
        }
        Cli::HookGate { feature } => {
            let enabled = match feature.as_str() {
                "agent_review_gate" => hooks_config.agent_review_gate,
                _ => false,
            };
            std::process::exit(if enabled { 0 } else { 1 });
        }
        Cli::Install { target, yes } => match target {
            InstallTarget::Claude => setup::install(&data_dir, yes),
            InstallTarget::Gemini => setup::install_agent(&data_dir, yes, setup::AgentTarget::Gemini),
            InstallTarget::Cursor => setup::install_agent(&data_dir, yes, setup::AgentTarget::Cursor),
            InstallTarget::Codex => setup::install_agent(&data_dir, yes, setup::AgentTarget::Codex),
            InstallTarget::Opencode => setup::install_opencode(&data_dir, yes),
            InstallTarget::Other => setup::install_other(&data_dir, yes),
        },
        Cli::Uninstall { target, purge, yes } => match target {
            InstallTarget::Claude => setup::uninstall(&data_dir, purge, yes),
            InstallTarget::Gemini => setup::uninstall_agent(purge, yes, setup::AgentTarget::Gemini),
            InstallTarget::Cursor => setup::uninstall_agent(purge, yes, setup::AgentTarget::Cursor),
            InstallTarget::Codex => setup::uninstall_agent(purge, yes, setup::AgentTarget::Codex),
            InstallTarget::Opencode => setup::uninstall_opencode(purge, yes),
            InstallTarget::Other => {
                println!("No automatic uninstall for generic MCP clients.");
                println!("Remove the \"memory\" entry from your MCP config manually.");
                Ok(())
            }
        },
        Cli::Export => {
            let store = open_store(&data_dir, config)?;
            let all = store.list(None, None, Some(100000))?;
            println!("{}", serde_json::to_string_pretty(&all)?);
            Ok(())
        }
        Cli::Import => {
            let store = open_store(&data_dir, config)?;
            let input = std::io::read_to_string(std::io::stdin())?;
            let memories: Vec<memory_core::Memory> = serde_json::from_str(&input)?;
            let mut created = 0;
            for m in memories {
                let action = store.save(memory_core::SaveParams {
                    key: m.key,
                    value: m.value,
                    scope: Some(m.scope),
                    source_type: Some(m.source_type),
                    source_ref: m.source_ref,
                    source_commit: m.source_commit,
                    tags: m.tags,
                })?;
                if matches!(action, memory_core::SaveAction::Created(_)) {
                    created += 1;
                }
            }
            println!("Imported {} new memories.", created);
            Ok(())
        }
    }
}

fn open_store(data_dir: &std::path::Path, config: memory_core::Config) -> anyhow::Result<Store> {
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join("memory.db");
    let db_str = db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {:?}", db_path))?;

    // Guard against config/DB mismatch: config says encrypted but DB is plaintext
    if config.storage.encryption_enabled && db_path.exists() && !Store::is_encrypted(db_str) {
        eprintln!("Warning: config says encryption_enabled but DB is plaintext.");
        eprintln!("Opening without passphrase. Run `memory-agent config encryption status` to investigate.");
        let mut plain_config = config;
        plain_config.storage.encryption_enabled = false;
        return Ok(Store::open(db_str, plain_config, None)?);
    }

    let passphrase = resolve_passphrase(&config)?;
    Ok(Store::open(db_str, config, passphrase.as_deref())?)
}

fn resolve_passphrase(config: &memory_core::Config) -> anyhow::Result<Option<String>> {
    if config.storage.encryption_enabled {
        config_loader::retrieve_passphrase()
    } else {
        Ok(None)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", memory_core::safe_truncate(s, max.saturating_sub(3)))
    } else {
        s.to_string()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{}", tokens)
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("MEMORY_AGENT_LOG_LEVEL")
        )
        .init();
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{ts}")
}

fn blake3_hash(data: &[u8]) -> blake3::Hash {
    blake3::hash(data)
}

/// Tracks which memory IDs have been injected for a scope to avoid duplicates.
/// Persists to `~/.memory-agent/injected-{scope_slug}.ids`.
struct InjectionTracker {
    scope_slug: String,
    pub ids: std::collections::HashSet<i64>,
}

impl InjectionTracker {
    fn load(data_dir: &std::path::Path, scope: &str) -> Self {
        // Convert scope to a safe filename slug (replace '/' with '-', strip leading '-')
        let slug: String = scope
            .chars()
            .map(|c| if c == '/' || c == '\\' { '-' } else { c })
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let slug = slug.trim_matches('-').to_string();
        let slug = if slug.is_empty() { "root".to_string() } else { slug };

        let mut ids = std::collections::HashSet::new();
        let path = data_dir.join(format!("injected-{slug}.ids"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                if let Ok(id) = line.trim().parse::<i64>() {
                    ids.insert(id);
                }
            }
        }
        Self { scope_slug: slug, ids }
    }

    fn seen(&self, id: i64) -> bool {
        self.ids.contains(&id)
    }

    fn mark(&mut self, id: i64) {
        self.ids.insert(id);
    }

    fn save(&self, data_dir: &std::path::Path) {
        let path = data_dir.join(format!("injected-{}.ids", self.scope_slug));
        let content: String = self.ids.iter().map(|id| format!("{id}\n")).collect();
        std::fs::write(&path, content).ok();
    }
}

fn config_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("config.toml")
}

fn set_config_value(data_dir: &std::path::Path, key: &str, value: &str) -> anyhow::Result<()> {
    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        config_loader::create_default_config(data_dir)?;
    }
    let content = std::fs::read_to_string(&config_path)?;

    // Reject malformed TOML before making any changes.
    if let Err(e) = toml::from_str::<toml::Value>(&content) {
        anyhow::bail!("config.toml is malformed and cannot be safely updated: {e}");
    }

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Look for the key (commented or not) under [storage]
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed == format!("{} = {}", key, value)
            || trimmed == format!("# {} = {}", key, value)
            || trimmed.starts_with(&format!("{} =", key))
            || trimmed.starts_with(&format!("# {} =", key))
        {
            *line = format!("{} = {}", key, value);
            found = true;
            break;
        }
    }

    if !found {
        if let Some(pos) = lines.iter().position(|l| l.trim() == "[storage]") {
            let insert_at = lines
                .iter()
                .enumerate()
                .skip(pos + 1)
                .find(|(_, l)| l.trim().starts_with('['))
                .map(|(i, _)| i)
                .unwrap_or(lines.len());
            lines.insert(insert_at, format!("{} = {}", key, value));
        } else {
            lines.push(String::new());
            lines.push("[storage]".to_string());
            lines.push(format!("{} = {}", key, value));
        }
    }

    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    std::fs::write(&config_path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_config_value_updates_existing_key() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(
            &config,
            "[storage]\nencryption_enabled = true\nother = 1\n",
        )
        .unwrap();

        set_config_value(dir.path(), "encryption_enabled", "false").unwrap();

        let content = std::fs::read_to_string(&config).unwrap();
        assert_eq!(
            content.matches("encryption_enabled").count(),
            1,
            "should not duplicate key"
        );
        assert!(content.contains("encryption_enabled = false"));
    }

    #[test]
    fn set_config_value_updates_commented_key() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(
            &config,
            "[storage]\n# encryption_enabled = true\n",
        )
        .unwrap();

        set_config_value(dir.path(), "encryption_enabled", "false").unwrap();

        let content = std::fs::read_to_string(&config).unwrap();
        assert!(content.contains("encryption_enabled = false"));
        assert!(!content.contains("# encryption_enabled"));
    }

    #[test]
    fn set_config_value_creates_storage_section_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "[search]\ndefault_limit = 10\n").unwrap();

        set_config_value(dir.path(), "encryption_enabled", "true").unwrap();

        let content = std::fs::read_to_string(&config).unwrap();
        assert!(content.contains("[storage]"));
        assert!(content.contains("encryption_enabled = true"));
    }

    #[test]
    fn set_config_value_no_duplicate_on_repeated_calls() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(
            &config,
            "[storage]\nencryption_enabled = true\n",
        )
        .unwrap();

        set_config_value(dir.path(), "encryption_enabled", "false").unwrap();
        set_config_value(dir.path(), "encryption_enabled", "true").unwrap();
        set_config_value(dir.path(), "encryption_enabled", "false").unwrap();

        let content = std::fs::read_to_string(&config).unwrap();
        assert_eq!(
            content.matches("encryption_enabled").count(),
            1,
            "repeated calls should not create duplicates"
        );
        assert!(content.contains("encryption_enabled = false"));
    }
}
