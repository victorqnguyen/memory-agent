# Memory Agent Setup: Gemini CLI

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
memory-agent install gemini
```

This creates/updates `~/.gemini/settings.json` with the MCP config and walks you through optional LLM and encryption setup.

To uninstall: `memory-agent uninstall gemini`

## Manual Configuration

If you prefer manual setup, Gemini CLI reads MCP server configuration from `.gemini/settings.json` in the project root (project-local) or `~/.gemini/settings.json` (global).

Create or edit the file:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "memory-agent",
      "args": ["mcp"],
      "transport": "stdio"
    }
  }
}
```

If the binary is not on your PATH, use the full path:

```json
{
  "mcpServers": {
    "memory-agent": {
      "command": "/home/user/.cargo/bin/memory-agent",
      "args": ["mcp"],
      "transport": "stdio"
    }
  }
}
```

## Verification

After saving the config, start a new Gemini CLI session. Run:

```
memory_list
```

A response of `{ "memories": [], "total": 0 }` confirms the connection is working.

## Recommended Workflow

Add these instructions to your Gemini CLI system prompt or GEMINI.md project file:

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
      "transport": "stdio",
      "env": {
        "MEMORY_AGENT_DATA_DIR": "/path/to/your/data"
      }
    }
  }
}
```

## Troubleshooting

**Tool not found:** Verify the `settings.json` is valid JSON and that `memory-agent` is on your PATH. Run `which memory-agent` to confirm.

**Session not picking up config:** Gemini CLI reads config at startup — restart the CLI after editing `settings.json`.

**Permission denied on binary:** Run `chmod +x /path/to/memory-agent`.

**View server logs:** Run `memory-agent mcp 2>memory.log` in a terminal and inspect `memory.log` while reproducing the issue.
