# Memory Agent Setup: Claude Code

## Prerequisites

Install the binary:

```bash
cargo install memory-agent
```

Or download a pre-built binary from the releases page and place it on your `PATH`.

Verify it works:

```bash
memory-agent version
```

## Quick Install

```bash
memory-agent install claude
```

This registers the MCP server, adds instructions to `~/.claude/CLAUDE.md`, installs hooks and slash commands, and walks you through optional LLM and encryption setup.

To uninstall: `memory-agent uninstall claude` (add `--purge` to also remove all data)

## Manual Configuration

Claude Code reads MCP server configuration from `~/.claude/claude_mcp_config.json` (global) or `.claude/claude_mcp_config.json` (project-local).

Add memory-agent as an MCP server:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "memory-agent",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

If the binary is not on your PATH, use the full path in `command`:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "/home/user/.cargo/bin/memory-agent",
      "args": ["mcp"]
    }
  }
}
```

## Verification

Restart Claude Code after editing the config. In a new conversation, run:

```
memory_list
```

If the tool is available and returns `{ "memories": [], "total": 0 }`, the connection is working.

## Hooks

`memory-agent install claude` installs 10 hook scripts across 9 Claude Code hook events. The hooks run automatically — no manual invocation needed.

| Script | Event | What it does |
|-|-|-|
| `session-start.sh` | SessionStart | Loads recent memories into context at session start |
| `user-prompt.sh` | UserPromptSubmit | Injects relevant memories before each prompt |
| `post-edit.sh` | PostToolUse/Edit\|Write | Saves file-edit context to memory |
| `agent-review-gate.sh` | PostToolUse/Agent | Prompts reviewer dispatch after agent tasks (opt-in) |
| `pre-compact.sh` | PreCompact | Saves important context before compaction |
| `stop.sh` | Stop | Ends memory session on Claude stop |
| `task-completed.sh` | TaskCompleted | Records task completion |
| `subagent-stop.sh` | SubagentStop | Records subagent completion |
| `instructions-loaded.sh` | InstructionsLoaded | Injects static prompt text if configured |
| `session-end.sh` | SessionEnd | Ends and summarizes session |

### Configuring hooks

Hook behavior is controlled via `~/.memory-agent/config.toml` under `[hooks]`:

```toml
[hooks]
# Static text to inject into Claude's context on every InstructionsLoaded event.
# Useful for team-wide conventions or personal reminders.
injection_prompt = "Always check memory_context before starting work."

# Enable the agent-review-gate hook (disabled by default).
# When enabled, Claude is prompted to dispatch a reviewer after agent subtasks.
agent_review_gate = true
```

The `injection_prompt` field is the primary customization point. Leave it empty (or omit it) to disable static injection.

The `agent-review-gate` hook is opt-in because it adds a review step after every agent subtask, which slows down execution. Enable it when you want automated quality gates on agent work.

## Recommended Workflow

With hooks installed, session lifecycle is fully automated — no manual MCP calls needed for session start/end or context loading. Claude's only job is to save knowledge when it discovers something worth remembering:

```
memory_save key="arch/pattern" value="..." scope="/my-project"
```

Save things like:
- Architecture decisions and patterns
- Recurring bugs and their fixes
- Project conventions and file locations
- Commands and configurations

The hooks handle the rest: `session-start.sh` calls `memory_session_start` and loads context at session start, `user-prompt.sh` injects relevant memories before each prompt, and `session-end.sh` ends the session automatically.

## Data Location

Memories are stored at `~/.memory-agent/memory.db` by default. To use a custom location:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "memory-agent",
      "args": ["mcp"],
      "env": {
        "MEMORY_AGENT_DATA_DIR": "/path/to/your/data"
      }
    }
  }
}
```

## Troubleshooting

**Tool not found after config change:** Restart Claude Code completely (not just the conversation).

**Permission denied on binary:** Run `chmod +x /path/to/memory-agent`.

**Database locked:** Only one `memory-agent mcp` process should run at a time. Check for stale processes with `ps aux | grep memory-agent`.

**View server logs:** Run `memory-agent mcp` directly in a terminal to see stderr output.
