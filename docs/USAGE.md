# Memory Agent Usage Guide

## Installation

### Quick install (recommended)

```bash
cargo install memory-agent
memory-agent install claude    # or: gemini, cursor, codex, opencode
```

This will interactively:
1. Create default config (`~/.memory-agent/config.toml`)
2. Register as global MCP server (`claude mcp add -s user`)
3. Add Memory Agent instructions to `~/.claude/CLAUDE.md`
4. Copy slash commands to `~/.claude/commands/`

Use `--yes` to skip confirmation prompts.

For OpenCode-specific setup (different config format + LSP notes):
see [docs/guides/opencode.md](guides/opencode.md)

### Manual install

If you prefer to set things up yourself:

#### 1. Install the binary

```bash
cargo install memory-agent
```

#### 2. Register as MCP server (global — all Claude Code sessions)

```bash
claude mcp add -s user memory-agent -- /path/to/memory-agent mcp
```

Or per-project only, create `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "/path/to/memory-agent",
      "args": ["mcp"]
    }
  }
}
```

#### 3. Add instructions to CLAUDE.md

Add this to `~/.claude/CLAUDE.md` so Claude knows to save knowledge it discovers:

```markdown
## Memory Agent
When you learn something important about a project (patterns, conventions, architecture decisions, bugs), save it with `memory_save`.
```

Session start/end and context loading are handled automatically by the hooks installed in step 2.

#### 4. Initialize config (optional)

```bash
memory-agent init
```

Creates `~/.memory-agent/config.toml` with defaults you can customize.

---

## Uninstallation

Remove everything that was added during installation, without touching source files:

```bash
memory-agent uninstall claude
```

This removes:
1. Global MCP server registration
2. Memory Agent section from `~/.claude/CLAUDE.md`
3. Slash commands (`memory-*.md`) from `~/.claude/commands/`

Data is preserved by default. To also delete the database and config:

```bash
memory-agent uninstall claude --purge
```

Use `--yes` to skip confirmation prompts.

---

## Data Location

All data lives in `~/.memory-agent/`:

| File | Purpose |
|-|-|
| `memory.db` | SQLite database (all memories, sessions, metrics) |
| `config.toml` | Configuration (created by `memory-agent init`) |

Override with `MEMORY_AGENT_DATA_DIR` env var.

---

## CLI Commands

### Save a memory

```bash
memory-agent save -k "project/convention" -v "Always use snake_case for functions" --scope "/my-project" -t "style,conventions"
```

| Flag | Required | Description |
|-|-|-|
| `-k, --key` | Yes | Unique identifier (e.g., `project/auth-flow`) |
| `-v, --value` | Yes | The memory content (max 2000 chars) |
| `--scope` | No | Project/directory scope (default: `/`) |
| `--source-type` | No | `explicit`, `codebase`, `observed`, or `derived` |
| `-t, --tags` | No | Comma-separated tags |

If a memory with the same key+scope exists, it's updated (revision tracked).

### Search memories

```bash
memory-agent search "authentication"
memory-agent search "rust patterns" -s "/my-project" -l 20
```

| Flag | Description |
|-|-|
| `-s, --scope` | Filter by scope |
| `-l, --limit` | Max results (default: 10) |

### List memories

```bash
memory-agent list
memory-agent list -s "/my-project" --source-type codebase -l 50
```

### View full detail

```bash
memory-agent detail 42
```

Shows complete content, metadata, tags, session info, revision count.

### Delete a memory

```bash
memory-agent delete -k "project/old-pattern"              # soft delete
memory-agent delete -k "project/old-pattern" --hard        # permanent
memory-agent delete -k "project/old-pattern" --scope "/x"  # specific scope
```

Soft-deleted memories are purged after 90 days (configurable).

### Extract from project files

```bash
memory-agent extract                        # current directory
memory-agent extract -d /path/to/project    # specific project
memory-agent extract -d . --scope "/my-app" # custom scope
```

Scans and auto-imports from:
- `package.json` — name, scripts, dependencies
- `Cargo.toml` — package info, dependencies
- `.env.example` / `.env.local` — variable names (never values)
- `CLAUDE.md` — chunks by heading

### Check for stale memories

```bash
memory-agent stale
memory-agent stale -s "/my-project" -d /path/to/project
```

Compares memories with `source_ref` against git history. Reports which memories reference files that have changed since they were recorded.

### View metrics

```bash
memory-agent metrics
```

Shows which memories are being injected into context and how often they lead to hits (usage). Helps identify low-value memories to prune.

### Health check (doctor)

```bash
memory-agent doctor
```

Single dashboard showing everything that's working and what's not:
- Memory counts by source type
- Dedup saves and revision counts
- Session status
- Injection/hit metrics and hit rate
- Access log activity (search, detail, context, list)
- Top search queries
- Stale memory count
- Database size and vacuum status
- Actionable issues list

