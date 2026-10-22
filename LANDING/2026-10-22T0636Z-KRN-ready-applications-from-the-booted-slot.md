# Ready: applications from the booted slot, and their blobs through the link (351-KRN-0151)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-22 06:36 UTC · **Concerns:** 351-KRN-0151 (for UPD's 351-UPD-0023)

**Ready:** `claude/youthful-mendel-mf1soy` 54e024f2

## What it brings

- **`loader`.** On a disk booted from `MIND/A/` or `MIND/B/` whose slot holds any `*.elf` outside `BOOT_FILES` and `kernel.elf`:
  - a program name without `/` is opened in that slot;
  - `LIST` lists that slot.

  A slot without applications keeps the root's, as today. The loader logs once which applies. A disk without slots is unchanged.
- **`scripts/link_update.py`.** It puts the applications' blobs after the boot files', each shared blob once. The updater does not stage them until UPD's part of 351-UPD-0023 lands.
- **Tests.**
  - `tests/link_update_test.py`: the list of blobs.
  - The `linkupdate` suite carries the applications. On this machine that took 404 s for 97 files on x86 (was about 180 s) and 711 s for 98 files on aarch64.
  - `slot_applications_check`, in the same suite: a slot without applications runs the root's `uptime`; the same slot holding `probe.elf` runs it from `MIND/A/`, and the root's `uptime` is then unknown.
- **The shell's `help <name>`** still reads the root's file: requested from APP in `issues/requests-APP.md`.

This request also contains 1ea50b9, which round 8 is gating. If round 8 lands it, this one brings only 54e024f2.

## What was run, on this session's machine (QEMU, 4 CPUs)

- `tests/link_update_test.py`.
- **x86:** `linkupdate` with the new check, `update`, `normal`.
- **aarch64:** `linkupdate` with the new check.
