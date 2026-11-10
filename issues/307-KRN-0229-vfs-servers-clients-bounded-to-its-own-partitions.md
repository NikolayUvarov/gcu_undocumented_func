# 307-KRN-0229 — `vfs_server`'s clients of the store's disk bounded to its own partitions

**Type:** `init` (grants), `vfs_server` (mounts) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.3

Split from [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md), plan step 3's second half: "`vfs_server` gets none of that partition".

## Problem

Since 307-KRN-0177:
- `blockstore` reaches its own partition through window 1 of the disk's driver, and nothing else.
- `vfs_server` still gets that disk whole, write-badged, for its FAT partitions on the same stick (the boot volume and `log:`).
- So the store's sectors are inside `vfs_server`'s reach. Only its own code keeps it out of them, not its capability (MC-3.1).

## Plan

1. **Windows for `vfs_server`.** On the disk that holds the store's partition, `init` sets a window for each other partition `vfs_server` mounts (the FAT types it reads, at most 6 beside the store's) and gives `vfs_server` those windows' clients in place of the whole disk.
2. **`vfs_server` mounts a window as a volume over the whole device.** It already does that for a FAT volume without a table (`fat_starts`).
   - What it reads from the table now (the boot volume's partition number, the disk signature, `log:` as the partition after it, the update zone) comes from `init` with each client instead.
3. **The windows are set again after a restart,** and dropped after a return (000-KRN-0085), as the store's are.

## Acceptance criteria

- **QEMU, `tests/usb_image_smoke.py`, x86 and aarch64:**
  - `vfs_server` holds no client of the stick's whole disk;
  - the boot volume, `log:` and the update zone work as before;
  - a write by `vfs_server` past a window's end is refused.
- **`docs/profile/tcb.md`'s row for disk integrity** names `vfs_server` as outside the store's sectors.

## Related

[307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md), [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md), [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md).
