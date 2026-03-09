#!/bin/bash
# Hook: SessionEnd — run lightweight maintenance at conversation end
# Receives JSON on stdin with cwd, reason fields

set -euo pipefail

MEMORY_AGENT="${MEMORY_AGENT_BIN:-$HOME/.cargo/bin/memory-agent}"

if [ ! -x "$MEMORY_AGENT" ]; then
  exit 0
fi

# Run background maintenance (decay, purge, vacuum if overdue) — fire and forget
"$MEMORY_AGENT" maintenance 2>/dev/null || true

exit 0
