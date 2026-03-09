# Memory Agent Setup: Cursor

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
memory-agent install cursor
```

This creates/updates `~/.cursor/mcp.json` with the MCP config and walks you through optional LLM and encryption setup.

To uninstall: `memory-agent uninstall cursor`

## Manual Configuration

Cursor supports MCP servers via its settings. Open **Cursor Settings** (Cmd+, on macOS / Ctrl+, on Linux/Windows), then navigate to **Features > MCP**.

Click **Add MCP Server** and fill in:

| Field | Value |
|-|-|
| Name | `memory` |
| Type | `stdio` |
| Command | `memory-agent` |
| Arguments | `mcp` |

Or edit `~/.cursor/mcp.json` directly:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "memory-agent",
      "args": ["mcp"]
    }
  }
}
```

For a project-local config, create `.cursor/mcp.json` in the project root with the same structure.

If the binary is not on your PATH, use the full path:

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

After saving the config, click the refresh icon in the MCP panel or restart Cursor. The `memory` server should appear with status **Connected**.

Open the Cursor chat and type:

```
Call memory_list
```

A response of `{ "memories": [], "total": 0 }` confirms the connection is working.

## Recommended Workflow

Add these instructions to your Cursor Rules (`.cursorrules` or **Cursor Settings > Rules for AI**):

```
Memory management:
- Start each session: memory_session_start with the project name
- Load context: memory_context for the current project scope
- Save decisions: memory_save after architecture decisions, discovered patterns, or important commands
- Search before solving: memory_search before tackling a problem you may have encountered before
- End each session: memory_session_end with a summary
```

## Data Location

Memories are stored at `~/.memory-agent/memory.db` by default. To use a different location, set the environment variable in the MCP config:

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

**Server shows disconnected:** Check that `memory-agent` is on your PATH. Test by running `memory-agent mcp` in a terminal — it should hang waiting for input (that is correct behavior).

**No tools appearing:** Cursor may need a full restart after adding a new MCP server.

**Multiple Cursor windows:** Each window spawns its own `memory-agent mcp` process. This is safe — SQLite handles concurrent readers, and writes are serialized via the WAL journal.

**View server logs:** Run `memory-agent mcp 2>memory.log` in a terminal and inspect `memory.log` while reproducing the issue.
