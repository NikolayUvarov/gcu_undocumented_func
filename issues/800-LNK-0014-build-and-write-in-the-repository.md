# 800-LNK-0014 — The maintainer's `build-and-write.sh` in the repository, as `scripts/build_and_write.sh`

**Type:** tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

The maintainer writes every image for the MacBook Pro with `build-and-write.sh`.
- **What it does:** it pulls `fast-test`, builds, writes the disk from WSL through an elevated PowerShell, and then puts the test files on it: `link.conf`, the kept Wi-Fi networks and network policy, `data/wifi-ofdm.txt` and others.
- **Where it lived:** outside the repository, next to the clone it builds. So its changes were in no history, and the tracks that added options to it (LNK, DRV, ASR) changed a file no one else could see.

The maintainer asked on 2026-10-21 to keep it in the repository, and to leave a symbolic link where it was.

## Plan

1. **`scripts/build_and_write.sh`,** the script as it was, with three changes:
   - it runs from a copy of itself in `$TMPDIR`, because the clone it lives in is checked out and pulled while it runs;
   - it builds the repository it lives in, also when started through a symbolic link: `git rev-parse --show-toplevel` of its real path;
   - after `--commit <id>` the clone returns to its branch when the script ends, so the script is not left out of the tree.

   `--list` no longer prints PowerShell's progress records.
   - **`--slots`** (KRN's 351-KRN-0141, plan item 4):
     - the image is made with `make_usb_image.py --slots --update`, its `update.txt` naming `data/link/rel`, with `link.conf` as `--link` writes it;
     - the volume is then found by the build's MANIFEST at its root or in `MIND/A`, here and in `disk_copy.sh`, which copies the proprietary files and pictures;
     - checked by building such an image here: `update.txt` at the root, the build in `MIND/A`, whose MANIFEST is `usb_root/MANIFEST`.
2. **`docs/write-disk.md`, section 5,** English and Russian: what it does, in a few lines.
3. **On the maintainer's machine:** `gcu_uf_fast-test/build-and-write.sh` becomes a symbolic link to `gcu_undocumented_func/scripts/build_and_write.sh`, in the clone it builds. Each run pulls `fast-test`, so the script's changes reach it from there.

## Acceptance criteria

- [x] Through the link, `--list` lists the USB disks and leaves no copy behind; a copy variable from outside is refused (2026-10-21, on the maintainer's machine, `gcu_uf_fast-test/build-and-write.sh` → the clone at `fast-test` 0eb5f32).
- [x] A write of the MacBook Pro's disk through the link, with `--link-dev --keep-wifi`, as before: on 2026-10-21 with `--slots` too, `fast-test` b559bdf. The Mac started slot A, `link` in developer mode, joined CoolNet2 from the kept network, and the updater waited for `data/link/rel/`.
## Progress

- **2026-10-23, `--mode=<modes>`, at the maintainer's request:** "a flag that makes `data\mode.conf` with one line, `maintenance`".
  - It puts `data/mode.conf` on the volume holding this build, through the same step as `--wifitest` (written, flushed, read back).
  - It takes `development`, `maintenance` or both, comma-separated; other words are refused before anything is built. They are the two words `init` reads (750-KRN-0143).
  - Tried without a disk: `--mode=maintenance` gives the line `maintenance`; `--mode=development,maintenance,development` gives `development maintenance`; `--mode=bogus` and `--mode=` stop with the usage.
  - Docs: `docs/write-disk.md`, section 5, English and Russian.

## Related

[800-LNK-0012](../issues-done/800-LNK-0012-the-developer-mode.done) (`--link-dev`), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (`--wifijoin`, `--keep-wifi`), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) (`--wifi-ofdm`), [docs/write-disk.md](../docs/write-disk.md).
