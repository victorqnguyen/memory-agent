#!/bin/bash
# Hook: Stop — Claude finished responding. Capture activity summary.
# Receives JSON on stdin with session_id, cwd, stop_reason, num_turns fields.

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
STOP_REASON=$(printf '%s' "$INPUT" | jq -r '.stop_reason // ""')
NUM_TURNS=$(printf '%s' "$INPUT" | jq -r '.num_turns // 0')

# Only capture meaningful interactions (more than 2 turns)
if [ "$NUM_TURNS" -lt 2 ] 2>/dev/null; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Log the stop event as an observation
"$MEMORY_AGENT" save \
  -k "observed/stop/$(date +%s)" \
  -v "Session stop ($STOP_REASON) after $NUM_TURNS turns in $PROJECT" \
  --scope "/$PROJECT" \
  --source-type observed \
  -t observed stop 2>/dev/null || true

exit 0
