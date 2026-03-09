#!/bin/bash
# Hook: SubagentStop — a subagent finished. Capture what it learned.
# Receives JSON on stdin with subagent_id, subagent_type, result, cwd fields.

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
AGENT_ID=$(printf '%s' "$INPUT" | jq -r '.subagent_id // ""')
AGENT_TYPE=$(printf '%s' "$INPUT" | jq -r '.subagent_type // "unknown"')
RESULT=$(printf '%s' "$INPUT" | jq -r '.result // ""')

# Skip if no meaningful result
if [ ${#RESULT} -lt 20 ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Truncate result to fit value limit
RESULT=$(printf '%.1500s' "$RESULT")

"$MEMORY_AGENT" save \
  -k "subagent/$AGENT_TYPE/$AGENT_ID" \
  -v "$RESULT" \
  --scope "/$PROJECT" \
  --source-type derived \
  -t subagent "$AGENT_TYPE" 2>/dev/null || true

exit 0
