#!/bin/bash
# Hook: SessionStart — load project context at conversation start
# Receives JSON on stdin with cwd, source fields

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
CWD=$(printf '%s' "$INPUT" | jq -r '.cwd // ""')
SOURCE=$(printf '%s' "$INPUT" | jq -r '.source // "startup"')

PROJECT=$(basename "$CWD")

# Skip on compaction — context already loaded
if [ "$SOURCE" = "compact" ]; then
  exit 0
fi

CONTEXT=""

# Extract from project files (fast, idempotent)
EXTRACT=$("$MEMORY_AGENT" extract -d "$CWD" --scope "/$PROJECT" 2>/dev/null) || true
if [ -n "$EXTRACT" ]; then
  CONTEXT="[memory-agent extract] $EXTRACT"
fi

# List existing memories for this project
MEMORIES=$("$MEMORY_AGENT" list -s "/$PROJECT" -l 10 2>/dev/null) || true
if [ -n "$MEMORIES" ]; then
  CONTEXT="${CONTEXT}"$'\n'"[memory-agent] Project memories:"$'\n'"$MEMORIES"
fi

# Static injection_prompt — always appended verbatim, no LLM required
STATIC=$("$MEMORY_AGENT" hook-inject session-start --static-only 2>/dev/null) || true
if [ -n "$STATIC" ]; then
  CONTEXT="${CONTEXT}
${STATIC}"
fi

if [ -n "$CONTEXT" ]; then
  printf '%s' "$CONTEXT" | jq -Rs '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: .
    }
  }'
fi

exit 0
