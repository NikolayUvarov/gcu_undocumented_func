# 211-KRN-0223 — The system log saves less often, with fewer flushes (a USB boot disk accessed all the time)

**Type:** `vfs_server` (the journal) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track H, stage I · **Constitution:** MC-5.1, MC-10.6

**Asked by** DRV, 2026-11-09 (`KRN/2026-11-09T0429Z-DRV-the-system-log-writes-the-boot-disk-every-11-s.md` on `internal-dev-messages`), after the maintainer saw the MacBook Pro's USB boot disk accessed all the time.

## Problem

- **`vfs_server`'s journal** (`vfs_server/src/journal.rs`, 211-KRN-0019) saves at most every `SAVE_MS`, 2 s, whenever `logd` has new records.
- **On the MacBook Pro** (DRV's measure): records came in 134 two-second windows in 1480 s, so a save every 11 s.
- **A save cost** about 110 system calls in `usb_storage` and 90 in `vfs_server`, roughly 25 SCSI commands:
  - the file's lookup in the root directory;
  - its data, its directory entry, and the FAT when a cluster is added;
  - three flushes (SYNCHRONIZE CACHE): the dirty mark's, the write's and the clean mark's;
  - the FAT's sector 0 written twice, dirty and then clean.
- **On a flash stick,** that is two rewrites of one FAT sector each save.
- **DRV's part** (`bcm_wifi`'s 10-second tally) is reduced in 211-DRV-0053; a record still came about every 90 s from the updater and the agent.

## Plan

- **Two cadences:**
  - records below a warning saved every 30 s;
  - warnings and errors within `SAVE_MS`, as now;
  - everything at `sync`, a reboot and a shutdown.
- **Fewer flushes a save:** the log volume stays marked dirty while the journal holds it open, and is marked clean at shutdown and at `sync`. A save then writes the data and the directory entry, and flushes once.
- **What a power loss may cost, said in the profile** (MC-10.6): at most the last 30 s of ordinary records, and nothing at or above a warning older than `SAVE_MS`. A volume left dirty is checked at the next mount as now.

## Acceptance criteria

- **A test** counts the journal's writes and flushes over a quiet minute with ordinary records: at most two saves, one flush each.
- **A warning reaches the disk within `SAVE_MS`** (the `boot` suite's log check).
- **A shutdown leaves the volume clean;** a reset leaves it dirty, and the next mount checks it and keeps the records saved.
- **docs/profile** states the loss bound.
- **On the MacBook Pro** (DRV, over the link): the disk is quiet between saves.

## Related

[211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done) (the journal), 211-KRN-0050 (a drive that is gone), 211-DRV-0053.
