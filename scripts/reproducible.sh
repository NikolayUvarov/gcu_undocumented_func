#!/bin/bash
# Reproducible build check (350-UPD-0001, docs/update/README.md; MC-9.5, 9.7). Builds one commit twice, each time from
# a fresh checkout at the same path with the pinned toolchain and the locked dependencies, and compares every staged
# file byte for byte: any difference fails. With --other-path it also builds at a second path and lists what differs
# there, since the checkout path is a condition that affects the result (docs/update/README.md).
# Usage: scripts/reproducible.sh [--arch x86_64|aarch64] [--other-path] [--keep] [REF]
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH=x86_64 OTHER=0 KEEP=0 REF=HEAD
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch) ARCH="$2"; shift 2 ;;
        --other-path) OTHER=1; shift ;;
        --keep) KEEP=1; shift ;;
        -h|--help) sed -n '2,6p' "$0"; exit 0 ;;
        *) REF="$1"; shift ;;
    esac
done
case "$ARCH" in x86_64) OUT=usb_root ;; aarch64) OUT=aarch64_root ;; *) echo "--arch x86_64 or aarch64" >&2; exit 2 ;; esac
COMMIT=$(git -C "$ROOT" rev-parse --verify "$REF^{commit}") || exit 2
BASE=$(mktemp -d /tmp/mind-repro.XXXXXX)
[[ $KEEP == 1 ]] || trap 'git -C "$ROOT" worktree prune; rm -rf "$BASE"' EXIT

# Builds the commit in a fresh checkout at $1 and records the hash of every staged file in $BASE/$2.sums.
build() {
    local where="$1" label="$2" started=$SECONDS
    git -C "$ROOT" worktree add -q --detach "$where" "$COMMIT" || return 1
    if ! (cd "$where" && ARCH=$ARCH bash 02_build.sh > "$BASE/$label.log" 2>&1); then
        echo "build $label failed: $BASE/$label.log"; return 1
    fi
    (cd "$where/$OUT" && find . -type f -print0 | sort -z | xargs -0 sha256sum) > "$BASE/$label.sums"
    echo "built $label at $where in $((SECONDS - started)) s: $(wc -l < "$BASE/$label.sums") files"
    [[ $KEEP == 1 ]] || git -C "$ROOT" worktree remove --force "$where"
}

echo ">>> $COMMIT for $ARCH, twice at $BASE/src"
build "$BASE/src" first || exit 1
[[ $KEEP == 1 ]] && mv "$BASE/src" "$BASE/first-src" && git -C "$ROOT" worktree repair "$BASE/first-src"
build "$BASE/src" second || exit 1
if ! diff -q "$BASE/first.sums" "$BASE/second.sums" > /dev/null; then
    echo "NOT REPRODUCIBLE at one path; files that differ:"
    diff "$BASE/first.sums" "$BASE/second.sums" | grep '^[<>]' | awk '{print $3}' | sort -u
    exit 1
fi
echo "REPRODUCIBLE: $(wc -l < "$BASE/first.sums") files identical in two builds at one path"
if [[ $OTHER == 1 ]]; then
    build "$BASE/elsewhere/deeper/src" other || exit 1
    differ=$(diff "$BASE/first.sums" "$BASE/other.sums" | grep '^[<>]' | awk '{print $3}' | sort -u)
    echo "AT ANOTHER PATH: $(echo -n "$differ" | grep -c .) of $(wc -l < "$BASE/first.sums") files differ"
    echo "$differ"
fi
