#!/bin/bash
# Hook: PostToolUse (Edit|Write) — observe file edits passively.
# Runs async (fire-and-forget) to avoid blocking.
# Receives JSON on stdin with tool_name, tool_input, cwd fields.

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
FILE_PATH=$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // ""')
CWD=$(printf '%s' "$INPUT" | jq -r '.cwd // ""')

if [ -z "$FILE_PATH" ]; then
  exit 0
fi

PROJECT=$(basename "$CWD")

# Record the edit observation (fire-and-forget)
"$MEMORY_AGENT" observe-edit "$FILE_PATH" -s "/$PROJECT" 2>/dev/null || true

exit 0
