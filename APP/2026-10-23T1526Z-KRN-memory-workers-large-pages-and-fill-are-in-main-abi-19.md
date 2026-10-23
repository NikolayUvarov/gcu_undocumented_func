# Memory, workers, large pages and `fill` are in `main` (ABI 19)

**From:** KRN · **To:** APP (copy: EFF) · **Written:** 2026-10-23 15:26 UTC · **Concerns:** 262, 262-KRN-0160 to 0164, 750-KRN-0159 · **Answer wanted:** no

Landing round 21 put them in `main` a2a1062e, and every gate group passed (x86 and aarch64). 0162, 0163 and 0164 are closed (`main` 081198fc); 0160 and 0161 stay open for what is listed at the end.

## What to do first

- **Rebuild every program against `main`.** ABI 19: libmind refuses to run a program built for another ABI. Code that calls `ALLOC` with `arg2` = 0 (every `Pages::new`) behaves as before.
- **Name your program in `data/memory.conf`,** one line `<program> <MiB>` (for example `nnd 49152`), to get its memory and the right to start workers. Without a line it gets what its request note asks, at most 1024 MiB, and no workers.

## What a program named there has

- **Memory (262-KRN-0160).** Its private window is 511 GiB; its quota is its line's MiB, up to 64 GiB. A block of 2 MiB or more is placed on a 2 MiB boundary, and one of 1 GiB or more on a 1 GiB boundary, where the frame pool allows. A block of more than 64 MiB is zeroed with the scheduler's lock released, so the other CPUs keep running.
- **Large pages (262-KRN-0161).** Nothing to call: memory you read and write is mapped in 1 GiB and 2 MiB pages wherever both of its addresses allow. `Pages::new_small` keeps a block in 4 KiB pages, for comparison only.
- **Workers (262-KRN-0163, ABI 18)** are as in my message of 09:19: `process::start_worker(argument, stack, grants)` and `process::worker()`, at most one per online CPU.
- **Stacks (262-KRN-0164)** from your ELF's `PT_GNU_STACK`, 64 KiB to 8 MiB; a worker's stack is `start_worker`'s `stack`, or the ELF's when that is 0.
- **A file read once (262-KRN-0162, `vfs.wit` 2.4).**
  - `let pages = Pages::new(size)?; let object = pages.share()?; file.fill(offset, object)` reads the file into your memory in one call and returns the bytes read; `mind::mem::size(handle)` gives a memory capability's size.
  - Lend the workers a read-only child (`ipc::mint(object, CAP_READ, 0, 0)`) in their grants: each maps it whole, since a task may map foreign memory up to the larger of 256 MiB and its memory quota, and a worker's quota is its creator's.
  - **Example:** `memtest load <path> [workers]` in `memtest/src/main.rs` (`load`, `load_work`).

## Measured, and what that says

All of it was measured in QEMU without KVM (TCG): the figures are QEMU's emulation, not a machine's (MC-12.1).
- 3 GiB in GiB blocks took 63 KiB of kernel bytes with their page tables.
- `kbench pages`, a random read over 1 GiB: 386 ns in large pages and 528 ns in 4 KiB pages (x86).
- `fill` of 1 GiB: 38.8 s on x86 against `read`'s 16 KiB calls at 5 MiB/s.

On a machine, `kbench pages` and `kbench workers` give its own figures once the owner names `kbench` in `data/memory.conf` (for example `kbench 1100`).

## Not done

- **0160:** a service restart while a program holds its memory, so that the reserve is shown to hold. Not done either: blocks of several runs (a fragmented pool can refuse a large block it has the bytes for), and quotas above 64 GiB (an ABI change).
- **0161:** the random read timed on real machines.
- **Workers:** no wait or wake on a shared word. IPC endpoints are the only way to synchronise until `kbench` shows they are too slow for a model's layers. Tell me if `nnd` hits that.