### Statistics

```bash
memory-agent stats
```

Shows total memories, breakdown by source type, session counts.

### Export / Import

```bash
memory-agent export > backup.json     # dump everything
memory-agent import < backup.json     # restore from backup
```

### Version info

```bash
memory-agent version
```

Shows binary version, protocol version, schema version, data directory.

### Show config

```bash
memory-agent config          # print effective config
memory-agent config --path   # print config file location
```

---

## MCP Tools (used by Claude)

When registered as an MCP server, Claude has access to these tools:

| Tool | Description |
|-|-|
| `memory_save` | Save or update a memory (key, value, scope, tags) |
| `memory_search` | Full-text search across memories |
| `memory_detail` | Get complete content of a memory by ID |
| `memory_delete` | Soft or hard delete by key+scope |
| `memory_list` | List memories with scope/source filters |
| `memory_context` | Get scope-aware relevant memories for context injection |
| `memory_budget` | Get best memories that fit within a token budget |
| `memory_extract` | Extract memories from project config files |
| `memory_stale` | Check for memories with changed source files |
| `memory_metrics` | Get token efficiency metrics |
| `memory_relate` | Create relationships between memories |
| `memory_relations` | Get relationships for a memory |
| `memory_consolidate` | Find and merge similar memories (dry run by default) |
| `memory_skill_start` | Load procedural memories before a skill runs |
| `memory_skill_end` | Record outcome and learnings after a skill runs |

---

## TUI (Terminal UI)

```bash
memory-agent tui
```

Interactive terminal interface with four tabs:

| Tab | Navigate | Description |
|-|-|-|
| Search | `/` to type, `Enter` to search, `Enter` on result for detail | Browse and search all memories |
| Sessions | `Tab` to reach | View all sessions (active/ended) |
| Metrics | `Tab` to reach | Token efficiency — injections, hits, hit rates |
| Scopes | `Tab` to reach | Tree view of all scopes and memory counts |

### TUI Keybindings

| Key | Action |
|-|-|
| `Tab` / `Shift+Tab` | Next / previous tab |
| `j` / `k` or arrows | Scroll up/down |
| `/` | Enter search mode (Search tab) |
| `Enter` | Open detail view / execute search |
| `Esc` | Back to list / cancel search |
| `q` | Quit |

---

## Project Onboarding Workflow

To set up memory-agent for a new project:

1. **Extract config files** — auto-populates memories from project files:
   ```bash
   memory-agent extract -d /path/to/project
   ```

2. **Ask Claude to audit** — in a Claude session in that project, say:
   > "Scan this project and save key architecture decisions, patterns, conventions, and important file paths using memory_save. Use scope '/project-name'."

3. **Review what was saved**:
   ```bash
   memory-agent list -s "/project-name"
   memory-agent tui   # or use the TUI
   ```

4. **Prune and refine** — delete anything wrong or redundant:
   ```bash
   memory-agent delete -k "some/bad-memory" --scope "/project-name"
   ```

5. **Check health over time**:
   ```bash
   memory-agent metrics   # are memories being used?
   memory-agent stale     # are any outdated?
   ```

---

## Scopes

Scopes are hierarchical paths that organize memories by project or context.

- `/` — global (applies everywhere)
- `/my-project` — project-specific
- `/my-project/backend` — subsystem-specific

When Claude calls `memory_context`, it gets memories from the current scope AND parent scopes. A memory at `/` is visible everywhere. A memory at `/my-project` is only visible in that project.

Scope is auto-detected from the directory name during `extract`, or you can set it explicitly.

---

## Source Types

| Type | Meaning |
|-|-|
| `explicit` | Manually saved by user or agent (default) |
| `codebase` | Extracted from project files |
| `observed` | Inferred from agent behavior/patterns |
| `derived` | Generated by consolidation or compression |

---

## Configuration

After `memory-agent init`, edit `~/.memory-agent/config.toml`:

```toml
# Log level: error, warn, info, debug, trace
log_level = "info"

# Maximum key length (chars)
max_key_length = 256

# Maximum value length (chars)
max_value_length = 2000

# Days before soft-deleted memories are purged
retention_days = 90

# Seconds between automatic VACUUM runs (default: 1 week)
vacuum_interval_secs = 604800

[privacy]
# Extra regex patterns to strip from content before storage
# Built-in patterns catch common secrets (API keys, tokens, passwords)
extra_patterns = []

# Set true to replace built-in patterns entirely
replace_defaults = false
```

Environment variable overrides use `MEMORY_AGENT_<SECTION>_<KEY>`:
```bash
export MEMORY_AGENT_LOG_LEVEL=debug
export MEMORY_AGENT_RETENTION_DAYS=30
```

