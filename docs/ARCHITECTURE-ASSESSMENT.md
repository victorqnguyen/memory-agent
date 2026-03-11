# Architecture Assessment — memory-agent

**Assessment framework**: victor-architecture (14 sections)
**Date**: 2026-03-10
**Version assessed**: v0.2.0
**Scope**: Full codebase, all 14 architectural sections

---

## Executive Summary

memory-agent has a **sound, well-reasoned architecture** for a local-first developer tool. The core decisions (Rust workspace separation, SQLite + FTS5, MCP protocol, secret detection, input validation) are solid and well-implemented. The main risks are in operational gaps — missing graceful shutdown, no backup mechanism, missing MCP E2E tests — rather than fundamental design flaws.

**Overall assessment**: Production-ready for Stage 1-2 release with P1 gaps addressed.

---

## Risk Register (Critical & High)

| ID | Section | Gap | Risk | Priority |
|-|-|-|-|-|
| GAP-06-001 | Security | File permissions not programmatically enforced | high | P1 |
| GAP-06-005 | Security | Unix socket peer UID verification not implemented | high | P1 |
| GAP-07-001 | Testing | No MCP tool end-to-end tests | high | P1 |
| GAP-07-002 | Testing | No AsyncStore concurrency tests | high | P1 |
| GAP-11-01 | Resilience | No graceful SIGTERM/SIGINT shutdown handler | high | P1 |
| GAP-11-02 | Resilience | No daemon health heartbeat / liveness check | high | P1 |
| GAP-13-001 | Performance | No latency baselines or performance SLAs defined | high | P1 |
| GAP-14-001 | Compliance | Ollama URL validation allows non-localhost URLs | high | P1 |
| GAP-14-002 | Compliance | No `cargo deny` license audit in CI | high | P1 |
| GAP-02-002 | Data | No backup/export CLI command | high | P2 |
| GAP-02-005 | Data | File permissions not enforced at DB open | high | P2 |
| GAP-02-006 | Data | Encryption key lifecycle undefined (no rotation) | high | P2 |
| GAP-03-001 | API | Value truncation instead of rejection (silent data loss) | medium | P2 |
| GAP-06-002 | Security | Passphrase rotation path undefined | high | P2 |
| GAP-10-003 | Features | No release artifact checksums/signatures | high | P1 |
| GAP-08-006 | DX | No ADR process for post-launch decisions | high | P1 |

---

## Section Summaries

### 01 Foundation — DECIDED ✓
Sound architecture. Rust workspace with compiler-enforced crate boundaries (memory-core WASM-safe, memory-agent async). MSRV 1.85. Zero-warnings policy. WASM compiles but not tested in CI beyond compilation.

**Key decisions**: AD-1 crate boundary enforced, AD-8 AsyncStore pattern, AD-16 zero-log-values.
**Open gap**: WASM integration test missing from CI.

---

### 02 Data — DECIDED with gaps
SQLite + FTS5, transactional migrations, soft-delete with 90-day retention, blake3 dedup, BM25 ranking. Core data model is solid.

**Key gaps**:
- **GAP-02-002 HIGH P2**: No backup CLI (`memory-agent backup`)
- **GAP-02-005 HIGH P2**: DB file permissions (0600) documented but not programmatically verified
- **GAP-02-006 HIGH P2**: No passphrase rotation, no unencrypted→encrypted migration path

---

### 03 API — DECIDED with one gap
15 MCP tools, all `memory_` prefixed, names locked. Input validation at boundary. FTS5 sanitization. Error codes stable (-32602, -32001, -32000).

**Key gap**:
- **GAP-03-001 MEDIUM P2**: Values >2000 chars truncated (not rejected) — silent data loss

---

### 04 Infrastructure — DECIDED ✓
Multi-platform CI (Linux/macOS/Windows). Release gated on CI. cargo audit. Multi-platform binary releases via GitHub Actions.

