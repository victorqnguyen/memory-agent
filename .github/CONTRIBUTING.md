# Contributing to memory-agent

## What's Public vs Private

### ✅ Safe to commit (public docs):
- `README.md` - Main project documentation
- `docs/ARCHITECTURE.md` - High-level system design
- `docs/SECURITY.md` - Security documentation
- `docs/USAGE.md` - Usage documentation
- `docs/guides/` - Agent integration guides
- `docs/spec/` - Protocol specifications
- `LICENSE*` files
- Cargo.toml files (workspace configuration)
- Source code in `crates/`
- Test files and fixtures

### ❌ Never commit (internal/sensitive):
- `CLAUDE.md` - Internal development guidelines
- `.claude/` - Local Claude Code configuration
- `.mcp.json` - Personal MCP server configuration
- `plan/` - Detailed implementation plans
- `docs/plans/` - Internal planning documents
- Any files with personal paths (e.g., `/Users/username/`)
- Research notes, drafts, experiments
- Configuration with secrets, API keys, or credentials
- Local development artifacts

### Security Guidelines

1. **No personal information**: Remove any personal file paths, usernames, or system-specific details
2. **No secrets**: API keys, tokens, passwords, or other credentials must never be committed
3. **No internal processes**: Implementation details, planning documents, and development workflows should remain private
4. **Review documentation**: Before pushing any documentation changes, verify they don't expose sensitive information

### Pull Request Checklist

Before submitting a PR:

- [ ] No personal file paths or usernames in committed files
- [ ] No API keys, tokens, or secrets in any files
- [ ] Documentation focuses on public features, not internal implementation
- [ ] Test data doesn't contain sensitive information
- [ ] Configuration examples use placeholder values only

### Questions?

If you're unsure whether something should be public, err on the side of caution and ask in the issue discussion first.