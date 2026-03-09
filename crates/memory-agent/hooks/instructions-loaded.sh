#!/bin/bash
# Hook: InstructionsLoaded — re-extract if instructions changed, inject delta context.
# Deduplicates injections across the session.
# Receives JSON on stdin with file_path, cwd, session_id fields.

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
FILE_PATH=$(printf '%s' "$INPUT" | jq -r '.file_path // ""')
CWD=$(printf '%s' "$INPUT" | jq -r '.cwd // ""')
SESSION_ID=$(printf '%s' "$INPUT" | jq -r '.session_id // ""')

if [ -z "$FILE_PATH" ] || [ ! -f "$FILE_PATH" ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Build command args
CMD=("$MEMORY_AGENT" instructions-loaded "$FILE_PATH" -s "/$PROJECT")
if [ -n "$SESSION_ID" ]; then
  CMD+=(--session-id "$SESSION_ID")
fi

# Run instructions-loaded: hashes file, re-extracts if changed, outputs delta context
OUTPUT=$("${CMD[@]}" 2>/dev/null) || true

# Gather all context into MERGED, then emit a single JSON output
CONTEXT=$(printf '%s' "$OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // ""' 2>/dev/null) || true
MERGED="$CONTEXT"

# Static injection_prompt — always appended verbatim, no LLM required
STATIC=$("$MEMORY_AGENT" hook-inject instructions-loaded --static-only 2>/dev/null) || true
if [ -n "$STATIC" ]; then
  if [ -n "$MERGED" ]; then
    MERGED="${MERGED}
${STATIC}"
  else
    MERGED="${STATIC}"
  fi
fi

if [ -n "$MERGED" ]; then
  printf '%s' "$MERGED" | jq -Rs '{
    hookSpecificOutput: {
      hookEventName: "InstructionsLoaded",
      additionalContext: .
    }
  }'
elif [ -n "$OUTPUT" ]; then
  printf '%s' "$OUTPUT"
fi

exit 0
