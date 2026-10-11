#!/usr/bin/env bash
# Copies main and the tags of this repository on GitHub to the showcase, github.com/NikolayUvarov/mind-core-os, with
# your own git access (your SSH key): only what changed, never forced (000-KRN-0088). Run it from any clone of this
# repository, on any branch; Linux, macOS, WSL or Git Bash.
#
#   ./06_sync_showcase.sh                 copy what changed now
#   ./06_sync_showcase.sh --every-hours 48   copy only if the showcase's last push is 48 hours old (for cron)
#   ./06_sync_showcase.sh [...] URL       another address of the showcase (default: its SSH address)
#
# On Windows without bash: 06_sync_showcase_windows.ps1.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
every=()
if [[ ${1:-} == --every-hours ]]; then every=(--every-hours "$2"); shift 2; fi
exec scripts/mirror_showcase.sh ${every[@]+"${every[@]}"} "${1:-git@github.com:NikolayUvarov/mind-core-os.git}"