**Key gap**: No release artifact checksums/signatures (GAP-10-003, carries from 04 as well).

---

### 05 Observability — DECIDED ✓
Dual logging (tracing stderr + error.log at 0600). Event log (SQLite) for TUI Live/Metrics tabs. Value redaction enforced (AD-16). 30s auto-refresh for TUI data tabs.

**Key gap**:
- **GAP-05-001 MEDIUM P1**: Log rotation not implemented — error.log grows unbounded

---

### 06 Security — DECIDED with gaps
FTS5 injection prevention (whitelist + quoting). Secret detection pre-storage. Input validation at MCP boundary. cargo audit in CI. Optional SQLCipher encryption.

**Critical gaps**:
- **GAP-06-001 HIGH P1**: File permissions not programmatically enforced — all critical files
- **GAP-06-005 MEDIUM P1**: Unix socket peer UID verification not yet implemented (Stage 2)
- **GAP-06-002 HIGH P2**: No passphrase rotation mechanism
- **GAP-06-007 MEDIUM P2**: MCP parameter validation inconsistent across tools

---

### 07 Testing — DECIDED with gaps
72+ tests in memory-core. TDD enforced. In-memory SQLite (no mocks). Security tests present (FTS5 injection, secret detection, input validation). Multi-platform CI test matrix.

**Critical gaps**:
- **GAP-07-001 HIGH P1**: No MCP tool E2E tests — public API untested
- **GAP-07-002 HIGH P1**: No AsyncStore concurrency tests — async wrapper untested
- **GAP-07-003 MEDIUM P2**: No daemon lifecycle tests (startup, shutdown, PID, permissions)

---

### 08 Developer Experience — DECIDED ✓
Excellent CLAUDE.md for AI-agent-driven development. Compiler-enforced workspace boundaries. Multi-platform CI. Comprehensive ADRs (AD-1 through AD-22).

**Key gaps**:
- **GAP-08-006 HIGH P1**: No ADR process for post-v0.2.0 decisions (AD-23+)
- **GAP-08-002 MEDIUM P1**: CONTRIBUTING.md missing onboarding path for new contributors
- **GAP-08-003 MEDIUM P1**: No PR template or branch protection rules

---

### 09 Events & Integration — DECIDED ✓
InjectionTracker with 6-hour TTL (recently fixed). Hook system for Claude Code. Event log in SQLite.

**Key gap**:
- **GAP-09-001 MEDIUM P2**: Event log purge not auto-scheduled (manual only)

---

### 10 Feature Management — DECIDED ✓
Cargo feature flags (`tui`, `local-llm`, `encryption`) are well-designed. Multi-platform release automation. Semver with locked MCP tool names.

**Key gaps**:
- **GAP-10-003 HIGH P1**: No release artifact checksums or signatures
- **GAP-10-001 MEDIUM P2**: No feature combination testing in CI (e.g., `--no-default-features`)
- **GAP-10-005 MEDIUM P2**: Encryption "one-command" docs don't match compile-time requirement

---

### 11 Offline & Resilience — DECIDED ✓
memory-agent is inherently offline-first (no cloud). WAL mode + busy timeout for crash resilience. Transactional migrations. Soft-delete + VACUUM.

**Critical gaps**:
- **GAP-11-01 HIGH P1**: No SIGTERM/SIGINT graceful shutdown — in-flight requests killed abruptly
- **GAP-11-02 HIGH P1**: No daemon health verification in `doctor`
- **GAP-11-03 MEDIUM P2**: No backup mechanism (echoes GAP-02-002)
- **GAP-11-05 MEDIUM P2**: No mutex lock timeout — poisoned lock hangs indefinitely

---

### 12 Multi-Tenancy — DECIDED ✓
Scope-based isolation with CSS-specificity-style inheritance is well-designed. Validation at boundaries. Parameterized SQL prevents injection. No traditional multi-tenancy needed (single-user tool).

