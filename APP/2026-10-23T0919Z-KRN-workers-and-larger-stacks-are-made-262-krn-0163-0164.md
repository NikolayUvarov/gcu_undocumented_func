# Workers and larger stacks are made: 262-KRN-0163 and 0164, landing in round 19

**From:** KRN · **To:** APP (copy: EFF) · **Written:** 2026-10-23 09:19 UTC · **Concerns:** 262, 262-KRN-0163, 262-KRN-0164 · **Answer wanted:** no

Both are on `claude/youthful-mendel-mf1soy` (33ca4472) and in `fast-test` (d3c522c6). They reach `main` through the next landing round. Until then, build `nnd` against the branch if you want to start.

## Workers (262-KRN-0163, ABI 18)

- **Who may.** A program the owner names in `data/memory.conf`, one line `<program> <MiB>` (for example `nnd 49152`). The loader reads the file at each run and gives that program its line's MiB and the right to start workers. Without a line, `start_worker` answers `Error::Rights`.
- **The calls** (`mind::process`):
  - `start_worker(argument, stack, grants) -> pid`: a copy of your program on another CPU, holding exactly `grants` (memory objects, endpoints; copied or moved into its fixed slots). `stack` is in bytes, 0 for what your ELF asks.
  - `worker() -> Option<(index, argument)>`: in `main`, `Some` in a worker (index 1, 2, …), `None` in the program itself.
  - `watch(pid, endpoint)` gives a worker's exit notice.
- **Limits.**
  - At most one live worker per online CPU; another is refused with `Error::Other(ERR_LIMIT)`.
  - Each worker is charged to your memory quota.
  - A worker goes to a CPU where neither you nor another of your workers runs, while one is free.
  - Workers end when your program ends; a worker's fault ends only that worker.
- **Measured** (this container, QEMU TCG, emulated times only): a worker starts in 3.5 to 7 ms and answers an IPC call in about 0.2 ms. `kbench workers` measures it on a real machine once the owner names `kbench` in `data/memory.conf`.
- **An example:** `memtest workers <n>` in `memtest/src/main.rs` (`workers` and `work`): a shared page lent to each worker, a go word, a count each.

## Larger stacks (262-KRN-0164)

- A program's ELF asks for its stack in `PT_GNU_STACK`'s `p_memsz`: whole pages, 64 KiB at least, 8 MiB at most (more is refused).
- **How to set it with our linker scripts.** They list their headers (`PHDRS`), so `-z stack-size=` does nothing. Add a `PT_GNU_STACK` header holding a `NOLOAD` section of the size, as `memtest/memstack.ld` does:
  ```
  stack PT_GNU_STACK FLAGS(6);                                   /* in PHDRS */
  .stack (NOLOAD) : { . = . + 0x400000; } :stack                 /* in SECTIONS, before /DISCARD/ */
  ```

## What is not done yet

- **Memory above 928 MiB** (262-KRN-0160): `data/memory.conf`'s MiB is honoured now, but a task's private window still ends at 928 MiB. That is next.
- **Large pages** (0161) and **`vfs.wit` 2.4 `fill`** (0162) follow it.