---

## Memory Relationships

Memories can be linked:

```
memory_relate(source_id=1, target_id=2, relation="derived_from")
```

| Relation | Meaning |
|-|-|
| `derived_from` | Memory was derived from another |
| `supersedes` | Memory replaces an older one |
| `conflicts_with` | Memories contradict each other |
| `related_to` | General association |

View with `memory_relations(id=1)`.

---

## Consolidation

Over time, similar memories accumulate. Consolidate merges them:

```
memory_consolidate(scope="/my-project", threshold=0.85, dry_run=true)
```

- `threshold` (0.0-1.0): similarity threshold for merging (default 0.85)
- `dry_run` (default true): preview groups without merging

Set `dry_run=false` to actually merge. The merged memory keeps the highest-confidence version and tracks revision history.

---

## Skills Integration

Memory-agent tracks procedural knowledge per skill:

1. **Before a skill runs**: `memory_skill_start(skill_name="commit")` returns past learnings, overrides, and relevant context.
2. **After a skill completes**: `memory_skill_end(skill_name="commit", outcome="Pattern: always run clippy before committing")` stores the learning.

Outcome text with `Pattern:`, `Learned:`, or `Takeaway:` headers is automatically extracted as procedural memory.

---

## Agent Instructions

For your AI agent to proactively use memory-agent, you need to tell it when and how. The instructions differ depending on your tool:

**Claude Code** — hooks automate session lifecycle. Only one line is needed in `~/.claude/CLAUDE.md` (global) or a project's `CLAUDE.md`:

```markdown
## Memory Agent
When you learn something important about a project (patterns, conventions, architecture decisions, bugs), save it with `memory_save`.
```

The hooks handle the rest: session start/end, context loading, and memory injection are all automatic.

**Other agents** (Cursor, Windsurf, Gemini CLI, etc.) — add the full instruction set to their system prompt or rules file:

```markdown
## Memory Agent
At the start of every conversation, call `memory_context` with the current project scope to load relevant memories.
When you learn something important about a project (patterns, conventions, architecture decisions, bugs), save it with `memory_save`.
Before making changes to unfamiliar code, call `memory_search` to check if there are relevant memories.
```

Without these instructions, the MCP tools exist but the agent won't proactively use them. The full lifecycle:

1. **Context loading** — `memory_context` at conversation start (automated by hooks in Claude Code)
2. **Knowledge retrieval** — `memory_search` + `memory_detail` when needed (counts as hits)
3. **Knowledge capture** — `memory_save` when the agent discovers something worth remembering

You can tune these instructions. For example, to make Claude more aggressive about saving:

```markdown
After every significant code change, save a memory describing what was changed and why.
```

Or more conservative:

```markdown
Only save memories when explicitly asked, or when you discover a non-obvious pattern that would be hard to rediscover.
```

---

## Claude Code Slash Commands

If you're working in the memory-agent project (or copy `.claude/commands/` to another project), these slash commands are available:

| Command | What it does |
|-|-|
| `/memory-start` | Init session: extract configs, start session, load context |
| `/memory-audit` | Deep project scan: reads key files, saves 5+ architecture/convention memories |
| `/memory-review` | Health check: runs doctor, metrics, stale, suggests actions |
| `/memory-end` | End session: saves learnings, writes summary, shows final stats |
| `/memory-save <topic>` | Save a specific memory with proper scope and categorization |
| `/memory-search <query>` | Search and show relevant memories with full detail |

### Making commands available globally

Copy the commands to your global Claude config:

```bash
cp -r .claude/commands/ ~/.claude/commands/
```

Then `/memory-start`, `/memory-audit`, etc. work in any project.

### Typical workflow

```
/memory-start          # beginning of session
... do your work ...
/memory-save the auth system uses JWT with refresh tokens
... more work ...
/memory-review         # check health mid-session
/memory-end            # wrap up
```

---

## Troubleshooting

**MCP server not connecting**: Verify the binary path in your MCP config. Run `memory-agent version` to confirm it works.

**Context not loading at conversation start**: For Claude Code, verify the `session-start.sh` hook is installed (`memory-agent install claude`). For other tools, ensure the agent is explicitly calling `memory_context` — add the instruction to the agent's system prompt or rules file.

**Empty search results**: Memories may be in a different scope. Try `memory-agent list` with no filters to see everything.

**Stale check shows nothing**: Memories need `source_ref` and `source_commit` fields set. Extracted memories have these; manually saved ones may not.

**Database issues**: Check `memory-agent stats`. The database is at `~/.memory-agent/memory.db`. Export a backup before troubleshooting: `memory-agent export > backup.json`.
