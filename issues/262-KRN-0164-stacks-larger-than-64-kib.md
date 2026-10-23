# 262-KRN-0164 — Stacks larger than 64 KiB

**Type:** kernel (spawn), `libmind` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.13, MC-5.1

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md), item 5.

## Problem

- **Every task's stack is 64 KiB** (`STACK_SIZE`, `kernel/src/scheduler.rs`), below an unmapped guard page.
- **What does not fit:** a tokenizer's recursion, or a compute kernel's tiles on the stack.

## Plan

1. **The program states its stack** in its ELF:
   - in `PT_GNU_STACK`'s `p_memsz`, which the linker's `-z stack-size=` sets;
   - rounded up to pages, at most 8 MiB, the room below the screen's window;
   - 0 or no such header: 64 KiB, as now.
2. **It is charged** to the spawner's memory quota with the image, as the 64 KiB is today.
3. **A worker** ([262-KRN-0163](262-KRN-0163-workers-on-several-cpus.md)) gets its stack's size in `WORKER_START`, within the same limit.
4. **The guard page stays** below every stack.

## Acceptance criteria

- [ ] A QEMU suite (x86 and aarch64):
  - a test program linked with `-z stack-size=4194304` recurses through 3 MiB of stack;
  - the same program linked without it faults at the guard page, and the fault names the stack.
- [ ] `docs/api` says how a program asks for its stack.

## Related

[262-KRN-0163](262-KRN-0163-workers-on-several-cpus.md), [262](262-model-runtime-on-the-machines-full-hardware.md).
