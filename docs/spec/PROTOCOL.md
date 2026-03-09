# Agent Memory Protocol v1.0

## Abstract

The Agent Memory Protocol (AMP) is an open protocol for persistent, structured memory in AI coding agents. It is built on the Model Context Protocol (MCP) and defines a standard set of tools, data types, and behavioral contracts that any agent framework can implement or consume.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHOULD", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

---

## Conformance Levels

Implementations are classified into three levels based on which tools they expose.

### Level 0 — Required

An implementation claiming AMP conformance MUST implement all Level 0 tools:

| Tool | Purpose |
|-|-|
| `memory_save` | Persist a key/value memory |
| `memory_search` | Full-text search across memories |
| `memory_detail` | Fetch a single memory by ID |
| `memory_delete` | Soft- or hard-delete a memory |
| `memory_list` | List memories with optional filters |
| `memory_context` | Return scope-relevant memories for injection |
| `memory_session_start` | Open a named session |
| `memory_session_end` | Close a session with summary |

### Level 1 — Optional

Implementations MAY implement Level 1 tools for richer knowledge management:

| Tool | Purpose |
|-|-|
| `memory_extract` | Extract memories from project config files |
| `memory_stale` | Detect memories whose source files have changed |
| `memory_relate` | Create a typed relationship between two memories |
| `memory_relations` | List all relationships for a memory |

### Level 2 — Optional

Implementations MAY implement Level 2 tools for advanced features:

| Tool | Purpose |
|-|-|
| `memory_metrics` | Return usage statistics |
| `memory_consolidate` | Detect and merge near-duplicate memories |
| `memory_budget` | Return top memories within a token budget |

Level 0 tools are frozen after v1.0. Breaking changes to Level 0 tools require a v2.0 bump.

---

## Transport

Implementations MUST support MCP stdio transport. MCP HTTP/SSE transport is OPTIONAL.

The server MUST be invocable as:

```
memory-agent mcp
```

The binary MUST read MCP JSON-RPC messages from stdin and write responses to stdout. All logs and diagnostics MUST go to stderr, never stdout.

---

## Data Model

### Memory

A memory is the atomic unit of stored knowledge.

| Field | Type | Description |
|-|-|-|
| `id` | integer | Unique row identifier, assigned by server |
| `key` | string | Semantic identifier, max 256 chars (e.g., `architecture/auth-model`) |
| `value` | string | The knowledge content, max 2000 chars |
| `scope` | string | Scope path, default `/` |
| `source_type` | string | One of `explicit`, `codebase`, `observed`, `derived` |
| `source_ref` | string? | Optional reference to origin (e.g., `src/auth.rs:12-34`) |
| `confidence` | float | 0.0–1.0, CHECK constraint enforced |
| `tags` | string[]? | Searchable labels |
| `revision_count` | integer | Number of times the value was updated |
| `duplicate_count` | integer | Number of deduplicated writes with identical content |
| `created_at` | string | ISO 8601 timestamp |
| `accessed_at` | string | ISO 8601 timestamp, updated on read |

### Session

A session tracks a bounded agent interaction.

| Field | Type | Description |
|-|-|-|
| `session_id` | string | UUID assigned by server |
| `project` | string | Project name |
| `status` | string | `active` or `ended` |
| `started_at` | string | ISO 8601 timestamp |
| `ended_at` | string? | ISO 8601 timestamp, set on session end |
| `summary` | string? | Agent-provided summary of the session |

### SearchResult

A trimmed memory representation returned by search and list tools.

| Field | Type | Description |
|-|-|-|
| `id` | integer | Memory ID |
| `key` | string | Semantic key |
| `value_preview` | string | Truncated value (first ~150 chars) |
| `scope` | string | Scope path |
| `source_type` | string | Source type |
| `confidence` | float | Confidence score |
| `rank` | float | FTS5 relevance rank (search only) |

---

## Tool Specifications

### memory_save

