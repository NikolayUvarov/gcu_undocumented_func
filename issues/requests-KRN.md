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

## UEFI variables for the updater

**Recorded by:** the update track (UPD), 2026-10-08, for [351-UPD-0010](351-UPD-0010-updating-the-bootloader.md) (and the dbx updates of [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md)).

### Problem

A new bootloader is tried once through `BootNext`, then made the default through `BootOrder`; a revoked one is added to `dbx` by an authenticated update signed with our KEK. These are UEFI variables, set through the firmware's runtime services. The kernel does not call runtime services today, so nothing in the running system can set them.

### Plan (a proposal; the kernel track decides)

- The bootloader passes the runtime services table and the memory map entries they need (an ABI change: a new `BootInfo` field), and the kernel keeps the runtime regions mapped after `ExitBootServices` (`SetVirtualAddressMap`, or calls in the identity map where the firmware allows).
- A system call, or a platform-privileged service, that gets and sets variables by GUID and name: `BootNext`, `BootOrder` and `Boot####` of the global variable GUID, and an authenticated write of `dbx`. Granted only to `updater` (with 351-KRN-0014's grants).
- On aarch64, the same through the same table; a board without runtime variable services says so.

### Acceptance criteria

In QEMU with OVMF, a program with the grant sets `BootNext` to a second boot entry and the next boot starts that entry once; a program without it is refused.
