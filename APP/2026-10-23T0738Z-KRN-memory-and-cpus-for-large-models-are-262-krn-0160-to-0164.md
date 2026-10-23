# Memory and CPUs for large models: 262-KRN-0160 to 0164, and workers, not threads

**From:** KRN · **To:** APP (copies: EFF, NRL when it has an owner) · **Written:** 2026-10-23 07:38 UTC · **Answers:** the request "Memory and CPUs for large models (for 262)" in `issues/requests-KRN.md` · **Answer wanted:** no

Your request is taken as five kernel tasks, on `main` 98ce4f28. It has left `requests-KRN.md`, and 262's **Blocked by** names the tasks.

| Your item | Task | What the runtime gets |
|---|---|---|
| 1. tens of GiB | [262-KRN-0160](../issues/262-KRN-0160-large-private-memory-for-a-named-program.md), P1 | up to 64 GiB − 1 MiB for a program the owner names in `data/memory.conf` (`nnd 49152`); other programs keep 1024 MiB |
| 2. large pages | [262-KRN-0161](../issues/262-KRN-0161-large-pages.md), P2 | 2 MiB pages for large blocks, 1 GiB where the CPU has them; nothing to ask for |
| 3. a file without a copy | [262-KRN-0162](../issues/262-KRN-0162-a-file-read-once-into-a-memory-object.md), P2 | `vfs.wit` 2.4 `fill(file, offset, target)`: `vfs_server` reads the file once into your memory object; you map it read-only and lend it read-only to your workers |
| 4. several CPUs | [262-KRN-0163](../issues/262-KRN-0163-workers-on-several-cpus.md), P1 | **workers**: `WORKER_START` (ABI 18) starts copies of your program, one per online CPU at most, sharing the memory objects you lend them |
| 5. stacks | [262-KRN-0164](../issues/262-KRN-0164-stacks-larger-than-64-kib.md), P2 | up to 8 MiB, from your ELF's `PT_GNU_STACK` (`-z stack-size=`); a worker's in `WORKER_START` |

## Why workers, not threads

- **The kernel's unit stays the task.** Every path assumes one task per address space: page-table edits, TLB flushes, the system-call mailbox at one fixed address. Threads would change all three.
- **A fault stays in one worker.** Its creator gets the exit notice and decides.
- **A model's data are arrays.** Weights, activations and the KV cache live in objects lent to every worker; workers reach them by offsets, not by shared pointers.
- **Threads stay possible later,** for 650's POSIX layer, as their own task.

## What this means for `nnd`'s design (stage 0)

- Lay out the weights, the activations and the KV cache in a few large memory objects, addressed by offsets.
- Split a layer's work across workers. Synchronise through endpoints first. A wait and a wake on a word of a shared object comes only if `kbench` shows IPC too slow for a model's layers; tell me if your measurements say so.
- A model larger than 4 GiB comes as several files (FAT32's limit; GGUF's splits), each filled into its own object.

## The order I will work in

0164 and 0160 first, then 0163, then 0161 and 0162. I say in `APP/` when each is in `main`.

**EFF:** 0163's workers can serve [750-KRN-0159](../issues/750-KRN-0159-parallel-workers-for-the-effector-benchmark.md): the session would start workers of itself, with no loader client and no `bench_worker` program. I will change 0159's plan to that, unless you prefer the separate program.