Persist a key/value memory. If a memory with the same key and scope already exists, the value is updated and `revision_count` is incremented. If the value is identical to the current value (within the dedup window), `duplicate_count` is incremented instead and no new row is created.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `key` | string | REQUIRED | Semantic key, max 256 chars |
| `value` | string | REQUIRED | Content, max 2000 chars |
| `scope` | string | OPTIONAL | Scope path, default `/` |
| `source_type` | string | OPTIONAL | `explicit`\|`codebase`\|`observed`\|`derived`, default `explicit` |
| `source_ref` | string | OPTIONAL | File reference for `codebase` type (e.g., `src/main.rs:1-10`) |
| `tags` | string[] | OPTIONAL | Searchable labels |

**Output**

| Field | Type | Description |
|-|-|-|
| `id` | integer | Memory ID |
| `action` | string | `created`, `updated`, or `duplicate` |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32602 | invalid params: key is required | Empty key |
| -32602 | invalid params: value is required | Empty value |
| -32602 | invalid params: key exceeds N chars | Key > 256 chars |
| -32602 | invalid params: value exceeds N chars | Value > 2000 chars |
| -32602 | invalid params: scope ... | Invalid scope (contains `..`, null bytes, etc.) |
| -32602 | invalid params: max N tags | Too many tags |
| -32602 | invalid params: tag exceeds N chars | Tag too long |
| -32602 | invalid params: unknown source_type "..." | Invalid source_type value |
| -32000 | server error: database error | Internal database failure |

---

### memory_search

Full-text search across memories using FTS5. Results are returned ranked by relevance. The implementation MUST sanitize query input before passing to FTS5.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `query` | string | REQUIRED | Search terms |
| `scope` | string | OPTIONAL | Filter to this scope (exact match) |
| `source_type` | string | OPTIONAL | Filter to source type |
| `limit` | integer | OPTIONAL | Max results, default 10, max 50 |

**Output**

| Field | Type | Description |
|-|-|-|
| `results` | SearchResult[] | Ranked results |
| `total` | integer | Count of results returned |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_detail

Retrieve a single memory by ID. Accessing a memory SHOULD update its `accessed_at` timestamp.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `id` | integer | REQUIRED | Memory ID from search results |

**Output**

Full Memory object (see Data Model).

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32001 | memory not found | ID does not exist or is soft-deleted |
| -32000 | server error: database error | Internal failure |

---

### memory_delete

Delete a memory by key and scope. Default is soft delete (sets `deleted_at`, excluded from search). Hard delete (`hard: true`) removes the row permanently.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `key` | string | REQUIRED | Memory key to delete |
| `scope` | string | OPTIONAL | Scope, default `/` |
| `hard` | boolean | OPTIONAL | Hard delete, default `false` |

**Output**

| Field | Type | Description |
|-|-|-|
| `deleted` | boolean | `true` if deleted, `false` if not found |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_list

List memories with optional filters. Returns abbreviated memory records (no full value).

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `scope` | string | OPTIONAL | Filter to scope |
| `source_type` | string | OPTIONAL | Filter to source type |
| `limit` | integer | OPTIONAL | Max results, default 20 |

**Output**

| Field | Type | Description |
|-|-|-|
| `memories` | ListItem[] | Abbreviated memory records |
| `total` | integer | Count of results returned |

Each `ListItem` has: `id`, `key`, `value_preview`, `scope`, `source_type`, `confidence`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_context

Return the most relevant memories for the current scope. Intended for context injection at session start. The implementation SHOULD traverse the scope chain (see Scope Resolution) and return memories from the requested scope and all parent scopes.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `scope` | string | OPTIONAL | Target scope, default `/` |
| `limit` | integer | OPTIONAL | Max results, default 10 |

**Output**

Same as `memory_list` output: `{ memories: ListItem[], total: integer }`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_session_start

Open a new session. Sessions are used to group memories by agent interaction for audit and recall. The implementation MUST return a unique `session_id` for subsequent calls.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `project` | string | REQUIRED | Project name |
| `directory` | string | OPTIONAL | Working directory |

