# Agent Memory Protocol — Request/Response Examples

All examples use MCP JSON-RPC 2.0 format. The `method` is always `tools/call`. Each example shows the `params` object and the expected `result` content.

---

## memory_save

### Create a new memory

Request:
```json
{
  "tool": "memory_save",
  "params": {
    "key": "commands/test",
    "value": "bun test",
    "scope": "/projects/myapp",
    "source_type": "explicit",
    "tags": ["commands", "testing"]
  }
}
```

Response:
```json
{
  "id": 1,
  "action": "created"
}
```

### Update an existing memory (same key, different value)

Request:
```json
{
  "tool": "memory_save",
  "params": {
    "key": "commands/test",
    "value": "bun test --watch",
    "scope": "/projects/myapp"
  }
}
```

Response:
```json
{
  "id": 1,
  "action": "updated"
}
```

### Duplicate write (same key, same value — dedup)

Request:
```json
{
  "tool": "memory_save",
  "params": {
    "key": "commands/test",
    "value": "bun test --watch",
    "scope": "/projects/myapp"
  }
}
```

Response:
```json
{
  "id": 1,
  "action": "duplicate"
}
```

### Save a codebase memory with source reference

Request:
```json
{
  "tool": "memory_save",
  "params": {
    "key": "architecture/auth-model",
    "value": "JWT tokens, 15min expiry, refresh stored in httpOnly cookie",
    "scope": "/projects/myapp",
    "source_type": "codebase",
    "source_ref": "src/auth/middleware.rs:1-45"
  }
}
```

Response:
```json
{
  "id": 2,
  "action": "created"
}
```

---

## memory_search

### Basic search

Request:
```json
{
  "tool": "memory_search",
  "params": {
    "query": "test commands"
  }
}
```

Response:
```json
{
  "results": [
    {
      "id": 1,
      "key": "commands/test",
      "value_preview": "bun test --watch",
      "scope": "/projects/myapp",
      "source_type": "explicit",
      "confidence": 1.0,
      "rank": -0.832
    }
  ],
  "total": 1
}
```

### Search with scope filter

Request:
```json
{
  "tool": "memory_search",
  "params": {
    "query": "auth JWT",
    "scope": "/projects/myapp",
    "limit": 5
  }
}
```

Response:
```json
{
  "results": [
    {
      "id": 2,
      "key": "architecture/auth-model",
      "value_preview": "JWT tokens, 15min expiry, refresh stored in httpOnly cookie",
      "scope": "/projects/myapp",
      "source_type": "codebase",
      "confidence": 1.0,
      "rank": -1.241
    }
  ],
  "total": 1
}
```

---

## memory_detail

Request:
```json
{
  "tool": "memory_detail",
  "params": {
    "id": 2
  }
}
```

Response:
```json
{
  "id": 2,
  "key": "architecture/auth-model",
  "value": "JWT tokens, 15min expiry, refresh stored in httpOnly cookie",
  "scope": "/projects/myapp",
  "source_type": "codebase",
  "source_ref": "src/auth/middleware.rs:1-45",
  "confidence": 1.0,
  "tags": null,
  "revision_count": 0,
  "duplicate_count": 0,
  "created_at": "2026-03-05T10:00:00Z",
  "accessed_at": "2026-03-05T10:05:00Z"
}
```

### Not found error

Response:
```json
{
  "error": {
    "code": -32001,
    "message": "memory not found"
  }
}
```

---

## memory_delete

### Soft delete (default)

Request:
```json
{
  "tool": "memory_delete",
  "params": {
    "key": "commands/test",
    "scope": "/projects/myapp"
  }
}
```

Response:
```json
{
  "deleted": true
}
```

### Hard delete

Request:
```json
{
  "tool": "memory_delete",
  "params": {
    "key": "architecture/auth-model",
    "scope": "/projects/myapp",
    "hard": true
  }
}
```

Response:
```json
{
  "deleted": true
}
```

### Key not found

Response:
```json
{
  "deleted": false
}
```

---

## memory_list

Request:
```json
{
  "tool": "memory_list",
  "params": {
    "scope": "/projects/myapp",
    "limit": 20
  }
}
```

Response:
```json
{
  "memories": [
    {
      "id": 1,
      "key": "commands/test",
      "value_preview": "bun test --watch",
      "scope": "/projects/myapp",
      "source_type": "explicit",
      "confidence": 1.0
    },
    {
      "id": 2,
      "key": "architecture/auth-model",
      "value_preview": "JWT tokens, 15min expiry, refresh stored in httpOnly cookie",
      "scope": "/projects/myapp",
      "source_type": "codebase",
      "confidence": 1.0
    }
  ],
  "total": 2
}
```

---

## memory_context

Request:
```json
{
  "tool": "memory_context",
  "params": {
    "scope": "/projects/myapp/frontend",
    "limit": 10
  }
}
```

Response (includes memories from `/projects/myapp/frontend`, `/projects/myapp`, `/projects`, and `/`):
```json
{
  "memories": [
    {
      "id": 5,
      "key": "ui/component-library",
      "value_preview": "shadcn/ui with Tailwind CSS v4",
      "scope": "/projects/myapp/frontend",
      "source_type": "explicit",
      "confidence": 1.0
    },
    {
      "id": 1,
      "key": "commands/test",
      "value_preview": "bun test --watch",
      "scope": "/projects/myapp",
      "source_type": "explicit",
      "confidence": 1.0
    }
  ],
  "total": 2
}
```

