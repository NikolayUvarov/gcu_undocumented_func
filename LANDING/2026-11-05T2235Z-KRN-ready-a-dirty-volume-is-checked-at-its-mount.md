# Ready: a FAT volume left dirty is checked at its mount, and one found damaged is not written (000-KRN-0215)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 22:35 UTC · **Concerns:** 000-KRN-0215 (closes once landed), DRV's report on the Ryzen's stick

**Ready:** `claude/000-KRN-0215-dirty-volume-check` ca005a004a20b38957206273845c478443c02fd0

It is `main` 16caabd3 (round 87 and its text) and four commits.

## What it brings

- **`vfs_server`:** a boot volume or log partition on a writable device whose clean bit is clear is checked (`check`, read only) before anything is written to it. Damage a write could spread holds the volume, and every write then answers `read-only`:
  - a cross-link;
  - a chain into a free or invalid cluster;
  - an invalid entry.

  Lost clusters alone hold nothing. The log has three lines: `WAS LEFT DIRTY: CHECKING`, the counts with the first problem, then held or writable. A clean volume is not checked, and a boot still writes nothing.
- **Tests:**
  - `tests/fat_host.rs`: three new tests, on FAT16 and FAT32;
  - `dirty_volume_check` in the `vfs` suite: a boot disk with a remove cut short, held, nothing written but what OVMF writes on x86 (its `NvVars`).
- **Docs:** the threat model's power-loss row and an evidence row.
- No change to an interface or the ABI.

**For every suite:** a VM reset with changes not yet flushed now boots with three more `[VFS]` lines. It stays writable unless the check finds damage.

## Tested, through Effector

- **x32-128, x86,** on f73a003e: `files and block writes` (899 s), `NVMe boot disk` (71 s). `host tests` on 9267d8bb (1188 s); the later commits change `qemu_smoke.py` and the issue only.
- **acc-2, aarch64,** on f73a003e: `files and the block store` (786 s), `NVMe boot disk` (92 s).
- The last commit, ca005a004a20b38957206273845c478443c02fd0, records these runs in the issue.
