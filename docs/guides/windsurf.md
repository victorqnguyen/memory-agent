# Memory Agent Setup: Windsurf

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

## Configuration

Windsurf supports MCP servers via a JSON config file. Create or edit `.windsurf/mcp.json` in your project root for a project-local config, or the equivalent global settings file in the Windsurf config directory.

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

Alternatively, open **Windsurf Settings** and navigate to the MCP section to add the server through the UI with:

| Field | Value |
|-|-|
| Name | `memory` |
| Command | `memory-agent` |
| Arguments | `mcp` |
| Transport | `stdio` |

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

After saving the config, reload Windsurf or restart it fully. Open the Cascade chat and type:

```
Call memory_list
```

A response of `{ "memories": [], "total": 0 }` confirms the connection is working.

## Recommended Workflow

Add these instructions to your Windsurf Rules or system prompt:

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

**Server not connecting:** Check that `memory-agent` is on your PATH. Test by running `memory-agent mcp` in a terminal — it should hang waiting for input (that is correct behavior).

**Tools not appearing after config change:** Windsurf may need a full restart after adding a new MCP server.

**Permission denied on binary:** Run `chmod +x /path/to/memory-agent`.

**View server logs:** Run `memory-agent mcp 2>memory.log` in a terminal and inspect `memory.log` while reproducing the issue.
