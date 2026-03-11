# Versioning and Deprecation Policy

## Binary Versioning

Follows semver `0.x.y` during pre-1.0 development.

- **Patch** (`0.x.Y`): bug fixes, non-breaking changes.
- **Minor** (`0.X.0`): may break CLI flags. Announce in CHANGELOG.md.
- **1.0.0**: first stable release. CLI flags are stable from this point.

## Protocol Versioning

Protocol version starts at `1.0` and is independent of the binary version.

- Tool parameters are **additive-only**. Adding an optional field is non-breaking.
- Removing a field, renaming a field, or changing a field's type is a **breaking change** requiring a protocol major bump.
- The protocol version is reported by `memory-agent version` and in the MCP `ServerInfo`.

## Schema Versioning

Schema version is a monotonically increasing integer.

- Each migration stage adds one `M::up()` entry via `rusqlite_migration`.
- Forward-only: no rollbacks. A binary supports all schemas from v1 to its current version.
- Schema version is reported by `memory-agent version`.

## MCP Tool Names

Tool names (e.g., `memory_save`, `memory_search`) are **locked**. Renaming a tool is a breaking change because agent configurations reference tool names directly. A rename requires a major version bump and a deprecation period — the old name must continue to function until the next major release.

## Deprecation Lifecycle for MCP Tools

When a tool needs to be superseded by a replacement:

1. **Mark deprecated in source.** Prepend the deprecation notice to the tool's `description` string in the `#[tool(...)]` attribute:

   ```rust
   #[tool(
       name = "memory_old_name",
       description = "DEPRECATED in v1.2: use memory_replacement instead. <original description>"
   )]
   ```

2. **Log a warning on every call.** Add `log_deprecated` as the first line of the handler body:

   ```rust
   async fn old_name(&self, params: Parameters<OldRequest>) -> Result<CallToolResult, McpError> {
       log_deprecated("memory_old_name", "memory_replacement");
       // ... rest of handler
   }
   ```

3. **Keep it functional.** Deprecated tools remain fully operational until the next major version bump (0.x → 1.0, or 1.x → 2.0).

4. **Announce.** Add an entry to `CHANGELOG.md` when deprecating a tool. Include the version it will be removed in.

5. **Remove.** At the major version boundary, delete the handler and its request/response types.

## Encryption

The release binary always includes SQLCipher (encryption compiled in by default). Encryption is enabled at **runtime** via `encryption_enabled = true` in `~/.memory-agent/config.toml` — it is not a compile-time flag.
