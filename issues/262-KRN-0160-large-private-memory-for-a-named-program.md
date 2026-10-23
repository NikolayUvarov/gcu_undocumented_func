# 262-KRN-0160 — Tens of GiB of private memory for a program the owner names

**Type:** kernel, `init`, `loader` · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.13, MC-5.1, MC-5.5, MC-12.6

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md) ("Memory and CPUs for large models", item 1), for 262's stage 1. Its other items are [262-KRN-0161](262-KRN-0161-large-pages.md) to [262-KRN-0164](262-KRN-0164-stacks-larger-than-64-kib.md).

## Problem

A 7B-parameter model in 4 bits takes about 4 GB, a 70B one about 40 GB, and its KV cache grows with the context. Today a program cannot hold that:

- **The loader's limit.** A program may ask for at most `APP_MEMORY_MAX`, 1024 MiB (`loader/src/main.rs`).
- **The window.** A task's private memory lives in a 928 MiB window (`USER_HEAP` to `USER_END`, `kernel/src/paging.rs`), shared with its mappings of foreign memory.
- **One run per block.** Each heap block is one contiguous run of the frame pool (`Region::task`, first fit). Once the pool is fragmented, a large block is refused although the pool has the bytes.
- **Zeroing stalls the machine.** A block is zeroed under the scheduler's one lock.
- **The quota's unit.** `SPAWN` carries the memory quota in MiB in 16 bits, so at most 64 GiB − 1 MiB (`QUOTA_MAX`).

## Plan

1. **A larger window.** A task's private window grows to the rest of its 512 GiB top-level entry: `USER_IMAGE` is `0x80_0000_0000` on x86-64 and `255 << 39` on aarch64, so both have room.
   - The first GiB keeps its layout: image, stack, screen, information page, mailbox.
   - Page tables are still made on demand and charged to the task's payers (171-KRN-0032).
2. **Large blocks stay contiguous, aligned for large pages.**
   - A memory capability names one physical range (`Capability::Memory`), and sharing, revocation and the orphans' accounting rely on it. Blocks made of several runs would change all of them, so they wait for a measurement that fragmentation refuses large blocks in practice.
   - The frame pool gives task memory from the highest range first, which on a PC is the RAM above 4 GiB in one piece, so a model's blocks taken early fit.
   - A block of 2 MiB or more is taken 2 MiB-aligned, and one of 1 GiB or more 1 GiB-aligned where the pool allows, for [262-KRN-0161](262-KRN-0161-large-pages.md).
   - A large block refused while the pool has the bytes says so in the log (`FRAGMENTED`), so the measurement exists.
3. **Zeroing outside the scheduler's lock.**
   - `ALLOC` zeroes its frames today while it holds the one scheduler lock, with interrupts off: about 50–100 ms a GiB, a stall of every CPU.
   - A block above 64 MiB is taken and charged under the lock, zeroed after the lock is let go, and mapped only then. Its task stays in the system call meanwhile, and nothing else can reach the block.
4. **The quota stays in MiB, up to 64 GiB.** That holds a 70B model in 4 bits. A quota beyond 64 GiB is a later ABI change.
5. **Who may ask for more: the owner's file.**
   - `data/memory.conf`, one line per program: its name and its MiB, for example `nnd 49152`.
   - The loader reads it at each run and gives a named program up to its line, and every other program up to `APP_MEMORY_MAX` as now.
   - `init` gives a boot service named there the same quota.
   - The file is the owner's, written with the disk (MC-12.6); a program cannot raise its own line.
6. **The system keeps its reserve.** What an application takes comes from the frame pool less the reserve that application-band tasks may not take (`MEMORY_RESERVE`, issue 169, MC-5.5). A request beyond what is free is refused at once, never granted and faulted later.
7. **`libmind`'s heap** gives an allocation larger than its arena a block of its own (`libmind/src/heap.rs`), as it does for 16 MiB today.

## Acceptance criteria

- [ ] A QEMU suite with 8 GiB of RAM (x86 and aarch64):
  - a test program named in `data/memory.conf` for 6144 MiB allocates and writes 6 GiB in blocks of 1 GiB, each aligned as stated, and reads each byte back;
  - a program not named is held to 1024 MiB;
  - while it holds its memory, a service restarts (`svc restart`), so the reserve held;
  - its memory goes back to the pool when it ends.
- [ ] Host tests: a large block is aligned as stated; a refused block leaves nothing mapped or charged.
- [ ] While a program allocates 4 GiB, a clock on another CPU keeps its second (no stall of the other CPUs).
- [ ] `docs/profile` states the window, the alignment, the zeroing and the owner's file, and what is not done (blocks of several runs; above 64 GiB).

## Related

[262](262-model-runtime-on-the-machines-full-hardware.md), issue 150 (memory quotas), issue 169 (the reserve), 171-KRN-0032 (kernel structures charged), [750-KRN-0158](750-KRN-0158-the-effector-sessions-memory-for-its-benchmark.md).
