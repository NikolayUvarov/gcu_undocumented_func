# 262-KRN-0163 — Workers: copies of a program on several CPUs, sharing memory objects

**Type:** kernel, ABI 18 (`WORKER_START`), `libmind`, `loader` · **Owner:** kernel session · **Priority:** P1 · **Status:** open (made on `claude/youthful-mendel-mf1soy`; landing left) · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-2.5, MC-3.4, MC-3.13, MC-5.1, MC-12.4

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md), item 4: "several CPUs working on one model: threads in one address space, or worker tasks sharing memory objects. Which one is the kernel track's choice; the runtime follows it."

## Problem

- **A program runs on one CPU.** A task is one thread of control, and a program cannot start another task: `SPAWN` needs the spawn privilege, which only `init` and the loader hold.
- **So one model uses one core** of a Ryzen's 16 or an i9's 24.

## The choice: workers, not threads

**Workers** are tasks of their own, each with its own address space, that share the memory objects their creator lends them. The kernel track chooses them over threads, because:

1. **The kernel's unit stays the task.**
   - A thread would share its address space, capability table and quota with others, while every path of the kernel assumes one task per space:
     - page-table edits under the lock;
     - a flush only on the CPU that runs the space (`paging.rs`: "a Space … belongs to the sole, pinned task");
     - each task's system-call mailbox at one fixed address.
   - Threads would change all three.
2. **A fault stays in one worker.** It ends that worker only, and its creator gets the exit notice.
3. **What a model needs is shared memory, not shared pointers.**
   - Weights, activations and the KV cache are arrays.
   - Workers reach them through objects lent to them, by offsets.

**Threads stay possible later**, for a POSIX layer ([650](650-building-on-the-target.md)), as their own task with their own measurement.

## Plan

1. **`WORKER_START`, ABI 18.** A task starts a copy of its own program:
   - **its image** is the caller's: the same boot image, or the same file the loader gave;
   - **its entry** is the program's own;
   - **its argument word,** and its index, are in its information page;
   - **its capabilities** are only those the caller grants in the call, as `SPAWN`'s grant array: memory objects and endpoints. It never gets the spawn privilege.
   - It needs no privilege: a task may start workers of itself only.
2. **Limits and accounting (MC-3.13, MC-5.1).**
   - At most one worker per online CPU, for each task.
   - Each worker counts against its creator's task quota. Its image, stack and kernel structures are charged to its creator's memory quota.
   - The loader gives a program named in `data/memory.conf` ([262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md)) a task quota of the online CPUs. Other programs get none, as now.
3. **Placement.** A worker goes to the online CPU with the fewest tasks of its band, and not to its creator's CPU or a sibling's while another is free.
4. **Lifetime.**
   - A worker ends when its creator ends, as a child of `init` does when `init` stops it.
   - Its creator watches it (`TASK_WATCH`) and gets its exit notice.
5. **Synchronisation.**
   - First through IPC endpoints the creator makes and grants.
   - A wait and a wake on a word of a shared object comes only if `kbench` shows IPC too slow for a model's layers.
6. **`libmind`.** `process::start_worker(argument, stack, grants)` and `process::worker()`, the worker's view of its index and argument.
7. **[750-KRN-0159](750-KRN-0159-parallel-workers-for-the-effector-benchmark.md)'s workers** can be these: the Effector session would start workers of itself, with no loader client.

## Acceptance criteria

- [x] A QEMU suite with 4 CPUs (x86 and aarch64):
  - a test program starts 3 workers and lends each a part of one memory object;
  - each counts in its part, and the sum is right;
  - each ran on a CPU of its own (`ps`'s CPU of each task);
  - a fifth worker is refused;
  - killing the program ends its workers;
  - a worker's fault ends only that worker, and its creator gets the notice.
- [x] `kbench` times a worker's start and an IPC round trip with a worker.
- [x] `docs/api` and `docs/profile` state the call, its limits and what it does not give (threads).

## Related

[262](262-model-runtime-on-the-machines-full-hardware.md), [262-KRN-0162](262-KRN-0162-a-file-read-once-into-a-memory-object.md), [262-KRN-0164](262-KRN-0164-stacks-larger-than-64-kib.md), [750-KRN-0159](750-KRN-0159-parallel-workers-for-the-effector-benchmark.md), [650](650-building-on-the-target.md), [178](178-models-on-every-accelerator.md) (several CPUs, step 1).

## Progress (2026-10-23)

Made on `claude/youthful-mendel-mf1soy`:
- **The kernel** (`kernel/src/scheduler.rs`): `WORKER_START` (64), ABI 18.
  - A task started with `SPAWN_WORKERS` keeps its program: a boot image's index, or a copy of its ELF charged with its image.
  - A worker is made by the same path as `SPAWN`, from that program, with the granted capabilities (the grant reading is one method for both calls now); its index and argument are at `WORKER_OFFSET` of its info page.
  - A worker goes to a CPU without its creator or a sibling while one is free; at most one per online CPU; it ends when its creator ends.
- **The loader** reads the owner's `data/memory.conf` at each run: a program named there gets its line's MiB, a task quota for workers and `SPAWN_WORKERS` (the window beyond 928 MiB is [262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md)'s).
- **`libmind`:** `process::start_worker`, `process::worker`, `process::spawn_in_front_with`.
- **Tests:**
  - `memtest workers <n> [hold|fault]`, and `workers_check` in the `smp` suite (also `--suites workers`);
  - `kbench workers`: a worker's start until it answers, and a call answered by a worker on another CPU.
- **Checked** (this session's container, QEMU TCG, 4 CPUs), x86 and aarch64 alike:
  - no worker without the owner's line;
  - 4 workers counted on a shared page, and a fifth was refused;
  - one that faulted ended alone, and its creator got the notice;
  - held, the program and its 3 workers ran on 4 CPUs (`ps`), and killing it ended them;
  - `kbench workers`: a worker started in 6.8 ms (x86) and 3.5 ms (aarch64), and answered a call in 179 µs and 207 µs. These are emulated times, for this container only (MC-12.1).
- **Left:** the wait and wake on a shared word, only if a model's layers need it.
