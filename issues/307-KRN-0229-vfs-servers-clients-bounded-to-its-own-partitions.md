# 307-KRN-0229 — `vfs_server`'s clients of the store's disk bounded to its own partitions

**Type:** `init` (grants), `vfs_server` (mounts) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (made on a branch, ABI 32, 2026-11-10; QEMU runs left) · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.3

Split from [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), plan step 3's second half: "`vfs_server` gets none of that partition".

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

## Progress (2026-11-10)

**Found: the plan's order cannot hold.**
- **What `init` knows when.** `init` reads `store.txt` through `vfs_server` (`read_boot_file`). So it knows the store's partition only after `vfs_server` holds its disks.
- **So plan step 1 cannot be kept as written.** "The disk that holds the store's partition" is not known when `vfs_server` gets its clients.

**What is made instead** (branch `claude/307-KRN-0229-vfs-server-gets-windows`, on 308-KRN-0205's, ABI 32):
- **Every disk with a partition table is given as windows, the store's or not.** `init` reads the table through a client of its own (`vfs_windows`). It sets a window for each partition `vfs_server` may mount, from window 2 of the disk's driver up:
  - an MBR entry of a FAT type (`partition::MBR_FAT`, the types `fat_starts` reads);
  - the GPT partition at the start the firmware booted from.
- **The slots.** `vfs_server` gets the windows' write-badged clients in slots 16 to 21 (`SLOT_VFS_PARTS_FIRST`, `VFS_PARTS` 6; ABI 32).
- **Its arguments say where each window lies:** `parts`, then, for each disk, its MBR signature in hex (0 on GPT) and its windows' first sectors, `S:F,F`.
- **What stays whole.** A disk without a table (blank, or one FAT volume over the whole disk), or one that did not answer `init`, is still given whole, in slots 2 to 4.
- **What the store may not take.** The store takes no partition of a disk `vfs_server` holds whole, and none whose sectors one of `vfs_server`'s windows covers; `vfs_windows` sets no window over the store's partition.
- **`vfs_server`.** It mounts a window at its sector 0 and names it by where it lies on its disk: the boot volume's MBR signature and start, `log:` as another window of the same disk, the `AT LBA` of its lines.
  - Nothing else read the table: the update zone and the boot records are files on the boot volume.
- **After a driver starts again,** `init` sets each of `vfs_server`'s windows on it again while the partition is still there with the same number, PARTUUID and sectors (`vfs_windows_again`), as it does the store's. A returned medium (000-KRN-0085) ends its driver, so the same check covers it.

**Tested so far:**
- host tests: `partition_host.rs` (12, the FAT types and the overlap among them), `runtime.rs` (46), `boot_abi_host.rs` (9), `block_host.rs` (8);
- `init` and `vfs_server` built with no warning.

**The criterion "a write by `vfs_server` past a window's end is refused":**
- **What shows it:** the drivers' common loop, tested on the host (`block_host.rs`, `a_windows_client_sees_its_window_as_the_device`: `Invalid` past the end).
- **What does not show it yet:** a QEMU run in which `vfs_server` itself writes there. A FAT volume whose boot sector claims more sectors than its partition would; `Volume::mount_at` does not check its size against the device's.

## Related

[307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done), [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md).
