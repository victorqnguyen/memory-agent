#!/bin/bash
# Hook: UserPromptSubmit — search memories relevant to the user's prompt
# and inject matching context automatically. Deduplicates across prompts.
# Receives JSON on stdin with prompt, cwd, session_id fields.

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
PROMPT=$(printf '%s' "$INPUT" | jq -r '.prompt // ""')
CWD=$(printf '%s' "$INPUT" | jq -r '.cwd // ""')
SESSION_ID=$(printf '%s' "$INPUT" | jq -r '.session_id // ""')

# Skip very short prompts (single word, commands)
if [ ${#PROMPT} -lt 10 ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Build command args
CMD=("$MEMORY_AGENT" auto-context "$PROMPT" -s "/$PROJECT" -l 5)
if [ -n "$SESSION_ID" ]; then
  CMD+=(--session-id "$SESSION_ID")
fi

# Search for relevant memories using auto-context (with dedup)
CONTEXT=$("${CMD[@]}" 2>/dev/null) || true

# Static injection_prompt — always appended verbatim, no LLM required
STATIC=$("$MEMORY_AGENT" hook-inject user-prompt --static-only 2>/dev/null) || true
if [ -n "$STATIC" ]; then
  if [ -n "$CONTEXT" ]; then
    CONTEXT="${CONTEXT}
${STATIC}"
  else
    CONTEXT="${STATIC}"
  fi
fi

if [ -n "$CONTEXT" ]; then
  printf '%s' "$CONTEXT" | jq -Rs '{
    hookSpecificOutput: {
      hookEventName: "UserPromptSubmit",
      additionalContext: .
    }
  }'
fi

exit 0
