# Memory Agent Setup: VS Code (Copilot Chat / Continue)

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

### GitHub Copilot Chat

Copilot Chat in VS Code supports MCP servers via VS Code settings. Open `settings.json` (**Cmd+Shift+P** → "Open User Settings (JSON)") and add:

```json
{
  "github.copilot.chat.mcpServers": {
    "memory-agent": {
      "command": "memory-agent",
      "args": ["mcp"]
    }
  }
}
```

For a project-local config, add the same block to `.vscode/settings.json` in the project root.

### Continue Extension

If you use the [Continue](https://continue.dev) extension, add the server to your Continue config at `~/.continue/config.json`:

```json
{
  "mcpServers": [
    {
      "name": "memory",
      "command": "memory-agent",
      "args": ["mcp"],
      "transport": "stdio"
    }
  ]
}
```

### Full Path (if binary not on PATH)

```json
{
  "github.copilot.chat.mcpServers": {
    "memory-agent": {
      "command": "/home/user/.cargo/bin/memory-agent",
      "args": ["mcp"]
    }
  }
}
```

## Verification

After saving the config, reload the VS Code window (**Cmd+Shift+P** → "Developer: Reload Window"). Open Copilot Chat and type:

```
Call memory_list
```

A response of `{ "memories": [], "total": 0 }` confirms the connection is working.

## Recommended Workflow

Add these instructions to your Copilot Chat instructions file (`.github/copilot-instructions.md`) or Continue system prompt:

```
Memory management:
- Start each session: memory_session_start with the project name
- Load context: memory_context for the current project scope
- Save decisions: memory_save after architecture decisions, discovered patterns, or important commands
- Search before solving: memory_search before tackling a problem you may have encountered before
- End each session: memory_session_end with a summary
```

## Data Location

Memories are stored at `~/.memory-agent/memory.db` by default. To use a different location:

```json
{
  "github.copilot.chat.mcpServers": {
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

**Tools not appearing:** Reload the VS Code window after editing settings. Use **Developer: Reload Window** rather than just restarting the chat panel.

**Copilot Chat version:** MCP support requires a recent version of the Copilot Chat extension. Update the extension if tools are not available.

**Continue not finding server:** Ensure the `config.json` is valid JSON. Check the Continue output panel (**View > Output**, select "Continue") for error messages.

**Permission denied on binary:** Run `chmod +x /path/to/memory-agent`.

**View server logs:** Run `memory-agent mcp 2>memory.log` in a terminal and inspect `memory.log` while reproducing the issue.
