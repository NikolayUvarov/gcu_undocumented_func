#!/bin/bash
# code_concat.sh — recursively walks a directory and concatenates all text files
# into a single txt for passing into a model's context.
#
# Usage:
#   ./code_concat.sh                      # project -> code_handoff/code_context.txt
#   ./code_concat.sh <dir>                # given dir -> <dir>/code_handoff/code_context.txt
#   ./code_concat.sh <dir> <out.txt>      # explicitly specified output file
#
# Environment variables:
#   EXCLUDE_DIRS="dir1 dir2"   additional directories to exclude
#   MAX_SIZE=1048576           skip files larger than this (bytes), default 1 MiB

set -euo pipefail

CONCAT_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR=${1:-$CONCAT_SCRIPT_DIR}
MAX_SIZE=${MAX_SIZE:-1048576}

if [ ! -d "$SRC_DIR" ]; then
    echo "Ошибка: каталог не найден: $SRC_DIR" >&2
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

: > "$OUT_FILE"

total=0
skipped=0

while IFS= read -r -d '' file; do
    # Skip the output file itself.
    [ "$file" = "$OUT_FILE" ] && continue

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
    } >> "$OUT_FILE"

    total=$((total + 1))
done < <(find "$SRC_DIR" \( "${prune_args[@]}" \) -prune -o -type f ! -name 'code_context*.txt' -print0 | sort -z)

echo "Записано файлов: $total (пропущено бинарных/крупных: $skipped)"
echo "Результат: $OUT_FILE ($(stat -c%s "$OUT_FILE") байт)"
