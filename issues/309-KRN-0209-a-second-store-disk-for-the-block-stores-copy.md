# 309-KRN-0209 — A second store disk for the block store's copy

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.4, Appendix B.3, B.6

**Asked by:** STO, 2026-11-04 (`requests-KRN.md`, taken out of it with this issue), for [309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md).

## Problem

- **The store has one medium.** `init` gives `blockstore` one disk (`store_disk`, 300-KRN-0025): the first VirtIO disk that is blank or starts with `MIND-STO`.
- **So nothing it holds has a second copy.** A damaged block is found but not repaired, a lost disk loses the store, and `copied` (`blockstore.wit` 2.0, MC-4.4, Appendix B.3) is never reached.
- **309-STO-0040 makes a second store on a second medium,** holding copies of the names chosen for it. That medium is `init`'s to give.

## Plan

1. **In QEMU, the copy's disk** is the next VirtIO disk after the store's own that is blank or starts with `MIND-STO`.
   - `init` looks at the disks as `store_disk` does, in the same order (`virtio_blk`, `virtio_blk#1`, `virtio_blk#2`), and in the same pass.
   - On aarch64 `virtio_blk` is the boot disk, so the store is `virtio_blk#1` and its copy `virtio_blk#2`.
   - A copy is given only when the store has a disk of its own, never beside `ramdisk#1`.
2. **`blockstore` gets it in slot 4,** a block client with `BADGE_WRITE`, as slot 2 holds the store's own. Without a second disk, slot 4 stays empty and the store runs as now.
3. **`vfs_server` never gets it,** as it never gets the store's disk (Appendix B.6).
4. **`init` logs it:** `[INIT] THE BLOCK STORE'S COPY: <driver> (BLANK|THE STORE'S)`, as it logs the store's disk.
5. **On real machines no copy is given** until [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md)'s partitions. That is for a later request.

`blockstore`'s side (the copy mounted from slot 4, names copied, repair) is the storage track's, in 309-STO-0040. No ABI change: slot 4 is between `init` and `blockstore`, as slots 2 and 3 are.

## Acceptance criteria

- **QEMU, x86 and aarch64,** with two blank VirtIO disks beside the boot disk:
  - `init` logs the store's disk and its copy;
  - `blockstore` holds the copy's client in slot 4, and `vfs_server` holds neither disk;
  - after a reboot, both are found again. The copy's is found as `THE STORE'S` once `blockstore` writes it (309-STO-0040); until then, as `BLANK`.
- **The `store` suite on one disk** passes as before.

## Progress

2026-11-04:
- **`init`** (`store_disks`) finds the store's disk and the copy's in one pass, and logs each.
  - `blockstore` gets the copy's write client in slot 4 when its own disk is not `ramdisk#1`.
  - `vfs_server` skips both drivers.
- **QEMU check:** `store_copy_disk_check`, run at the start of the `store` suite on x86 and aarch64. Its machine has two blank VirtIO disks beside the boot disk, and it checks:
  - both of `init`'s lines;
  - `blockstore`'s slots 2 and 4, each a client of the right driver badged `BADGE_WRITE`, from `stat caps` and `endpoints`;
  - that `vfs_server` holds no client of either driver;
  - after an object is published and the machine reboots: the store's disk as `THE STORE'S` with its name, and the copy's again on the same driver.
- **Docs:** [bootstrap.md](../docs/profile/bootstrap.md)'s `blockstore` row, and a row in [evidence.md](../docs/profile/evidence.md).

## Related

STO's [309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md); [300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done) (the store's own disk); [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md) (partitions on machines).
