#!/usr/bin/env bash
MEMORY_AGENT="${MEMORY_AGENT:-memory-agent}"
"$MEMORY_AGENT" hook-gate agent_review_gate 2>/dev/null || exit 0
INPUT=$(cat)
PROMPT=$(printf '%s' "$INPUT" | jq -r '.tool_input.prompt // ""')

if echo "$PROMPT" | grep -qiE 'reviewer|auditor|finalizer|explorer'; then
  exit 0
fi

printf '{"message":"[HOOK] Agent task completed. Dispatch the reviewer on the full diff before proceeding to the next phase."}'