**Key gap (future)**:
- **GAP-12-004 HIGH P1 (Stage 12)**: Default scope `/` will pollute team namespace if multi-user ever ships — design needed before Stage 12

---

### 13 Performance — DECIDED with gaps
WAL mode, 2048KB cache, BM25 FTS5, blake3 dedup (fast), regex-lite for secret detection (94KB). AsyncStore with spawn_blocking.

**Key gaps**:
- **GAP-13-001 HIGH P1**: No performance baselines or latency SLAs defined
- **GAP-13-002 HIGH P2**: FTS5 query plans unanalyzed
- **GAP-13-003 MEDIUM P2**: Secret detection regex performance unmeasured (runs on every save)

---

### 14 Compliance — DECIDED ✓
MIT license, all deps MIT/Apache-2.0 compatible. No telemetry. No data exfiltration. Secret detection before storage. SQLCipher exempt from export controls.

**Key gaps**:
- **GAP-14-001 HIGH P1**: Ollama URL validation must reject non-localhost — data privacy claim
- **GAP-14-002 HIGH P1**: No `cargo deny` license audit in CI
- **GAP-14-004 MEDIUM P2**: Secret redaction patterns incomplete (missing OpenAI sk- tokens, Slack tokens, JWTs)

---

## Recommended Action Sequence

### Before Stage 2 Release (P1)
1. Add SIGTERM/SIGINT graceful shutdown (GAP-11-01)
2. Programmatically enforce 0600 file permissions (GAP-06-001)
3. Add MCP tool E2E tests (GAP-07-001)
4. Add AsyncStore concurrency tests (GAP-07-002)
5. Add release artifact checksums to GitHub Actions (GAP-10-003)
6. Validate Ollama URL is localhost-only (GAP-14-001)
7. Add `cargo deny` to CI (GAP-14-002)
8. Implement log rotation via `tracing-appender` (GAP-05-001)
9. Document ADR process for AD-23+ (GAP-08-006)
10. Add PR template + branch protection (GAP-08-003)

### Before Stage 5 (P2)
11. Add `memory-agent backup` command (GAP-02-002)
12. Define passphrase rotation path (GAP-02-006)
13. Reject (not truncate) oversized values at MCP boundary (GAP-03-001)
14. Add daemon lifecycle tests (GAP-07-003)
15. Add mutex lock timeout in AsyncStore (GAP-11-05)
16. Expand secret detection patterns (GAP-14-004)
17. Define performance baselines with `#[instrument]` timing (GAP-13-001)
18. Implement Unix socket peer UID verification (GAP-06-005)

### Before v1.0 (P3)
19. Canary Cargo feature combination CI matrix (GAP-10-001)
20. Document encryption one-command correctly (GAP-10-005)
21. Add `memory_list_scopes` MCP tool (GAP-12-002)
22. Design team/multi-user scope convention for Stage 12 (GAP-12-004)
23. Add NO_COLOR support (GAP-14-003)

---

## Open Items (Require Design Decision)

1. **Value truncation vs rejection**: Silent truncation at 2000 chars loses data. Decision: reject with -32602 at MCP boundary, or increase limit and reject? Recommend: reject at MCP boundary, log key only.

2. **Encryption UX**: Current docs say "one command" but requires `--features encryption` recompile. Decision: ship encryption in default binary, or document recompile requirement?

3. **Passphrase rotation mechanism**: No path to rotate encryption key. Decision: `memory-agent encrypt --change-passphrase` CLI command, or rely on export/re-import?

4. **Multi-user scope model (Stage 12)**: Default scope `/` becomes a shared global namespace in team mode. Decision: enforce `/user/{id}/...` scope prefix, or separate DBs per user?

5. **Deprecation policy for MCP tools**: Tools are locked but no sunset process. Decision: add `deprecated` field to MCP tool schema, or version via protocol version bump?
