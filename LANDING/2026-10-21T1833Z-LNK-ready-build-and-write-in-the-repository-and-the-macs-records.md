# Ready: `scripts/build_and_write.sh` with `--slots`, and the MacBook Pro's records

**From:** LNK · **To:** LANDING · **Written:** 2026-10-21 18:33 UTC · **Concerns:** 800-LNK-0014, 800-LNK-0012, 351-KRN-0141 (plan item 4); `docs/write-disk.md`

**Ready:** `claude/LNK-link` 2be6315cb880c41f98117aefd52fb9d9e582c021

## What it brings

- **800-LNK-0014: the maintainer's `build-and-write.sh` in the repository** as `scripts/build_and_write.sh`, at the maintainer's request.
  - On the maintainer's machine the old path is now a symbolic link to it.
  - It runs from a copy of itself, because the clone it lives in is checked out and pulled while it runs.
  - It builds the repository it lives in.
  - After `--commit` it returns the clone to its branch.
- **`--slots`, KRN's 351-KRN-0141 plan item 4:**
  - it writes an image with slots, an `update.txt` naming `data/link/rel`, and `link.conf`;
  - the volume is then found by the build's MANIFEST at its root or in `MIND/A`, in the script and in DRV's `scripts/disk_copy.sh`.
- **Text:**
  - `docs/write-disk.md` section 5 describes the script;
  - section 6 tells how to make the stick an Intel Mac's default startup disk with `bless` from macOS Recovery (macOS itself refuses it);
  - 800-LNK-0012's record corrected: the restart came back by hand before `bless`, and by itself after it.

## `main` in it

e4fd3b3.

## What was run

On the link track's WSL machine:
- `bash -n` of both scripts;
- `--list` through the maintainer's symbolic link: the USB disks listed, no copy left behind, a copy variable from outside refused;
- **an image built with `--slots --update`, then read:**
  - `update.txt` at the root, as `link_update.py` asks;
  - the build in `MIND/A`, its MANIFEST equal to `usb_root/MANIFEST`;
- the PowerShell test that finds the volume, against a file on C:.

No suite runs these scripts. The disk write with `--slots` is the maintainer's next one.
