# Memory Agent Setup: OpenCode

## Quick Install

```bash
memory-agent install opencode
```

This writes the MCP config to `~/.config/opencode/opencode.json` and prints
next steps. Use `--yes` to skip confirmation prompts.

## Prerequisites

Install the binary if not already done:

```bash
cargo install memory-agent
```

Or download a pre-built binary from the releases page and place it on your `PATH`.

## Manual Configuration

OpenCode reads MCP server config from `~/.config/opencode/opencode.json`
(global) or `opencode.json` in the project root (project-local).

> **Note:** OpenCode uses `mcp.<name>` with `type: "local"` and `command` as an array —
> different from the `mcpServers` key used by Cursor/Codex. Do not mix the formats.

```json
{
  "mcp": {
    "memory-agent": {
      "type": "local",
      "command": ["memory-agent", "mcp"]
    }
  }
}
```

Full binary path if `memory-agent` is not on PATH:

```json
"command": ["/home/user/.cargo/bin/memory-agent", "mcp"]
```

## LSP — Built-in, Zero Config

OpenCode ships with 30+ language servers and auto-starts the right one when
it detects the file type (rust-analyzer for `.rs`, pyright for `.py`, etc.).

**You do not need an IDE open.** Code intelligence features —
`findReferences`, `incomingCalls`, `hover`, `goToDefinition` — work in a
pure terminal workflow, unlike Claude Code where an external IDE must be
running with the LSP server already started.

For the memory-agent project (Rust), rust-analyzer starts automatically on
first `.rs` file access.

## Hook Integration (Session Lifecycle)

The `memory-agent install opencode` command optionally installs a TypeScript
plugin to `~/.config/opencode/plugins/memory-agent.ts`. This plugin handles:

| OpenCode event | What happens |
|-|-|
| `session.created` | Starts a memory session for the project |
| `session.idle` | Injects relevant memories before the AI responds |
| `file.edited` | Records file edits as observed memories |
| `session.compacted` | Extracts learnings before context compaction |
| `session.deleted` | Ends the session with a summary |

To install the plugin manually, copy it from the repo:

```
crates/memory-agent/plugins/opencode-plugin.ts → ~/.config/opencode/plugins/memory-agent.ts
```

If not using the plugin, add these to your OpenCode system prompt:

```
At conversation start: call memory_context with the current project scope.
Before solving a problem: call memory_search first.
After discoveries: call memory_save.
```

## Verification

Start a new OpenCode session and run:

```
memory_list
```

Response `{ "memories": [], "total": 0 }` confirms the MCP server is connected.

## Model Compatibility

Memories are model-agnostic and project-scoped. Switching between any
models available in OpenCode (Claude, GPT-4o, Gemini, etc.) within the same
project shares the same memory store. No migration needed when changing models.

## Data Location

To use a custom data directory, pass it via the `env` field:

```json
{
  "mcp": {
    "memory-agent": {
      "type": "local",
      "command": ["memory-agent", "mcp"],
      "environment": {
        "MEMORY_AGENT_DATA_DIR": "/path/to/your/data"
      }
    }
  }
}
```

## Troubleshooting

**Server not found:** Verify `memory-agent` is on PATH (`which memory-agent`).
Restart OpenCode after editing the config.

**Wrong config key:** If MCP tools don't appear, confirm the config uses
`mcp.<name>` with `type: "local"` — not `mcpServers` or `mcp.servers`.

**Database locked:** Only one `memory-agent mcp` process at a time.
Check with `ps aux | grep memory-agent`.

**View server logs:** Run `memory-agent mcp 2>memory.log` in a terminal and
inspect `memory.log` while reproducing the issue.
