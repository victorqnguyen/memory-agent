# memory-agent

Persistent memory for AI coding agents. One binary, zero cloud, works with any MCP-compatible agent.

AI coding agents forget everything between sessions — architecture decisions, project conventions, debugging insights, what worked and what didn't. memory-agent fixes this with an encrypted local database that persists knowledge across sessions and makes it searchable via the [Model Context Protocol](https://modelcontextprotocol.io/).

## Install

```bash
cargo install memory-agent
memory-agent install claude    # or: gemini, cursor, codex
```

The installer walks you through config, encryption, LLM setup (Ollama or skip), and MCP registration. Verify with `memory-agent doctor`. Requires [Rust 1.85+](https://rustup.rs/).

## Why memory-agent?

### What makes it different

- **Encrypted at rest** — SQLCipher database encryption with passphrase stored in your system keychain. Enable with one command, decrypt just as easily. Your memories never touch disk unprotected.
- **Sandboxed local LLM** — optional Ollama integration (Qwen 3.5 2B) for smarter session summaries, memory consolidation, keyword extraction, and procedural learning. The LLM is strictly text-in/text-out — it cannot read files, execute commands, access the database, or make network calls. It only receives memory text snippets and returns processed text. Locked to localhost; remote URLs are rejected.
- **Token ROI tracking** — every injected memory is measured for actual usefulness. Know which memories earn their tokens and which are dead weight. `memory-agent doctor` surfaces actionable insights.
- **Git-aware staleness** — memories linked to source files are automatically flagged when those files change via `git diff`. No more acting on outdated knowledge.
- **Intelligent hygiene** — entropy filtering prevents low-information saves, stale session detection, automatic maintenance with confidence decay and cleanup pipelines.
- **Codebase as memory source** — extract memories directly from `package.json`, `Cargo.toml`, `CLAUDE.md`, `.cursorrules`, `.windsurfrules`, and `Makefile`. Your project config _is_ memory. Exclude files with `.memory-agentignore` (`.env*` ignored by default).
- **Scope hierarchy** — organize memories by project, subsystem, or branch with CSS-specificity-style inheritance. A memory at `/my-project` is visible there but not globally; `/` is visible everywhere.

### Everything else

- **15 MCP tools** — full CRUD, search, sessions, consolidation, metrics, relationships, skills tracking, token budgeting, maintenance operations
- **FTS5 full-text search** — weighted BM25 ranking, AND→OR fallback for partial-term recall, adaptive preview, injection-safe query sanitization
- **Agent-agnostic** — Claude Code, Opencode, Gemini CLI (not tested for bugs), Cursor (not tested for bugs), Codex (not tested for bugs), or any MCP client
- **Privacy by default** — secrets (API keys, tokens, passwords, connection strings) are stripped before storage via pattern matching. `<private>` tags are honored. Sensitive files (`.env`, `*.pem`, `*.key`) are never read.
- **Content deduplication** — blake3 hashing prevents duplicate writes within a configurable window
- **Procedural learning** — skills system records what worked and what didn't, then retrieves those learnings before the next execution
- **Hook integration** — 10 Claude Code hook scripts across 9 events auto-inject context on prompts, record file edits, extract learnings on compaction. TypeScript plugin for OpenCode with 5 event handlers.
- **TUI dashboard** — browse memories, sessions, activity log, metrics, scopes, and hook config in your terminal. End active sessions, view maintenance status.

## Supported Agents

```bash
memory-agent install claude    # Claude Code — MCP, CLAUDE.md, hooks, slash commands
memory-agent install gemini    # Gemini CLI — ~/.gemini/settings.json
memory-agent install cursor    # Cursor — ~/.cursor/mcp.json
memory-agent install codex     # OpenAI Codex — ~/.codex/config.json
memory-agent install opencode  # OpenCode — LSP integration, TypeScript plugin
```

## How It Works

```
┌─────────────────┐     MCP (stdio)     ┌──────────────────┐
│  AI Agent        │◄──────────────────►│  memory-agent     │
│  (Claude, etc.)  │                     │  (17 tools)       │
└─────────────────┘                     └────────┬─────────┘
                                                 │
                                        ┌────────▼─────────┐
                                        │  SQLCipher + FTS5 │
                                        │  ~/.memory-agent/ │
                                        └────────┬─────────┘
                                                 │
                                        ┌────────▼─────────┐
                                        │  Local LLM        │
                                        │  (Ollama, opt.)   │
                                        └──────────────────┘
```

1. Agent starts a session → `memory_session_start`
2. Relevant memories are loaded → `memory_context`
3. During work, knowledge is saved → `memory_save`
4. Before solving problems, past knowledge is searched → `memory_search`
5. Session ends with a summary → `memory_session_end` (LLM-generated if available)

For **Claude Code**, steps 1, 2, and 5 are automated by hooks — only `memory_save` requires an explicit call. For all other tools (Cursor, Windsurf, Gemini CLI, etc.), all steps apply.

### Security Model

| Layer | What happens |
|-|-|
| **Input** | Secrets stripped (AWS keys, GitHub tokens, API keys, DB URLs, private keys). FTS5 queries sanitized against injection. |
| **Storage** | Optional SQLCipher encryption. Passphrase in system keychain, never in config files. |
| **LLM** | Text-in/text-out only — no file access, no command execution, no tools, no agent loop. Locked to localhost (`127.0.0.1`, `::1`). Remote URLs rejected. Output sanitized and length-capped before storage. |
| **Output** | Progressive disclosure — compact summaries by default, full content only on explicit detail request. |

### LLM Tiers

memory-agent works without any LLM. When Ollama is available, it enhances specific operations:

| Operation | Without LLM (Tier 1) | With Ollama (Tier 2) |
|-|-|-|
| Session summaries | Activity list | Natural language summary |
| Memory consolidation | Line-union merge | Semantic merge |
| Skill learning | Pattern-header extraction | Full procedural extraction |
| Keyword extraction | N/A | 5-keyword extraction from prompts |
| Metrics insights | Rule-based thresholds | Actionable recommendations |

The LLM has no capabilities beyond text generation — no file access, no tool use, no database access. It receives only the specific memory text being processed and returns a text response. All output is sanitized (null bytes stripped, length-capped to 2KB) before entering storage.

## CLI

```bash
# Memory CRUD
memory-agent save -k "auth/flow" -v "Uses JWT with refresh tokens" --scope "/my-app"
memory-agent search "authentication" -s "/my-app"
memory-agent list -s "/my-app"
memory-agent detail 42
memory-agent delete -k "auth/flow"

# Project intelligence
memory-agent extract -d /path/to/project    # auto-import from project files
memory-agent stale -d /path/to/project      # find outdated memories via git
memory-agent metrics                         # token efficiency stats
memory-agent doctor                          # health check dashboard

# Sessions
memory-agent session-start my-project
memory-agent session-list --active
memory-agent session-end my-project -s "Implemented auth flow"
memory-agent session-end --stale            # end sessions older than 24 hours
memory-agent session-end --stale --older-than 48  # custom stale threshold

# Data management
memory-agent export > backup.json
memory-agent import < backup.json
memory-agent maintenance                     # run hygiene pipeline (purge, vacuum, decay)
memory-agent maintenance --dry-run           # preview maintenance actions
memory-agent vacuum                          # compact database
memory-agent tui                             # interactive terminal UI
memory-agent update                          # check for a newer version

# TUI keybindings
#   /         search memories
#   Enter     view detail (or expand scope)
#   d         delete memory (prompts confirmation)
#               y = soft delete, Y = hard delete, any other = cancel
#   e         end session (in Activity view, prompts confirmation)
#   r         refresh
#   j/k       scroll up/down
#   Tab       switch view (Search → Sessions → Activity → Metrics → Scopes → Hooks)
#   Esc       back / cancel
#   q         quit

# Configuration
memory-agent config encryption enable        # encrypt database
memory-agent config encryption status
memory-agent config llm                      # configure Ollama
```

## Architecture

```
memory-agent/
├── crates/
│   ├── memory-core/     # sync lib — SQLCipher, FTS5, dedup, privacy, search
│   ├── memory-agent/    # async bin — MCP server, CLI, hooks, LLM
├── LICENSE
├── LICENSE-COMMERCIAL.md
└── Cargo.toml           # workspace root
```

- **memory-core** — synchronous, no async runtime. All storage, search, dedup, privacy, and encryption logic.
- **memory-agent** — async MCP server, CLI, hook system, local LLM integration. Wraps core via `AsyncStore`.

## Requirements

- **Rust 1.85+** for `cargo install`
- An MCP-compatible AI agent
- **Optional:** [Ollama](https://ollama.com) for local LLM features (`ollama pull qwen3.5:2b`)
- **Optional:** `jq` for Claude Code hook integration

## Configuration

All config lives in `~/.memory-agent/config.toml` (created by `memory-agent init`):

```toml
log_level = "info"

[storage]
retention_days = 90
dedup_window_secs = 900
encryption_enabled = false       # enable via: memory-agent config encryption enable

[maintenance]
entropy_threshold = 0.35         # filter low-information content
stale_session_hours = 24         # auto-end sessions older than this
confidence_decay_enabled = true

[privacy]
extra_patterns = []              # additional secret-detection regexes
replace_defaults = false

[llm]
ollama_url = "http://localhost:11434"
ollama_model = "qwen3.5:2b"
timeout_secs = 30
```

Override any value with `MEMORY_AGENT_<SECTION>_<KEY>` environment variables.

### `.memory-agentignore`

Place a `.memory-agentignore` file in your project root to exclude files from `memory-agent extract`. One pattern per line, `*` glob supported. By default, `.env*` files are excluded:

```
# .memory-agentignore (auto-created on first extract)
.env*
```

To allow `.env.example` extraction, remove or edit this file.

## Uninstall

```bash
memory-agent uninstall claude         # remove integration (keeps data)
memory-agent uninstall claude --purge # remove everything including data
memory-agent uninstall gemini
memory-agent uninstall cursor
memory-agent uninstall codex
memory-agent uninstall opencode
```

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

```bash
git clone https://github.com/victorqnguyen/memory-agent
cd memory-agent
cargo test                    # run full test suite (192 tests)
cargo clippy -- -D warnings   # lint (zero warnings policy)
```

## License

Open-core model:

- **Core + CLI** — [MIT](LICENSE) (free forever)
- **Cloud/team features** (future) — [Commercial](LICENSE-COMMERCIAL.md)
