#!/usr/bin/env bash
# mindlink.exe called from WSL, for a converter on a Windows COM port: an argument naming an existing file or directory
# here goes as its Windows path. MINDLINK_EXE names the program (default: mindlink.exe on the PATH).
# Example: scripts/link_update.py COM22 --mindlink tools/mindlink/mindlink-exe.sh --mindlink-option=--baud --mindlink-option=460800
args=()
for a in "$@"; do if [[ $a == /* && -e $a ]]; then args+=("$(wslpath -w "$a")"); else args+=("$a"); fi; done
exec "${MINDLINK_EXE:-mindlink.exe}" "${args[@]}"
