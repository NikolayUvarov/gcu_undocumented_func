#!/bin/bash
# code_concat.sh — recursively walks a directory and concatenates its text files into two txt files for passing into a
# model's context: the code (everything but constitution/) and the Constitution (constitution/ without its archive/).
#
# Usage:
#   ./code_concat.sh                      # project -> code_handoff/code_context.txt and code_handoff/constitution_context.txt
#   ./code_concat.sh <dir>                # given dir -> <dir>/code_handoff/code_context.txt and constitution_context.txt
#   ./code_concat.sh <dir> <out.txt>      # the code to <out.txt>, the Constitution to constitution_context.txt beside it
#
# Environment variables:
#   EXCLUDE_DIRS="dir1 dir2"   additional directories to exclude
#   MAX_SIZE=1048576           skip files larger than this (bytes), default 1 MiB
#   CONSTITUTION_ARCHIVE=1     include constitution/*/archive/ (the superseded editions) in the Constitution's file

set -euo pipefail

CONCAT_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR=${1:-$CONCAT_SCRIPT_DIR}
MAX_SIZE=${MAX_SIZE:-1048576}

if [ ! -d "$SRC_DIR" ]; then
    echo "Error: directory not found: $SRC_DIR" >&2
    exit 1
fi

SRC_DIR=$(cd "$SRC_DIR" && pwd)
OUT_FILE=${2:-$SRC_DIR/code_handoff/code_context.txt}

# Place output at an absolute path so the walk doesn't pick it up itself.
case "$OUT_FILE" in
    /*) : ;;
    *)  OUT_FILE="$(pwd)/$OUT_FILE" ;;
esac
mkdir -p -- "$(dirname -- "$OUT_FILE")"
CONSTITUTION_FILE="$(dirname -- "$OUT_FILE")/constitution_context.txt"

# Directories that contain no source code.
SKIP_DIRS="
.git .svn .hg
target build dist out
code_handoff code-handoff
node_modules vendor
__pycache__ .venv venv
.idea .vscode .cache
usb_root
${EXCLUDE_DIRS:-}
"

# Build the -prune predicate for find.
prune_args=()
for d in $SKIP_DIRS; do
    prune_args+=( -name "$d" -o )
done
unset 'prune_args[${#prune_args[@]}-1]'   # drop the trailing -o

# Appends every text file find lists (NUL-separated on stdin) to $1; counts in `total` and `skipped`.
append() {
    local out=$1 file size rel fname
    while IFS= read -r -d '' file; do
        # Skip the output files themselves.
        [ "$file" = "$OUT_FILE" ] || [ "$file" = "$CONSTITUTION_FILE" ] && continue

        size=$(stat -c%s "$file" 2>/dev/null || echo 0)
        if [ "$size" -gt "$MAX_SIZE" ]; then
            skipped=$((skipped + 1))
            continue
        fi

        # Skip binary files: grep -Iq succeeds only for text.
        if ! grep -Iq . "$file" 2>/dev/null && [ "$size" -gt 0 ]; then
            skipped=$((skipped + 1))
            continue
        fi

        rel=${file#"$SRC_DIR"/}
        fname=$(basename "$file")

        {
            printf '=== file: %s\n' "$rel"
            printf '=== file %s content:\n' "$fname"
            cat "$file"
            # Ensure a newline before the closing marker.
            [ -n "$(tail -c 1 "$file")" ] && printf '\n'
            printf '=== end of file %s content\n\n' "$fname"
        } >> "$out"

        total=$((total + 1))
    done
}

# The code: everything but the Constitution's directory at the root.
: > "$OUT_FILE"
total=0; skipped=0
append "$OUT_FILE" < <(find "$SRC_DIR" \( \( "${prune_args[@]}" \) -o -path "$SRC_DIR/constitution" \) -prune -o \
                            -type f ! -name 'code_context*.txt' ! -name 'constitution_context*.txt' -print0 | sort -z)
echo "Code: $total files (binary/large skipped: $skipped) -> $OUT_FILE ($(stat -c%s "$OUT_FILE") bytes)"

# The Constitution: constitution/, its superseded editions left out unless asked for.
: > "$CONSTITUTION_FILE"
total=0; skipped=0
archive=( -name archive )
[ "${CONSTITUTION_ARCHIVE:-0}" = 1 ] && archive=( -name '' )
if [ -d "$SRC_DIR/constitution" ]; then
    append "$CONSTITUTION_FILE" < <(find "$SRC_DIR/constitution" \( "${archive[@]}" \) -prune -o -type f -print0 | sort -z)
fi
echo "Constitution: $total files (binary/large skipped: $skipped) -> $CONSTITUTION_FILE ($(stat -c%s "$CONSTITUTION_FILE") bytes)"
