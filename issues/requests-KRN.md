# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file, and the file goes when it is empty.

## Kernel structures outside the 64 MiB arena

### Problem

Issue 171's resolution names one limit that is neither the hardware nor an encoding: the kernel's 64 MiB arena holds every task's context, mailbox, info and exit pages, its page tables and its capability table, and none of it is charged to a quota. On a machine with gigabytes of RAM the arena, not the RAM, ends up bounding how many tasks run and how many capabilities they hold, and one spawner can fill it for everyone.

### Plan (a proposal; the kernel track decides)

- Take each task's structure and pages, its page tables and its capability table from the frame pool, as images, stacks and screens are (issue 168), and charge them to the spawner's memory quota.
- Leave in the arena only what is global and small: the task and endpoint tables' slots, DMA regions.
- `StatTask.kernel_bytes` and `STAT_MEMORY` follow; `kernel-objects.md` moves these rows from "kernel heap" to "frame pool, charged".

Before the kernel track took 171, the tools branch had a version of the first point (commit `7b7c895`, `Boxed<T>` in `kernel/src/memory.rs`; it lost to 171-KRN-0002 in the merge). It may help as a sketch.

### Acceptance criteria

- Spawning until memory runs out stops at the frame pool, not at the arena, in a machine of 512 MiB and of 6 GiB (QEMU).
- A spawner's memory quota bounds the kernel memory its children take.

## A model disk for `vfs_server`, next to the store's disk

**Recorded by:** the tools track (APP), 2026-10-08, for main task [251](251-model-cache-and-model-disk.md) at the maintainer's request.

### Problem

Speech models (25 MB to 3 GB) come on a disk of their own: a FAT32 volume labelled `MIND MODELS`, made by `scripts/models.py disk` (251). `vfs_server` will mount it as `models:` and read-only, whatever the device allows. The tools track does that part.

On x86 the boot disk is on `ata`, so a model disk on `virtio_blk` already reaches `vfs_server`. On aarch64 the boot disk is itself on `virtio_blk`, so the model disk is a second VirtIO block device, as the store's durable disk in the request above is. `init` then has to tell the two apart and send each to its service.

### Plan (a proposal; the kernel track decides)

- The drivers track's side: `virtio_blk` serves each device as an instance (`virtio_blk#1`, as `virtio_net#1`). Each instance reads its device's serial with `VIRTIO_BLK_T_GET_ID`.
  - The drivers track has no owner, so the tools track does this as `251-DRV-0003` if no one has taken it first (AGENTS.md, section 5).
- The QEMU harness names the role in the serial: `-device virtio-blk-pci,serial=MIND-MODELS` and `serial=MIND-STORE`.
- `init` routes by serial:
  - `MIND-STORE` gives its write client to `blockstore`, never to `vfs_server` (the request above);
  - `MIND-MODELS` gives a read client to `vfs_server`;
  - any other disk is handled as today.

### Acceptance criteria

On aarch64 and on x86 (QEMU), a model disk and a store disk attached together each reach their own service, and the boot disk still mounts as before.
