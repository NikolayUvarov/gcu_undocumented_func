#!/usr/bin/env bash
# What a damaged old disk does to the kept files (800-LNK-0036): the writer's own keep functions, taken from
# scripts/build_and_write.sh by name, run on plain directories; no disk, mount or Windows is involved.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
for name in has_content rotate_kept keep_file refill_empty keep_effector_from keep_settings_from; do
    eval "$(awk -v n="$name" '$0 ~ "^" n "\\(\\) \\{" { on = 1 } on { print } on && (/^}$/ || ($0 ~ "^" n "\\(\\) \\{.*\\}$")) { exit }' "$ROOT/scripts/build_and_write.sh")"
    declare -F "$name" > /dev/null || { echo "FAIL: $name not found in build_and_write.sh"; exit 1; }
done
SETTINGS_SKIP_DATA=(effector firmware wallpapers link wifitest.txt wifi-ofdm.txt wifijoin.txt)
SETTINGS_SKIP_SYSTEM=(keystore netpolicy effector)
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
fail=0
check() { if [[ $2 == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: wanted '$3', got '$2'"; fail=1; fi; }
put() { mkdir -p "$(dirname "$1")"; printf '%s' "$2" > "$1"; }

# 2026-11-05: the old disk lost data/effector; its system/effector survived.
EFFECTOR_KEEP=$work/a/effector
put "$EFFECTOR_KEEP/data/effector.conf" conf-saved; put "$EFFECTOR_KEEP/data/policy" policy-saved; put "$EFFECTOR_KEEP/system/agent_id" id-saved
put "$work/a/disk/system/effector/agent_id" id-disk
n=$(keep_effector_from "$work/a/disk" 2> "$work/a/err")
check "a lost data/effector keeps the saved configuration" "$(cat "$EFFECTOR_KEEP/data/effector.conf") $(cat "$EFFECTOR_KEEP/data/policy")" "conf-saved policy-saved"
check "the disk's surviving part is taken" "$(cat "$EFFECTOR_KEEP/system/agent_id")" id-disk
check "the count covers what is kept" "$n" 3
check "the replaced copy stays as .prev" "$(cat "$EFFECTOR_KEEP.prev/system/agent_id")" id-saved
check "the kept part is said" "$(grep -c 'data/effector is missing or empty' "$work/a/err")" 1

# An empty file on the disk keeps the saved file; the disk's other files are taken.
EFFECTOR_KEEP=$work/b/effector
put "$EFFECTOR_KEEP/data/effector.conf" conf-saved; put "$EFFECTOR_KEEP/data/policy" policy-saved
put "$work/b/disk/data/effector/effector.conf" ""; put "$work/b/disk/data/effector/policy" policy-disk
keep_effector_from "$work/b/disk" > /dev/null
check "an empty effector.conf is refilled from the saved one" "$(cat "$EFFECTOR_KEEP/data/effector.conf")" conf-saved
check "a policy with content is the disk's" "$(cat "$EFFECTOR_KEEP/data/policy")" policy-disk

# A first save: nothing before, the disk's files whole, no .prev.
EFFECTOR_KEEP=$work/c/effector
put "$work/c/disk/data/effector/effector.conf" conf-disk; put "$work/c/disk/system/effector/agent_id" id-disk
keep_effector_from "$work/c/disk" > /dev/null
check "a first save takes the disk's files" "$(cat "$EFFECTOR_KEEP/data/effector.conf") $(cat "$EFFECTOR_KEEP/system/agent_id")" "conf-disk id-disk"
check "a first save leaves no .prev" "$([[ -e $EFFECTOR_KEEP.prev ]] && echo yes || echo no)" no
check "the kept files are the owner's alone" "$(stat -c %a "$EFFECTOR_KEEP/data/effector.conf") $(stat -c %a "$EFFECTOR_KEEP/data")" "600 700"

# Settings: a disk without data/ keeps the saved data/, and takes its system/.
SETTINGS_KEEP=$work/d/settings
put "$SETTINGS_KEEP/data/wm.conf" wm-saved; put "$SETTINGS_KEEP/system/x.conf" x-saved
put "$work/d/disk/system/x.conf" x-disk; put "$work/d/disk/system/keystore/device.key" key
keep_settings_from "$work/d/disk" > /dev/null 2>&1
check "a lost data/ keeps the saved settings" "$(cat "$SETTINGS_KEEP/data/wm.conf")" wm-saved
check "the disk's system/ settings are taken" "$(cat "$SETTINGS_KEEP/system/x.conf")" x-disk
check "the keystore stays out of the settings" "$([[ -e $SETTINGS_KEEP/system/keystore ]] && echo yes || echo no)" no
put "$work/d/disk/data/wm.conf" wm-disk
keep_settings_from "$work/d/disk" > /dev/null
check "settings the disk has are its own" "$(cat "$SETTINGS_KEEP/data/wm.conf")" wm-disk

# One keystore file: an empty one keeps the saved one; one with content replaces it, the old one as .prev.
put "$work/e/keep/device.key" key-saved; put "$work/e/disk/device.key" ""
if keep_file "$work/e/disk/device.key" "$work/e/keep/device.key" 2> /dev/null; then taken=yes; else taken=no; fi
check "an empty device.key is not taken" "$taken $(cat "$work/e/keep/device.key")" "no key-saved"
put "$work/e/disk/device.key" key-disk
keep_file "$work/e/disk/device.key" "$work/e/keep/device.key"
check "a device.key with content is taken, the old one kept" "$(cat "$work/e/keep/device.key") $(cat "$work/e/keep/device.key.prev")" "key-disk key-saved"

[[ $fail == 0 ]] && echo "build_and_write keep: all passed"
exit "$fail"
