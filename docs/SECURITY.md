# Security Plan

Threat model and per-stage security controls for memory-agent. This document is a checklist — each stage implementation must verify its controls are in place before merging.

See also: [AD-16 in ARCHITECTURE-DECISIONS.md](ARCHITECTURE-DECISIONS.md#ad-16-security-controls) for the architectural decisions behind these controls.

---

## Threat Model

| ID | Threat | Vector | Stage | Severity | Control |
|-|-|-|-|-|-|
| T1 | SQL injection via FTS5 | Malicious search query with FTS5 operators | 1 | Critical | Whitelist sanitization in `search.rs` |
| T2 | Secrets persisted in memories | API keys, tokens, passwords stored as memory content | 1, 4, 6 | High | Secret pattern detection in `privacy.rs` |
| T3 | Path traversal via scope | `../` or null bytes in scope strings | 1 | High | Scope validation in `store/memory.rs` |
| T4 | Unauthorized daemon access | Unprotected Unix socket | 2 | High | Socket permissions + peer UID verification |
| T5 | DB file exposure | Database readable by other users | 2 | Medium | File permissions `0600` on creation |
| T6 | Log file secrets | Daemon logs memory content verbatim | 2 | Medium | Structured logging, value field redaction |
| T7 | Memory exhaustion | Extremely large MCP payloads | 2 | Low | Input size limits at MCP boundary |
| T8 | Dependency vulnerabilities | Known CVEs in crate dependencies | All | Medium | `cargo audit` in CI |
| T9 | .env file ingestion | Cold-start extracts secrets from .env files | 4 | High | Skip .env entirely, respect .gitignore |
| T10 | Stale PID file race | Two daemons start simultaneously | 2 | Low | Atomic PID file creation with advisory lock |

---

## Per-Stage Security Checklist

### Stage 1: Foundation

- [ ] **T1: FTS5 sanitization** — `search.rs::sanitize_fts_query()`
  - Whitelist: alphanumeric, spaces, hyphens, underscores, dots
  - Strip all FTS5 operators: `NEAR`, `AND`, `OR`, `NOT`, `*`, `^`, `column:`
  - Wrap each surviving term in double quotes (implicit AND)
  - Empty/whitespace-only input returns no results (not an error)
  - **Tests:** SQL injection string, FTS5 operator injection, column filter injection

- [ ] **T2: Secret detection** — `privacy.rs::strip_secrets()`
  - Default patterns compiled into binary (AD-18: `DEFAULT_SECRET_PATTERNS`)
  - Users add patterns via `[privacy] extra_patterns` in config.toml, or replace defaults entirely
  - AWS access keys: `AKIA[0-9A-Z]{16}`
  - Private keys: `-----BEGIN .* PRIVATE KEY-----`
  - Credential assignments: `api_key=...`, `token:...`, `password=...`
  - Connection strings: `mongodb://...`, `postgres://...`, `mysql://...`, `redis://...`
  - Platform tokens: GitHub `ghp_`, OpenAI `sk-`, Slack `xoxb-`
  - All replaced with `[SECRET_REDACTED]` before storage
  - Applied AFTER `<private>` tag stripping, BEFORE hash computation
  - **Tests:** Each pattern type detected and redacted, false positive rate acceptable, custom patterns work

- [ ] **T3: Input validation** — `store/memory.rs`
  - Key: max 256 chars, no null bytes, no control characters
  - Value: truncated to 2000 chars (with `...` suffix), null bytes stripped
  - Scope: no `..` sequences, no null bytes, normalized via `normalize_scope()`
  - Tags: max 20 tags, max 64 chars each, no null bytes
  - source_type: validated against enum variants
  - **Tests:** Each boundary condition, each invalid input pattern

- [ ] **T8: CI security** — workspace-level
  - `cargo audit` in CI pipeline
  - `cargo clippy -- -D warnings`
  - `Cargo.lock` committed to version control

### Stage 2: MCP Server + Daemon

- [ ] **T4: Socket security** — `daemon/ipc.rs`
  - Unix socket created with `0600` permissions
  - Verify peer UID on connection (`SO_PEERCRED` / `LOCAL_PEERCRED`)
  - Reject connections from different UIDs

- [ ] **T5: File permissions** — `daemon/mod.rs`
  - `memory.db`: `0600`
  - `daemon.sock`: `0600`
  - `daemon.log`: `0600`
  - `daemon.pid`: `0644`
  - Set permissions on creation, verify on open
  - **Tests:** Create files, verify permissions with stat

- [ ] **T6: Log sanitization** — `daemon/mod.rs`
  - Never log memory `value` fields
  - Log keys, IDs, scope, source_type only
  - Use tracing with structured fields; custom layer redacts `value`
  - **Tests:** Capture log output, verify no value content appears

- [ ] **T7: MCP input limits** — `mcp/tools.rs`
  - Key: max 256 chars (reject, don't truncate)
  - Value: max 10,000 chars (reject at MCP boundary; core truncates to 2000)
  - Query: max 500 chars
  - Tags: max 20, max 64 chars each
  - Limit parameter: max 50
  - Reject with MCP error, not silent truncation
  - **Tests:** Oversized inputs return proper MCP error codes

- [ ] **T10: PID file safety** — `daemon/mod.rs`
  - Atomic creation with `O_CREAT | O_EXCL`
  - Advisory file lock (`flock`) for race protection
  - Stale PID detection: check if process exists before claiming stale
  - **Tests:** Concurrent daemon start attempts

### Stage 4: Cold Start

- [ ] **T9: File content scanning** — `source/config.rs`
  - Skip `.env` files entirely (never read, never extract)
  - Respect `.gitignore` patterns (don't extract ignored files)
  - Run `strip_secrets()` on ALL extracted content
  - Configurable deny-list of file patterns in `config.toml`
  - Extract variable NAMES from `.env.example`, never values
  - **Tests:** .env with secrets not ingested, .gitignored files skipped

### Stage 5: Git Integration

- [ ] **File path validation** — `source/git.rs`
  - Canonicalize all paths before comparison
  - Reject symlinks pointing outside project directory
  - Validate source_ref paths exist within the repository

### Stage 6: Behavioral Observation

- [ ] **Observation filtering** — `source/observer.rs`
  - Run `strip_secrets()` on observed patterns before storage
  - Don't observe/store content from files matching `.gitignore`
  - Rate-limit observation storage (max N observations per minute)

---

## Security Testing

Each stage's security controls must have corresponding tests. Tests are organized by threat:

```
tests/
  security/
    fts5_injection_test.rs     # T1: FTS5 injection attempts
    secret_detection_test.rs   # T2: Secret pattern matching
    input_validation_test.rs   # T3: Boundary conditions, invalid input
    file_permissions_test.rs   # T5: Permission verification
    log_sanitization_test.rs   # T6: No secrets in logs
```

Alternatively, security tests can live inline in the module they protect (e.g., `search.rs` contains FTS5 injection tests). The key requirement is coverage, not location.

---

## Dependency Policy

- **Minimal dependencies** — every crate added is attack surface
- **`cargo audit`** on every CI run
- **`Cargo.lock` committed** — no surprise updates
- **`cargo update`** is a deliberate, reviewed action
- **Bundled SQLite** (`rusqlite` `bundled` feature) — no system library dependency, consistent version
- **No `unsafe` in memory-core** — if needed, document why and audit

---

## Incident Response

If a security issue is discovered post-release:

1. Secret exposure: provide `memory-agent purge-secrets` command that re-scans all stored memories
2. FTS5 injection: patch and release, no data corruption possible (FTS5 is read-only index)
3. File permission issue: patch, advise users to run `chmod 600 ~/.memory-agent/memory.db`
