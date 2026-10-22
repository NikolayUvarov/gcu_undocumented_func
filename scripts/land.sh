#!/bin/bash
# The landing queue (AGENTS.md, section 4): queue, candidate, gate, push, announce. `scripts/land.sh -h` says more.
exec python3 "$(dirname "${BASH_SOURCE[0]}")/land.py" "$@"
