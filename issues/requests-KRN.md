# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file, and the file goes when it is empty.

## With 16 CPUs the system stops answering after about 25 programs with screens

### Problem

The tools track's `applications_until_memory_ends` (`tests/qemu_smoke.py`, `normal` suite) starts `run clock &` until the frame pool refuses one. With 4 CPUs about 80 clocks start on x86 and 172 on aarch64, and the refusal is clean. With 16 CPUs (`--cpus 16`, TCG), after 24–26 clocks the next `run clock &` gets no answer, ever:

- It reproduces on `main` at 0e1d239 (24 clocks, x86), on the tools branch (26, x86) and on CI (x86 and aarch64, groups "16 CPUs").
- At the hang every CPU is in the kernel (CPL 0). In a first sample they were in `scheduler::reap` and `scheduler::interrupt` spinning on the lock, and in the idle loop (`ap_entry`). In a second sample 5 s later, all but one were in the idle loop (`hlt`) and CPU 0 in `_start`'s `hlt`. Ctrl+Z on the serial line got no answer.
- The frame pool was far from empty (26 screens of 4 MiB of 385 MiB).
- `main`'s own check of 40 applications at once (`memtest hold 0`, no screen) passes with 16 CPUs.

Since 171-KRN-0003 an idle CPU gets no tick and sleeps until a wake IPI. A task left ready on an idle CPU, or a wake that does not reach it, would look like this. That is a guess: the probe did not see the tasks' states.

### To reproduce

On `main`, after `./02_build.sh`: the normal suite's first part, then `run clock &` in a loop with `--cpus 16`. The tools branch's `normal` suite runs it (`applications_until_memory_ends`) and leaves it out with more than 8 CPUs until this is fixed.

### Acceptance criteria

With 16 CPUs on x86 and aarch64, clocks start until the frame pool refuses one, and the shell answers throughout. The tools track then runs its check with every CPU count.

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

### Plan

The kernel track decides: measure the loop's share against the guest's own time (the CPU's busy plus idle time from `STAT_CPUS` over the same interval) rather than the host's wall clock, or another way that keeps the check as strict.

### Acceptance criteria

The check keeps its meaning (no budget: the loop takes most of its CPU) and does not fail when the host is busy.
