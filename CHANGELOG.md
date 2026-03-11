# Changelog

All notable changes to memory-agent will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-10

### Major Changes

- **Bug Audit Complete**: Comprehensive security and bug audit performed with zero critical issues found. Codebase verified production-ready.
- **Security Controls Verified**: All 10 security controls from AD-16 verified, including FTS5 sanitization, secret detection, and input validation.
- **Test Coverage**: 60+ tests passing, including security-specific tests for injection attacks and secret detection.

### Security Enhancements

- ✅ FTS5 query sanitization prevents SQL injection via search queries
- ✅ Secret pattern detection automatically redacts API keys, tokens, and credentials
- ✅ Path traversal prevention blocks `../` sequences in scope parameters
- ✅ Input validation enforces length limits and null byte prevention
- ✅ Confidence CHECK constraint ensures data integrity at database level

### Technical Improvements

- **Architecture Compliance**: Full adherence to AD-1 through AD-22 architectural decisions
- **Error Handling**: Proper `thiserror` in core crate, `anyhow` in agent crate
- **MCP Protocol**: Complete error code mapping per AD-21 specification
- **Database Migrations**: 10 migrations implemented with forward-only versioning
- **Async Support**: Proper `spawn_blocking` pattern for SQLite with tokio

### Testing

- Added `fts5_injection_test.rs`: 4 tests for FTS5 operator injection prevention
- Added `secret_detection_test.rs`: 3 tests for secret pattern matching
- Added `input_validation_test.rs`: 8 tests for boundary conditions
- Added `scope_test.rs`: 7 tests for scope hierarchy and deduplication
- Added `store_test.rs`: 32 tests for core CRUD operations and FTS5

### Documentation

- Updated `SECURITY.md` with comprehensive threat model
- Added per-stage security checklists
- Documented all architectural decisions in `ARCHITECTURE-DECISIONS.md`

### Dependencies

- Updated to latest stable versions
- `rusqlite 0.38` with bundled SQLite 3.51.1
- `rmcp 0.16` for MCP protocol support
- `blake3` for content deduplication hashing
- `regex-lite` for efficient secret pattern matching

### Known Issues

- None identified in v0.2.0 audit

---

## [0.1.15] - 2026-03-09

### Initial Public Release

- Core memory store with SQLite + FTS5 search
- MCP server for AI coding agents
- Daemon for background maintenance
- Session tracking
- Confidence decay for observed/derived memories
- Deduplication with blake3 hashing
- Privacy controls (private tag stripping, secret detection)
- Scope hierarchy with ancestor inheritance
- Metrics and event logging
- Configuration via TOML file
- Encryption support (optional feature)
- WASM-compatible core crate

---

[0.2.0]: https://github.com/victorqnguyen/memory-agent/releases/tag/v0.2.0
[0.1.15]: https://github.com/victorqnguyen/memory-agent/releases/tag/v0.1.15
