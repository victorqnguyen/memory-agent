#!/bin/bash
# Hook: TaskCompleted — a task finished. Capture outcome as procedural memory.
# Receives JSON on stdin with task_id, task_description, result, cwd fields.

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
TASK_ID=$(printf '%s' "$INPUT" | jq -r '.task_id // ""')
DESCRIPTION=$(printf '%s' "$INPUT" | jq -r '.task_description // ""')
RESULT=$(printf '%s' "$INPUT" | jq -r '.result // ""')

if [ -z "$DESCRIPTION" ] && [ -z "$RESULT" ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Truncate result to fit value limit
RESULT=$(printf '%.1500s' "$RESULT")

VALUE="Task: $DESCRIPTION
Result: $RESULT"

"$MEMORY_AGENT" save \
  -k "task/$TASK_ID" \
  -v "$VALUE" \
  --scope "/$PROJECT" \
  --source-type derived \
  -t task completed 2>/dev/null || true

exit 0
