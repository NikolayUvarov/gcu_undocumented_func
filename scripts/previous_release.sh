#!/usr/bin/env bash
# 801-UPD-0038 (801 S4): the release `main` held before this tree, built in a worktree of its own, updates itself to this
# tree's release through its own updater (tests/qemu_smoke.py --suites previous). Usage: previous_release.sh [x86_64].
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
arch=${1:-x86_64}
[[ $arch == x86_64 ]] || { echo "previous_release.sh: x86_64 only for now"; exit 2; }
# main before this tree's change: the merge base with origin/main; for main itself, the commit before it.
git fetch -q origin main 2>/dev/null || true
base=$(git merge-base HEAD origin/main)
[[ $base == "$(git rev-parse HEAD)" ]] && base=$(git rev-parse HEAD^1)
tmp=${TMPDIR:-/tmp}
out=$tmp/mind-previous-$base
# One previous tree at a time: another base's is removed, this base's kept with its build for the next run.
for old in "$tmp"/mind-previous-*; do
    [[ -e $old && $old != "$out" ]] && { git worktree remove --force "$old" 2>/dev/null || rm -rf "$old"; }
done
git worktree prune
[[ -e $out/.git ]] || git worktree add -q --detach "$out" "$base"
echo "THE PREVIOUS RELEASE: $(git log -1 --format='%h %s' "$base")"
if ! (cd "$out" && ./02_build.sh) > "$tmp/mind-previous-build.log" 2>&1; then
    tail -30 "$tmp/mind-previous-build.log"
    echo "FAIL: the previous release ($base) did not build"
    exit 1
fi
exec python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --firmware "${OVMF:-/usr/share/ovmf/OVMF.fd}" --suites previous --previous-tree "$out"
