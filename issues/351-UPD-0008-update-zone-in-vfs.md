# 351-UPD-0008 — An update zone in `vfs_server`

**Type:** update (storage service) · **Owner:** `UPD` track; `vfs_server` has no owner in TRACKS.md · **Priority:** P1 · **Status:** open · **Blocked by:** [351-UPD-0006](351-UPD-0006-slots-and-boot-records.md) (the layout) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.2, MC-9.3

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner.

## Problem

`vfs_server` lets its clients write only to `data/` and `ram:`; boot files are never writable. That is right, and an updater must not get more than it needs.

## Plan

- **A new badge, granted only to `updater`.** It may write only the inactive slot's directory and the boot records. It may not touch the running slot, `EFI/` or anything else.
- **Writes to a boot record are one sector in place**, then a flush. This is the write 351-UPD-0006's records depend on.
- **The inactive slot is cleared and filled file by file**, with a flush before the record that points at it.
- **`idl/vfs.wit` gets a new minor version** if a call is added (MC-12.4).
- **Tests:**
  - the zone's limits: writes outside it refused, the running slot refused;
  - the record write.

## Acceptance criteria

A client with the update badge can fill the inactive slot and write a boot record, and nothing else. The shell's badge is unchanged.

## Related

[351](351-self-update.md), [351-UPD-0007](351-UPD-0007-updater-service.md).
