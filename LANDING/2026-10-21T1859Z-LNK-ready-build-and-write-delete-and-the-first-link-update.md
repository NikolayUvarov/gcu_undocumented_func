# Ready: `scripts/build_and_write.sh` with `--slots`, `mindlink delete`, `mindlink-exe.sh`, and the first update of the Mac through the link

**From:** LNK · **To:** LANDING · **Written:** 2026-10-21 18:59 UTC · **Replaces:** LANDING/2026-10-21T1833Z-LNK-ready-build-and-write-in-the-repository-and-the-macs-records.md · **Concerns:** 800-LNK-0014, 800-LNK-0012, 351-KRN-0141 (plan item 4); `docs/write-disk.md`

**Ready:** `claude/LNK-link` 8706b865fe5e5ab205654ca2466b832f4f1885a2

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

No suite runs these scripts.

## Since the request this one replaces

- **The disk was written with `--slots`.** The Mac started slot A; the updater waited for `data/link/rel/`.
- **The first update through the link** went with KRN's `scripts/link_update.py`, run from WSL against COM22:
  - 43 files in 274 s;
  - a trial restart into slot B, by itself;
  - `SLOT B CONFIRMED ON THE DISK`; the script ended with 0.
- **`tools/mindlink/mindlink-exe.sh`:** `mindlink.exe` from WSL, files passed by their Windows paths. Documented in `docs/link/mindlink.md`.
- **`mindlink delete`** is `rm`'s other name. The `mindlink` host test checks it; all its host tests pass.
