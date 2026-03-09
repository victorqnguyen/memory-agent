# Building a Custom Integration

This guide explains how to integrate memory-agent into any MCP-compatible client, or build a direct integration from scratch using the stdio transport.

## Prerequisites

Install the binary:

```bash
cargo install memory-agent
```

Verify it works:

```bash
memory-agent version
```

## How the Transport Works

`memory-agent mcp` implements the [Model Context Protocol](https://modelcontextprotocol.io) over stdio. The protocol is JSON-RPC 2.0:

- The client writes a JSON object to **stdin**, terminated by a newline.
- The server writes a JSON object to **stdout**, terminated by a newline.
- Errors and logs go to **stderr** (never stdout).
- The process runs until stdin is closed.

Start the server:

```bash
memory-agent mcp
```

The process will hang waiting for input. This is correct.

## Protocol Handshake

Send an `initialize` request first:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","capabilities":{},"clientInfo":{"name":"my-client","version":"0.1.0"}}}
```

The server responds with its capabilities, then send the `notifications/initialized` notification:

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

After this the server is ready to handle tool calls.

## Calling a Tool

Use `tools/call` with the tool name and parameters:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_save","arguments":{"key":"commands/test","value":"bun test"}}}
```

Response:

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"id\":1,\"action\":\"created\"}"}]}}
```

## Available Tools

| Tool | Description |
|-|-|
| `memory_save` | Save or update a memory by key |
| `memory_get` | Retrieve a memory by key |
| `memory_search` | Full-text search across all memories |
| `memory_delete` | Soft-delete a memory by key |
| `memory_list` | List memories, optionally filtered by scope |
| `memory_context` | Load all memories relevant to a scope |
| `memory_session_start` | Start a session and record it |
| `memory_session_end` | End a session with a summary |

### memory_save

```json
{
  "key": "path/to/memory",
  "value": "content to store",
  "scope": "/project/name",
  "confidence": 0.9,
  "source": "agent"
}
```

`key` and `value` are required. `scope`, `confidence`, and `source` are optional.

### memory_search

```json
{
  "query": "search terms",
  "scope": "/project/name",
  "limit": 10
}
```

Returns ranked results with snippet previews.

### memory_context

```json
{
  "scope": "/project/name",
  "limit": 50
}
```

Returns all memories relevant to the given scope, ordered by recency and confidence.

### memory_session_start / memory_session_end

```json
{ "project": "my-project" }
```

```json
{ "session_id": 1, "summary": "Implemented feature X, fixed bug Y" }
```

## Testing with Raw JSON

You can test the integration manually using a shell pipe:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_list","arguments":{}}}' | memory-agent mcp
```

Or interactively using `nc` as a line buffer:

```bash
memory-agent mcp
# paste JSON lines manually, one per line
```

## MCP Config for Any Client

Most MCP-compatible clients accept a configuration block of this form:

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

Consult your client's documentation for the exact config file location and field names. The `command`, `args`, and `transport` fields are standard across most implementations.

## Error Codes

| Code | Meaning |
|-|-|
| `-32602` | Invalid parameters (validation failure) |
| `-32001` | Not found (key does not exist) |
| `-32000` | Server error (internal failure) |

Error messages are safe to display to users. Internal details (SQL errors, file paths) are logged to stderr only.

## Data Location

By default, the database is at `~/.memory-agent/memory.db`. Override with:

```bash
MEMORY_AGENT_DATA_DIR=/path/to/dir memory-agent mcp
```

Or set `env` in the MCP server config block.

## Troubleshooting

**No response after sending JSON:** Ensure the JSON is on a single line and terminated with a newline (`\n`). The protocol is line-delimited.

**Initialize not acknowledged:** Send the `notifications/initialized` notification after receiving the `initialize` response. Some clients skip this and the server may not accept tool calls.

**Binary not found:** Use the full path to `memory-agent` in the `command` field. Run `which memory-agent` to find it.
