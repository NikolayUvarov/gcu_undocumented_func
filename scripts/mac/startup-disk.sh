#!/bin/sh
# Makes the MIND CORE disk this script is on an Intel Mac's startup disk (800-LNK-0029, docs/write-disk.md, section 6).
# build_and_write.sh copies it onto the disk's boot volume as mac/startup-disk.sh. In the Terminal of macOS Recovery
# (Command-R at power-on, then Utilities, Terminal), with the disk attached and diskN found by `diskutil list`:
#   mkdir -p /Volumes/MINDCORE
#   mount -t msdos /dev/diskNs1 /Volumes/MINDCORE
#   sh /Volumes/MINDCORE/mac/startup-disk.sh
# macOS itself refuses it: System Integrity Protection (bless answers 0xe00002e2 there).

fail() { echo "startup-disk: $*" >&2; exit 1; }

case $0 in */*) here=${0%/*} ;; *) here=. ;; esac
volume=$(cd "$here/.." && pwd -P) || fail "cannot find the volume of $0"
loader=$volume/EFI/BOOT/BOOTX64.EFI
[ -f "$loader" ] || fail "no EFI/BOOT/BOOTX64.EFI on $volume: mount the disk's boot partition and run the script from it"

# The partition's device, as bless --getBoot names it: diskutil's line "Device Node:  /dev/diskNsM".
info=$(diskutil info "$volume" 2>/dev/null)
case $info in *"Device Node:"*) ;; *) fail "diskutil does not know $volume: is it mounted?" ;; esac
node=${info#*Device Node:}
node=${node%%
*}
node=${node##* }

echo "startup-disk: $node, $loader"
bless --mount "$volume" --setBoot --file "$loader" --shortform ||
    fail "bless refused: run this in the Terminal of macOS Recovery (Command-R at power-on), not in macOS"
boot=$(bless --getBoot)
[ "$boot" = "$node" ] || fail "the firmware starts $boot, not $node"
echo "startup-disk: the Mac starts $node while the disk is attached, and macOS when it is not"
echo "startup-disk: now cd / && umount $volume, then restart"
