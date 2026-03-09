#!/bin/bash
# Hook: PreCompact — extract key learnings before context is lost.
# Receives JSON on stdin with transcript_path, cwd, source fields.

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
TRANSCRIPT=$(printf '%s' "$INPUT" | jq -r '.transcript_path // ""')
CWD=$(printf '%s' "$INPUT" | jq -r '.cwd // ""')

if [ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Extract learnings from the transcript before compaction
"$MEMORY_AGENT" pre-compact "$TRANSCRIPT" -s "/$PROJECT" 2>/dev/null || true

exit 0