**Output**

Session object (see Data Model). `status` MUST be `active`. `ended_at` MUST be `null`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_session_end

Close an active session. A session that has already been ended MUST return error -32002.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `session_id` | string | REQUIRED | Session ID from `memory_session_start` |
| `summary` | string | OPTIONAL | What was accomplished this session |

**Output**

Session object with `status: "ended"` and `ended_at` set.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32001 | session not found | Session ID does not exist |
| -32002 | session already ended | Session was previously ended |
| -32000 | server error: database error | Internal failure |

---

### memory_extract

Scan project configuration files and extract memories automatically. Currently supports `source: "config"` which reads well-known config files (`package.json`, `Cargo.toml`, `pyproject.toml`, etc.) and saves key/value pairs with `source_type: codebase`.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `source` | string | REQUIRED | Extraction strategy, currently `"config"` |
| `directory` | string | REQUIRED | Project root directory to scan |
| `scope` | string | OPTIONAL | Override auto-detected scope |

**Output**

| Field | Type | Description |
|-|-|-|
| `extracted` | integer | New memories created |
| `updated` | integer | Existing memories updated |
| `skipped` | integer | Files skipped (not found or unsupported) |
| `files_scanned` | string[] | Paths of files that were read |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32602 | invalid params: ... | Invalid source or directory |
| -32000 | server error: ... | Internal failure |

---

### memory_stale

Detect memories whose backing source files have changed since they were saved. Uses git to identify modified files and cross-references against `source_ref` on stored memories.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `scope` | string | OPTIONAL | Filter to scope |
| `directory` | string | OPTIONAL | Working directory for git discovery |

**Output**

| Field | Type | Description |
|-|-|-|
| `stale` | StaleItem[] | Stale memories |
| `checked` | integer | Number of memories checked |

Each `StaleItem` has: `memory_id`, `key`, `reason`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: ... | Git unavailable or internal failure |

---

### memory_relate

Create a typed directional relationship between two memories.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `source_id` | integer | REQUIRED | Source memory ID |
| `target_id` | integer | REQUIRED | Target memory ID |
| `relation` | string | REQUIRED | `derived_from`\|`supersedes`\|`conflicts_with`\|`related_to` |

**Output**

| Field | Type | Description |
|-|-|-|
| `id` | integer | Relation record ID |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32001 | memory not found | Source or target ID does not exist |
| -32602 | invalid params: ... | Unknown relation type |
| -32000 | server error: database error | Internal failure |

---

### memory_relations

List all relationships for a given memory (both directions).

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `id` | integer | REQUIRED | Memory ID |

**Output**

| Field | Type | Description |
|-|-|-|
| `relations` | RelationItem[] | All relations involving this memory |

Each `RelationItem` has: `id`, `source_id`, `target_id`, `relation_type`, `created_at`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32001 | memory not found | ID does not exist |
| -32000 | server error: database error | Internal failure |

---

### memory_metrics

Return aggregate usage statistics across all memories.

**Input**

No parameters.

**Output**

| Field | Type | Description |
|-|-|-|
| `aggregate_hit_rate` | float | Fraction of injected memories that were referenced |
| `total_injections` | integer | Total times memories were injected |
| `total_hits` | integer | Total times injected memories were used |
| `top_memories` | MetricsItem[] | Most-used memories |

Each `MetricsItem` has: `id`, `key`, `scope`, `injections`, `hits`, `hit_rate`.

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32000 | server error: database error | Internal failure |

---

### memory_consolidate

Detect and optionally merge near-duplicate memories within a scope. By default runs as a dry run (preview only).

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `scope` | string | OPTIONAL | Filter to scope |
| `dry_run` | boolean | OPTIONAL | Preview without merging, default `true` |
| `threshold` | float | OPTIONAL | Similarity threshold 0.0–1.0, default `0.85` |

**Output**

| Field | Type | Description |
|-|-|-|
| `groups` | ConsolidationGroupItem[] | Groups of similar memories |
| `consolidated` | integer | Number of merges performed (0 when dry_run) |

