# 351-STO-0006 — Releases as objects in the block store, the running and last-known-good ones pinned

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** — the durable medium is done ([300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done): a blank or store VirtIO disk is the store's own); [351-UPD-0007](351-UPD-0007-updater-service.md) (the updater) · **Main task:** [351](351-self-update.md) · **Roadmap:** track B; track C · **Constitution:** MC-9.3, MC-4.5, MC-4.11

Numbered by the storage track from the kernel track's request in `requests-STO.md` (recorded 2026-10-08 for main task 351, phase 4, at the maintainer's request).

## Problem

Self-update stages a release in slot B of the boot volume ([351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done)). MC-9.3 asks that recovery images and the objects they need be protected from ordinary cleanup. The block store has what that needs: objects named by their root, pins that only their owner ends, and collection that never frees what a pin retains ([303-STO-0002](../issues-done/303-STO-0002-pins-and-quotas.done)). But it lives on a RAM disk, so nothing it holds outlives a reset.

## Plan

The storage track's part; the updater's part is 351-UPD-0007.

- **The store on a durable medium.** `blockstore` mounts a disk of its own instead of `ramdisk#1`; the layout already works over any block device. This needs the durable block path of track A (a request to `DRV` and `KRN` when it starts).
- **Releases as objects.** The updater puts each release as a DAG object of its blobs (`mind::dag`), and names it (`release/<version>`).
- **Recovery roots.** The running and the last-known-good releases are pinned by the updater's badge. Collection never frees a pinned object, and only that badge can end its pins. The updater moves the pins only after a new release is confirmed (351-KRN-0014).
- **A quota for the updater** large enough for two releases, granted with its client (the per-owner quotas that 303-STO-0002 left fixed).
- **Rebuilding a slot:** the updater reads the last-known-good object back, every block checked against its CID, and writes the slot.

## Acceptance criteria

After collection runs, the last-known-good release is still complete in the store, and a damaged slot is rebuilt from it (QEMU, x86 and aarch64).

## Related

[351](351-self-update.md), [303](../issues-done/303-retention-and-collection.done), [docs/storage](../docs/storage/README.md).
