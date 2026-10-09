# 175-KRN-0049 — A FAT volume reads clean only after a flush that succeeded

**Type:** kernel (`vfs_server`) · **Owner:** `KRN` · **Priority:** P2 (the profile row narrowed now) · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) · **Constitution:** MC-4.8, MC-4.9, MC-12.3

## Problem

Audit finding A04 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)). The volume can read clean after unflushed or failed writes, in two ways and in the wrong order.
- **A stale copy overwrites the dirty bit.** `set_fat_bytes` reads a FAT sector, then `write_sector` calls `changing()`, which writes the dirty bit into that same sector. The stale copy then overwrites it. This happens for the first FAT update after a flush in sector 0 of the first FAT (FAT16 clusters below 256, FAT32 below 128).
- **The clean bit comes before the flush succeeds.** `flush()` sets it and clears `changed` before it knows whether `disk.flush()` succeeded.
- **The order.** `disk.rs` writes back in LBA order, so FAT sector 0 with the clean bit reaches the medium before the data.

## Plan

- **Dirty first.** The dirty state reaches the medium, flushed by itself, before the first change of a dirty period.
- **No stale overwrite.** The FAT sector that holds the bit is re-read after `changing()`, or the bit is applied in the cache.
- **Clean last.** The clean bit is written only after a flush of everything else has succeeded, then flushed itself.
- **On an error.** The volume stays dirty.
- **Tests.** Fault injection at each barrier, then a remount: an incomplete write never reads as a clean shutdown.

## Acceptance criteria

- The tests above pass in `tests/fat_host.rs`.
- The profile's FAT row states the dirty-until-flush guarantee again, with this test as its evidence.

## Progress

**2026-10-09:** the profile row is narrowed now (`docs/profile/evidence.md`, MC-12.3): the volume is not claimed to read dirty after every change until this task is done.

## Related

`vfs_server/src/fat.rs`, `vfs_server/src/disk.rs`, `docs/profile/evidence.md`.
