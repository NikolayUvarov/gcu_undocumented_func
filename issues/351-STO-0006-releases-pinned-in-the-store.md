# 351-STO-0006 — Releases as objects in the block store, the running and last-known-good ones pinned

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** in progress (2026-11-08) · **Blocked by:** `init`'s store client for the updater ([requests-KRN.md](requests-KRN.md)); the updater's part ([requests-UPD.md](requests-UPD.md)) · the durable medium is done ([300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done): a blank or store VirtIO disk is the store's own); [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) (the updater, done) · **Main task:** [351](351-self-update.md) · **Roadmap:** track B; track C · **Constitution:** MC-9.3, MC-4.5, MC-4.11

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

### The parts, as worked out on 2026-11-08

What the store needs is there already:
- pins that only their owner ends, and collection that never frees what a pin retains (303-STO-0002);
- objects of files and directories (`mind::dag`, `mind::tree`, 311-STO-0027);
- a working tree that writes a directory out as a root (`mind::worktree`, 311-STO-0029);
- reading back, with every block checked against its CID (`dag::each_chunk`, `tree::diff`).

So the work splits three ways:

1. **`STO`, the store's owner for the updater** (this task). The badge's bits 4 to 7 name an owner apart from its rights, and `mind::blockstore::OWNER_UPDATER` (`0x10`) is the updater's.
   - Its pins and its account are its own: no other client ends them, whatever rights it holds.
   - The host tests show a release tree, pinned by that owner, kept whole through the removal of every name and a collection, and read back with each block checked.
   - The quota stays three quarters of the medium for every owner. That holds two releases once the store's medium is at least three times a release (about 26 MiB on x86 now), and less, since the blocks two releases share are stored once. A quota of its own for the updater, and a bound on all owners together, stay open (303-STO-0002's "not done").
2. **`KRN`, `init`'s grant** ([requests-KRN.md](requests-KRN.md)):
   - the updater gets a store client badged `OWNER_UPDATER` with get, put and publish, when the store runs;
   - it starts after `blockstore`.
3. **`UPD`, the updater** ([requests-UPD.md](requests-UPD.md); `STO` offers to make it, as `STO` made 351-UPD-0007):
   - **Each staged release** goes into the store as a `mind::tree` directory of its files, once its files have passed their checks. It is named `release/<version>` (for people; names are anyone's to change), and pinned.
   - **After a confirmed boot** the pins of the running and the last-known-good release stay, and older ones end (351-KRN-0014). The updater finds its releases by its own pins (`pins`) and the manifest each holds, never by a name.
   - **A slot is rebuilt from the store** as from any other source: every file is checked by size and SHA-256 against the manifest, and the manifest against the boot key. So a name changed by another client, or a damaged medium, can stage nothing the signatures do not allow. A new minor version of `update.wit` asks for it.
   - **QEMU, the `update` suite on x86 and aarch64,** with a store disk:
     - after a confirmed update and `blocks collect`, both pinned releases are complete;
     - a slot damaged from the host is rebuilt from the store and boots.

## Acceptance criteria

After collection runs, the last-known-good release is still complete in the store, and a damaged slot is rebuilt from it (QEMU, x86 and aarch64).

## Related

[351](351-self-update.md), [303](../issues-done/303-retention-and-collection.done), [docs/storage](../docs/storage/README.md).
