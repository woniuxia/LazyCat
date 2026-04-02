#!/usr/bin/env bash
set -euo pipefail

CODEX_HOME_DIR="${CODEX_HOME:-$HOME/.codex}"
TARGET="$CODEX_HOME_DIR/skills/brainstorming/scripts/start-server.sh"

if [[ ! -f "$TARGET" ]]; then
  echo "{\"error\":\"Brainstorm server script not found: $TARGET\"}"
  exit 1
fi

exec "$TARGET" "$@"
