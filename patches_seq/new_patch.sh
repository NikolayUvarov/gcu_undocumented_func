#!/bin/bash
# new_patch.sh — creates the next numbered patch, reads its text from
# standard input and applies it immediately.
#
# Usage:
#   ./new_patch.sh              create patch_<max+1>.sh, enter text, apply
#   ./new_patch.sh -t           start from a header template (SCRIPT_DIR/ROOT_DIR)
#   ./new_patch.sh -n           only create the file, do not run it
#
# Patch text is read until Ctrl-D. Empty input cancels creation.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

USE_TEMPLATE=0
RUN_AFTER=1

while [ $# -gt 0 ]; do
    case "$1" in
        -t|--template) USE_TEMPLATE=1 ;;
        -n|--no-run)   RUN_AFTER=0 ;;
        -h|--help)     sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
    shift
done

# --- Determine the highest number -----------------------------------------

max=0
for f in "$SCRIPT_DIR"/patch_*.sh; do
    [ -e "$f" ] || continue
    n=$(basename "$f" | sed -n 's/^patch_0*\([0-9]\+\).*/\1/p')
    [ -n "$n" ] || continue
    [ "$n" -gt "$max" ] && max=$n
done

# If the last patch is empty, reuse its number instead of creating a new one.
reuse=""
if [ "$max" -gt 0 ]; then
    last=$(printf '%s/patch_%03d' "$SCRIPT_DIR" "$max")
    for cand in "$last".sh "$last"_*.sh; do
        if [ -f "$cand" ] && [ ! -s "$cand" ]; then
            reuse="$cand"
            break
        fi
    done
fi

if [ -n "$reuse" ]; then
    NUM=$max
    PATCH_FILE="$reuse"
    echo "Found empty patch $(basename "$PATCH_FILE"); filling it in."
else
    NUM=$((max + 1))
    PATCH_FILE=$(printf '%s/patch_%03d.sh' "$SCRIPT_DIR" "$NUM")
fi

if [ -s "$PATCH_FILE" ]; then
    echo "Error: $PATCH_FILE already exists and is not empty." >&2
    exit 1
fi

# --- Prompt and input -----------------------------------------------------

printf 'Highest patch number: %03d\n' "$max"
printf 'New patch: %s\n' "$(basename "$PATCH_FILE")"
echo "Enter the patch text. Finish with Ctrl-D; empty input cancels."
echo "----------------------------------------------------------------"
printf 'cat > %s\n' "$PATCH_FILE"

TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT

if [ "$USE_TEMPLATE" = 1 ]; then
    cat > "$TMP" <<TEMPLATE
#!/bin/bash
set -e

SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="\$(cd "\$SCRIPT_DIR/.." && pwd)"

echo ">>> Applying patch $(printf '%03d' "$NUM"): ..."

TEMPLATE
fi

cat >> "$TMP"

echo "----------------------------------------------------------------"

# "Empty" threshold: without a template — zero bytes, with a template — only the template itself.
min_lines=0
[ "$USE_TEMPLATE" = 1 ] && min_lines=8

if [ "$(wc -l < "$TMP")" -le "$min_lines" ]; then
    echo "No content entered; patch not created."
    exit 0
fi

cp "$TMP" "$PATCH_FILE"
echo "Written: $PATCH_FILE ($(wc -l < "$PATCH_FILE") lines)"

# --- Syntax check and run -------------------------------------------------

if ! bash -n "$PATCH_FILE"; then
    echo "Syntax error in patch; run cancelled." >&2
    exit 1
fi

if [ "$RUN_AFTER" = 0 ]; then
    echo "Run skipped (-n). Apply manually: bash $PATCH_FILE"
    exit 0
fi

echo ">>> Running $(basename "$PATCH_FILE") from $ROOT_DIR ..."
cd "$ROOT_DIR"
bash "$PATCH_FILE"
