#!/usr/bin/env bash
# 801-UPD-0038 and 0040 (801 S4): the release `main` held before this tree, built in a worktree of its own, updates itself
# to this tree's release through its own updater (tests/qemu_smoke.py --suites previous). Usage: previous_release.sh [x86_64|aarch64].
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
arch=${1:-x86_64}
case $arch in
    x86_64) harness=(--qemu qemu-system-x86_64 --firmware "${OVMF:-/usr/share/ovmf/OVMF.fd}") ;;
    aarch64) harness=(--arch aarch64) ;;
    *) echo "previous_release.sh: x86_64 or aarch64, not $arch"; exit 2 ;;
esac
# main before this tree's change: the merge base with origin/main; for main itself, the commit before it.
git fetch -q origin main 2>/dev/null || true
base=$(git merge-base HEAD origin/main)
[[ $base == "$(git rev-parse HEAD)" ]] && base=$(git rev-parse HEAD^1)
tmp=${TMPDIR:-/tmp}
out=$tmp/mind-previous-$arch-$base
# One previous tree for each architecture: any other of this name is removed, this base's kept with its build for the next run.
other=$([[ $arch == x86_64 ]] && echo aarch64 || echo x86_64)
for old in "$tmp"/mind-previous-*; do
    [[ -d $old && $old != "$out" && $old != "$tmp/mind-previous-$other-"* ]] && { git worktree remove --force "$old" 2>/dev/null || rm -rf "$old"; }
done
git worktree prune
[[ -e $out/.git ]] || git worktree add -q --detach "$out" "$base"
echo "THE PREVIOUS RELEASE ($arch): $(git log -1 --format='%h %s' "$base")"
if ! (cd "$out" && ARCH=$arch ./02_build.sh) > "$tmp/mind-previous-$arch-build.log" 2>&1; then
    tail -30 "$tmp/mind-previous-$arch-build.log"
    echo "FAIL: the previous release ($base) did not build for $arch"
    exit 1
fi
exec python3 tests/qemu_smoke.py "${harness[@]}" --suites previous --previous-tree "$out"