Each `ConsolidationGroupItem` has: `key`, `memory_ids` (array of integers), `similarity` (float).

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32602 | invalid params: ... | Threshold out of range |
| -32000 | server error: database error | Internal failure |

---

### memory_budget

Return the highest-value memories that fit within a token budget. Useful for context injection when the agent has a limited token allowance.

**Input**

| Field | Type | Required | Description |
|-|-|-|-|
| `max_tokens` | integer | REQUIRED | Token budget |
| `scope` | string | OPTIONAL | Filter to scope |

**Output**

| Field | Type | Description |
|-|-|-|
| `memories` | ListItem[] | Selected memories |
| `tokens_used` | integer | Tokens consumed by selected memories |
| `tokens_remaining` | integer | Tokens left from budget |

**Errors**

| Code | Message | Condition |
|-|-|-|
| -32602 | invalid params: ... | Invalid max_tokens |
| -32000 | server error: database error | Internal failure |

---

## Scope Resolution Algorithm

Scope paths are Unix-style hierarchical strings (e.g., `/projects/myapp/frontend`).

**Normalization (applied on every write and read):**

1. Strip trailing `/`
2. Ensure leading `/`
3. Empty string becomes `/`
4. Reject paths containing `..` or null bytes with error -32602

**Traversal for `memory_context`:**

Given scope `/projects/myapp/frontend`, the implementation MUST include memories from:

1. `/projects/myapp/frontend` (exact scope)
2. `/projects/myapp` (parent)
3. `/projects` (grandparent)
4. `/` (root)

Results are merged and deduplicated by ID, ordered by specificity (most specific scope first), then by relevance within each scope tier.

---

## Source Types

| Value | Meaning |
|-|-|
| `explicit` | Saved directly by the agent or user. Highest trust. |
| `codebase` | Extracted from project files (config, manifests, etc.). |
| `observed` | Inferred by the agent from runtime behavior or commands. |
| `derived` | Computed from other memories (e.g., via consolidation). |

Source type affects dedup behavior: `codebase` memories with the same `source_ref` are compared against their origin file on staleness checks.

---

## Privacy Requirements

Implementations MUST apply the following before writing any memory value to storage:

1. **Private tag stripping:** Content matching `#private` or similar user-configured tags MUST be stripped or the save MUST be rejected.
2. **Secret detection:** The implementation MUST scan values for common credential patterns (API keys, tokens, passwords in assignment form). Matching content MUST be redacted or rejected before storage. This protection applies even if the user explicitly supplies a value containing a credential pattern.
3. **Log safety:** Implementations MUST NOT log memory values. Only IDs and keys MAY appear in logs.

---

## Error Codes

| Code | Name | Meaning |
|-|-|-|
| -32602 | invalid params | Validation failure (bad key, value, scope, type) |
| -32001 | not found | Memory or session ID does not exist |
| -32002 | already ended | Session was previously ended |
| -32003 | schema too new | Database requires a newer binary version |
| -32000 | server error | Internal error (database failure, unexpected state) |

Implementations MUST NOT expose internal details (SQL errors, file paths) in error messages. Internal context SHOULD be logged server-side only.

---

## Protocol Versioning

| Change Type | Version Bump | Example |
|-|-|-|
| Breaking change to any tool schema or behavior | Major (v2.0) | Rename a required field |
| New optional tool or optional field | Minor (v1.1) | Add `memory_observe` |
| Clarification, typo fix, test fixture update | Patch (v1.0.1) | Fix example JSON |

- Level 0 tools are frozen after v1.0. New tools are always Level 1 or 2 until a major version.
- Deprecation: a tool or field is deprecated in one minor version and removed in the next major version.
- Implementations SHOULD advertise their supported protocol version in the MCP server info block.
- The binary version (`0.x.y`), protocol version (`1.0`), and schema version (integer) are independent and MUST be reported separately by `memory-agent version`.
