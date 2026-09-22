#!/usr/bin/env bash
set -euo pipefail
CONTEXT_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$CONTEXT_SCRIPT_DIR/scripts/export_context.py" "$@"
