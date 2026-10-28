# Ready again: several machines on one line, firmware over the cable, and an Intel Mac's startup-disk script

**From:** LNK · **To:** LANDING · **Written:** 2026-10-28 05:26 UTC · **Concerns:** 800-LNK-0026, 800-LNK-0027, 800-LNK-0029 (new), 800-LNK-0028 (issue only) · **Replaces:** `LANDING/2026-10-28T0345Z-LNK-ready-several-machines-and-firmware-over-the-cable.md` (1fabf12f, not answered; this branch holds it)

**Ready:** `claude/LNK-link` 28efcc614e8993ad34b4960e536647b83e12650e

## What it brings

- **Everything of the 0345Z request:**
  - 800-LNK-0026, several machines on one RS-485 line;
  - `build_and_write.sh` going on with a disk that holds no MIND CORE volume;
  - 800-LNK-0027, `data/firmware/` as `firmware/` in developer mode, and `proprietary.sh link`.

  KRN's 800-KRN-0173, which 800-LNK-0027 needs, has landed (c8fe8d03), so nothing waits on another request now.
- **800-LNK-0029, the maintainer's word (2026-10-28):**
  - `scripts/mac/startup-disk.sh` (POSIX sh). In macOS Recovery, after `mkdir` and `mount` of the boot partition, it blesses its own volume's `EFI/BOOT/BOOTX64.EFI` and checks `bless --getBoot`;
  - `build_and_write.sh` copies it onto the disk as `mac/startup-disk.sh` after the write, through `disk_copy.sh`;
  - `tests/mac_startup_disk_test.py` is added to `scripts/host_tests.sh`;
  - `docs/write-disk.md` is updated in both languages.
- **800-LNK-0028:** its issue file only, waiting on APP and KRN.

No ABI change. No IDL change.

## `main` in it

8bc1dfa9 (merged 2026-10-28; a text-only conflict in `issues/README.md`, both sides kept).

## What was run on the branch

- **On this tip:**
  - `tests/mac_startup_disk_test.py`: 6 passed, each under `sh` (dash) and `bash`;
  - `link_names_host`: 4 passed;
  - the `tools/mindlink` cargo tests passed;
  - `bash -n scripts/build_and_write.sh` passed.
- **On 1fabf12f (2026-10-27), as that request says:**
  - `./02_build.sh`;
  - the `link` and `linkupdate` QEMU suites;
  - on the DEXP: a release over the cable and firmware put into `firmware/rtl_bt/`.

  Since then the branch changed only by `main`, the new script, its test and text.
- **Not run:** the script on a Mac. That is 800-LNK-0029's open criterion, and it waits for the maintainer in Recovery.
