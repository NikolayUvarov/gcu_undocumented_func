# 211-KRN-0223 — The system log saves less often, with fewer flushes (a USB boot disk accessed all the time)

**Type:** `vfs_server` (the journal) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open (in `main` since round 99, c92256a3; the MacBook Pro's run, DRV's, left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track H, stage I · **Constitution:** MC-5.1, MC-10.6

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

- **The cadences:**
  - in the boot's first two minutes, every record within `SAVE_MS`, as now: the log of a boot that hangs still reaches its last seconds (docs/write-disk.md);
  - after them, records below a warning every 30 s, or once 256 wait (logd's ring holds 2048);
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

## Progress (2026-11-09)

- **`vfs_server/src/cadence.rs`** (pure): when to save. `BOOT_MS` 120 s, `SAVE_MS` 2 s, `ORDINARY_MS` 30 s, `MANY` 256, each record looked at once (its sequence number kept).
- **`journal.rs`:**
  - every `SAVE_MS` the journal reads logd's records not yet looked at, without the disk (`wanted`), and saves only when the cadence says;
  - a save writes the records and flushes the medium once (`fat::Volume::flush_open`), leaving the volume marked dirty, so that the next save needs no new mark.
- **`main.rs`:** a `flush` request on any volume saves the journal first and marks the log volume clean. That is the shell's `sync`, and `init`'s flush of every volume before a restart or power-off.
- **What a save costs now,** after the first: the data, the directory entry, the FAT when a cluster is added, and one flush, where three flushes and two writes of FAT[1] were.
- **`tests/log_cadence_host.rs`** (in `scripts/host_tests.sh`):
  - the cadence: the boot's two minutes, a warning at once, ordinary records waiting, many saved, a record looked at once;
  - on a FAT16 volume made by `mkfs.fat`: 20 saves, the first two flushes (the dirty mark's and its own), each later one flush; `fsck.fat` finds the dirty bit between saves and the volume clean after `flush`, every byte kept.
- **`tests/usb_image_smoke.py`:** `sync` after `logger` and after the debug-mode run, where it waited for the 2 s save; `sync` at its end, before the image is read on the host and `fsck.fat` checks it clean.
- **docs:** `docs/profile/evidence.md`, `docs/write-disk.md` and its Russian copy state the bounds.
- **Passed:** `x86: USB image` (`tests/usb_image_smoke.py`) twice in the kernel session's container (QEMU 8.2.2, OVMF, TCG): the log line after `sync`, the debug mode's line, the records of the time without the disk after the replug, and the image read on the host with `fsck.fat` finding the log volume clean.
  - **One earlier run failed before any of this:** after the first `device_del`, `usb_host` logged no `DISCONNECTED`. The two runs after it passed, on the same tree. Recorded, not explained (MC-12.2).

**Landed** in round 99 (`main` c92256a3, 2026-11-09): every group of the gate passed with it, the `boot` suite's log check and `x86: USB image` among them. **Left:** the MacBook Pro's run over the link (DRV): the disk quiet between saves.

## Related

[211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done) (the journal), 211-KRN-0050 (a drive that is gone), 211-DRV-0053.
