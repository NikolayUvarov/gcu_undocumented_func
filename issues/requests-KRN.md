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

## The busy suite's share check on one CPU depends on the host

### Problem

`busy_suite` checks that without a budget the busy loop gets more than 0.6 of its one CPU, measured against the host's wall clock over 3 s. Under TCG a host that deschedules the emulated CPU takes that time from the loop. In a full local run (`--cpu-model max --cpus 1`, nothing else running in the session) it measured 0.52 once and failed; a rerun passed. Eight runs each on `main` (ed08bed) and on the tools branch (909be4f) gave the same spread: 0.62–0.73, mean 0.69. So the margin above 0.6 is small and host noise can cross it.

On 2026-10-07 the same host was slower, and the check failed in two full local runs in a row (0.584, 0.572); GitHub CI passed it on the same commits. Thirteen more runs (`--cpu-model max --cpus 1 --suites busy`, nothing else running) gave 0.554–0.648. Four of them on images of the tools branch before a merge of `main` (bbb113c), alternating with four after it (21c2738), gave the same values: 0.648, 0.593, 0.588, 0.616 before and 0.637, 0.585, 0.590, 0.585 after. So the code did not move the share; the host's wall clock did.

### Plan

The kernel track decides: measure the loop's share against the guest's own time (the CPU's busy plus idle time from `STAT_CPUS` over the same interval) rather than the host's wall clock, or another way that keeps the check as strict.

### Acceptance criteria

The check keeps its meaning (no budget: the loop takes most of its CPU) and does not fail when the host is busy.

## A block store client with fewer rights

**Recorded by:** the storage track (STO), 2026-10-07.

### Problem

The shell holds the only `blockstore` client (slot 25, badge 7: get, put and publish), and lends that client for `REQUEST_BLOCKSTORE`. A badge is set once and children keep it, so neither the shell nor a program can narrow it.

So no program can hold a client that may only read. The service's refusals by badge (300-STO-0004, `mind::blockstore::allowed`) are therefore tested on the host only.

### Plan (a proposal; the kernel track decides)

- `init` also gives the shell a client badged `BADGE_GET` alone, in a slot of its own.
- The shell lends that client for a request that asks only to read, for example a new `REQUEST_BLOCKSTORE_READ`.

### Acceptance criteria

- A program that asked only to read holds a client with badge 1.
- The storage track's `store` suite then checks that `put` and `publish` are refused with `rights`, and that the refusals are logged.

## A durable disk for the block store

**Recorded by:** the storage track (STO), 2026-10-08, for [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md) and track B's "durable block path from track A".

### Problem

`init` starts `blockstore` over `ramdisk#1` (300-KRN-0001), so nothing the store holds outlives a reset. Its layout works over any block device, and its damage, collection and recovery are tested ([300-STO-0005](../issues-done/300-STO-0005-corruption-on-the-platform.done), [305](../issues-done/305-recovery-without-the-store.done)). But no disk is set aside for it: `vfs_server` gets a write client of every running block driver, and the store must not share a medium with a file system (Appendix B.6).

### Plan (a proposal; the kernel track decides, with `DRV` for the driver side)

- A disk of the store's own: for example a second VirtIO block device (`virtio_blk#1`, as `virtio_net#1`), or a GPT partition with a type GUID of its own on the boot disk.
- `init` gives `blockstore` the write client of that device instead of `ramdisk#1` when it exists, and does not give it to `vfs_server`. Without one, it keeps `ramdisk#1`.
- The QEMU harness attaches a blank image for it. The storage track then adds a suite that reboots the machine and finds the store whole.

### Acceptance criteria

- With the extra disk, `[BLOCKSTORE] READY` names a medium that survives a reboot of the VM; without it, the RAM disk as today.
- `vfs_server` holds no client of the store's disk.

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