---

## memory_session_start

Request:
```json
{
  "tool": "memory_session_start",
  "params": {
    "project": "myapp",
    "directory": "/home/user/projects/myapp"
  }
}
```

Response:
```json
{
  "session_id": "01948b2c-f3a1-7d8e-a4c2-9e1b3f2d6a7c",
  "project": "myapp",
  "status": "active",
  "started_at": "2026-03-05T10:00:00Z",
  "ended_at": null,
  "summary": null
}
```

---

## memory_session_end

Request:
```json
{
  "tool": "memory_session_end",
  "params": {
    "session_id": "01948b2c-f3a1-7d8e-a4c2-9e1b3f2d6a7c",
    "summary": "Refactored auth middleware to use refresh token rotation. Updated JWT expiry from 1h to 15min."
  }
}
```

Response:
```json
{
  "session_id": "01948b2c-f3a1-7d8e-a4c2-9e1b3f2d6a7c",
  "project": "myapp",
  "status": "ended",
  "started_at": "2026-03-05T10:00:00Z",
  "ended_at": "2026-03-05T11:30:00Z",
  "summary": "Refactored auth middleware to use refresh token rotation. Updated JWT expiry from 1h to 15min."
}
```

### Session already ended error

Response:
```json
{
  "error": {
    "code": -32002,
    "message": "session already ended"
  }
}
```

---

## memory_extract

Request:
```json
{
  "tool": "memory_extract",
  "params": {
    "source": "config",
    "directory": "/home/user/projects/myapp"
  }
}
```

Response:
```json
{
  "extracted": 4,
  "updated": 1,
  "skipped": 2,
  "files_scanned": [
    "/home/user/projects/myapp/package.json",
    "/home/user/projects/myapp/Cargo.toml"
  ]
}
```

---

## memory_stale

Request:
```json
{
  "tool": "memory_stale",
  "params": {
    "scope": "/projects/myapp",
    "directory": "/home/user/projects/myapp"
  }
}
```

Response:
```json
{
  "stale": [
    {
      "memory_id": 2,
      "key": "architecture/auth-model",
      "reason": "source file src/auth/middleware.rs has uncommitted changes"
    }
  ],
  "checked": 12
}
```

---

## memory_relate

Request:
```json
{
  "tool": "memory_relate",
  "params": {
    "source_id": 3,
    "target_id": 2,
    "relation": "supersedes"
  }
}
```

Response:
```json
{
  "id": 1
}
```

---

## memory_relations

Request:
```json
{
  "tool": "memory_relations",
  "params": {
    "id": 2
  }
}
```

Response:
```json
{
  "relations": [
    {
      "id": 1,
      "source_id": 3,
      "target_id": 2,
      "relation_type": "supersedes",
      "created_at": "2026-03-05T10:15:00Z"
    }
  ]
}
```

---

## memory_metrics

Request:
```json
{
  "tool": "memory_metrics",
  "params": {}
}
```

Response:
```json
{
  "aggregate_hit_rate": 0.73,
  "total_injections": 142,
  "total_hits": 104,
  "top_memories": [
    {
      "id": 1,
      "key": "commands/test",
      "scope": "/projects/myapp",
      "injections": 38,
      "hits": 35,
      "hit_rate": 0.92
    },
    {
      "id": 2,
      "key": "architecture/auth-model",
      "scope": "/projects/myapp",
      "injections": 24,
      "hits": 18,
      "hit_rate": 0.75
    }
  ]
}
```

---

## memory_consolidate

### Dry run (preview duplicates)

Request:
```json
{
  "tool": "memory_consolidate",
  "params": {
    "scope": "/projects/myapp",
    "dry_run": true,
    "threshold": 0.85
  }
}
```

Response:
```json
{
  "groups": [
    {
      "key": "architecture/auth",
      "memory_ids": [7, 9],
      "similarity": 0.91
    }
  ],
  "consolidated": 0
}
```

### Live run (merge duplicates)

Request:
```json
{
  "tool": "memory_consolidate",
  "params": {
    "scope": "/projects/myapp",
    "dry_run": false,
    "threshold": 0.85
  }
}
```

Response:
```json
{
  "groups": [
    {
      "key": "architecture/auth",
      "memory_ids": [7, 9],
      "similarity": 0.91
    }
  ],
  "consolidated": 1
}
```

---

## memory_budget

Request:
```json
{
  "tool": "memory_budget",
  "params": {
    "max_tokens": 2000,
    "scope": "/projects/myapp"
  }
}
```

Response:
```json
{
  "memories": [
    {
      "id": 1,
      "key": "commands/test",
      "value_preview": "bun test --watch",
      "scope": "/projects/myapp",
      "source_type": "explicit",
      "confidence": 1.0
    },
    {
      "id": 2,
      "key": "architecture/auth-model",
      "value_preview": "JWT tokens, 15min expiry, refresh stored in httpOnly cookie",
      "scope": "/projects/myapp",
      "source_type": "codebase",
      "confidence": 1.0
    }
  ],
  "tokens_used": 312,
  "tokens_remaining": 1688
}
```
