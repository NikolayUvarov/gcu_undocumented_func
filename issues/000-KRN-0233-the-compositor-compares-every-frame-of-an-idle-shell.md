# 000-KRN-0233 — The compositor compares a whole frame each time the shell wakes, so an idle machine's CPU 0 seldom halts

**Type:** kernel (the screen's dirty mark, `kernel/src/scheduler.rs`) and the compositor · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (in `main` since round 122, 33c38b60; ten runs of "x86: x2APIC" on acc-2 beside a loaded lane passed; a real machine's measure left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-1.2, MC-5.1, MC-12.2

## Problem

**Round 120's gate,** candidate b1693b08 (`main` 06909d9f with 110-NET-0066, 110-NET-0067 and APP's request of 0227Z), "x86: x2APIC" on acc-2. `smp_suite` failed `CPU 0 did not halt when idle`.
- CPU 0 was at CPL 3, at `0x80000026e4`, comparing a 1280×800 frame (R15 = 0x3E8000 bytes, buffers 4 MiB apart): the compositor's copy loop. The record is on acc-2 in `mind-core/KRN/logs/land-r120/acc2-08/`.
- **Rounds 65 and 70 failed the same way.** [000-KRN-0201](../issues-done/000-KRN-0201-the-idle-halt-window-holds-a-loaded-hosts-frame-copy.done) took it for the copy of the last output and widened the check's window to 10 s. A loaded acc-2 outlasted that too.

## What a probe showed (this container, 4 CPUs, QEMU 8.2.2 under TCG, not loaded)

The check's loop, made to sample each CPU's registers for 10 s (every 20 ms or so) instead of stopping at the first halt, with the x2APIC kernel and `--cpu-model max`:

| When | CPU 0 halted | CPU 0 busy at | CPUs 1 to 3 halted |
|---|---|---|---|
| After `smp_suite`'s checks | 17 of 59 samples | `0x26e4`, `0x26ca`, `0x26d8`, CPL 3 | 57 to 59 of 59 |
| 3 s after the boot's prompt | 10 of 30 | the same | 30 of 30 |
| After `ps` | 8 of 30 | the same | — |

- **So the compositor is busy about 70 % of the time on a machine that does nothing.** It is not the last frame being drawn. On a loaded host its compare takes longer still, and CPU 0 is not seen halted once in 10 s.
- **The compositor's log, 1 s after boot:** `SCREEN IN FRONT #1: 16 FRAMES IN 1057 MS, LONGEST 100 MS`.

## The cause

- **The kernel marks a task's screen dirty each time the task blocks, sleeps or is preempted** (`task.dirty = true` in `ipc_send`, `ipc_recv`, the sleeps and the tick), whether or not it wrote its screen.
- **The shell, the screen in front at idle, wakes every 10 ms** (`recv_timeout(…, 10)` or `sleep(10)` at the end of its main loop).
- **So nearly every pull finds the shell's screen dirty,** and the compositor compares all 1 024 000 pixels with its shadow, one by one ([000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md)), about 60 times a second, to find that nothing changed.
- **On a real machine** that is two 4 MiB reads per compare, 60 times a second: a measurable share of one CPU while the machine is idle. That is not measured yet.

## Plan

1. **The kernel learns whether the screen was written.** After a pull that reports a frame, the screen's pages are mapped read-only in its task. The task's first write faults; the kernel marks the screen dirty, maps it writable again and resumes the task. A task that does not write its screen leaves no dirty frame.
   - One way on both architectures: the x86 dirty bit and aarch64's hardware dirty state (FEAT_HAFDBS, which the A64's Cortex-A53 lacks) are not used.
   - The other CPUs' TLBs: the task runs on one CPU at a time, so a pull while it runs elsewhere asks that CPU to flush.
   - The hold of a task found drawing ([000-KRN-0186](../issues-done/000-KRN-0186-a-frame-copied-while-it-was-drawn.done)) stays as it is.
2. **The probe becomes the check.** `smp_suite`'s idle check samples each CPU for its window and asks that it be seen halted in most samples, not once.
3. **The kernel's budget** (MC-1.2) is checked: the change must fit it, or the plan is revised here.

## Acceptance criteria

- At idle after the boot's prompt, CPU 0 is halted in at least 90 % of 10 s of samples, x86 and aarch64, in QEMU.
- The display checks still pass: whole frames (000-KRN-0086, 000-KRN-0186), the screen shown after a program draws it, the boot's first frame.
- "x86: x2APIC" passes ten times on acc-2 beside a loaded lane.
- The share of one CPU the compositor takes at idle, measured on one real machine before and after, is recorded for that machine only (MC-12.1).

## Progress

**Made (2026-11-11)** on `claude/000-KRN-0233-screen-written`, steps 1 and 2:
- **`kernel/src/paging.rs`:** a space keeps one range read-only until it is written (`keep`, `written`, `unwritten`). `STAT VMAP` still shows the range writable, as the task may write it, and a write by the kernel itself through `writable` is noted as a change.
- **`mmu::with_write`** and **`context::write_protected`** on both architectures: a page made writable or read-only; a write refused by a page's permissions (x86 error code P and W; aarch64 DFSC 0b0011xx with WnR).
- **`kernel/src/scheduler.rs`:**
  - a task's `dirty` mark, set at each block, sleep and tick, is gone;
  - a pull counts the screen changed unless it is kept and untouched; a screen copied while its task does not run is kept;
  - a write fault in the kept screen makes it writable again and is retried.
- **The idle check** (`smp_suite`, x86): once a CPU halts, its state is sampled 50 times, and it must be halted in at least half of them.
- **The host test** `a_kept_range_is_read_only_until_written` (`tests/runtime.rs`).
- **The kernel's size:** 7566 lines against its budget of 7600.

**Tested in this container** (4 CPUs, QEMU 8.2.2 under TCG), x86:
- **The probe at idle, 3 s after the boot's prompt:** CPU 0 halted in 28 of 30 samples and 26 of 30 after a command, where `main` gave 10 of 30 and 8 of 30. The compositor drew 2 frames in its first watched second, where `main` drew 16.
- **The suites:** boot, display, normal, memory, dzen and power (39 checks); busy, smp, isolation and heap (8: all CPUs halted under the stricter check, whole frames copied: 443 screens); wm (17). All passed.
- **aarch64, the `normal`, `shell`, `smp` and `busy` suites: passed** (27 checks; all CPUs in WFI, a QEMU thread runnable 5 to 6 % of the time).
- **Left:** the gate; the ten runs of "x86: x2APIC" on acc-2 beside a loaded lane; the real machine's measure.

**Landed** in round 122 (`main` 33c38b60, 2026-11-11), with 000-KRN-0234's step 1: every group of the gate passed on its first run, "x86: x2APIC" among them (acc-1, 230 s). The kernel is 7566 lines against its budget of 7600. **Left:** ten runs of "x86: x2APIC" on acc-2 beside a loaded lane (started), and the compositor's share of a CPU at idle on a real machine.

**The ten runs** (2026-11-11 and 2026-11-12, operations `KRN-krn0233-ten-acc2-01` to `-10`): "x86: x2APIC" on `main` 33c38b60 in lane acc2 on acc-2. Each run had "x86: services, storage, audio" running in lane acc2-2 on the same machine, five runs of it in all (`KRN-krn0233-ten-busy-acc2-2-01` to `-05`, the fifth cancelled once the series ended). **Ten of ten passed**, in 240 to 244 s each. Before the change, rounds 65, 70 and 120 each failed it once in a gate. This is evidence for that machine, that load and those runs, not a proof (MC-12.1, MC-12.2).

**Round 130's gate (2026-11-13)** failed the idle check twice on acc-1, one group after the other, and passed it there before and after:
- **The failures:** "x86: one CPU" at 18:39 UTC (CPU 0 halted in 19 of 50 samples) and "x86: padded vector area" at 18:42 (24 of 50).
- **On the same host:** "x86: x2APIC" at 18:31 and "x86: scheduling, isolation, heap" at 18:39 passed the same check.
- **Alone on x32-128** both groups passed again (138 s and 129 s).
- **What else ran there:** acc-1 also runs other users' KVM guests; under TCG a guest slowed by its host halts in fewer samples. Neither the guests' load nor a cause in the candidate was measured, so this is not a cause found. If it comes back, the next step is the samples beside the host's load at that moment.

**Left:** the compositor's share of one CPU at idle, before and after, on one real machine. It needs a machine with a screen that runs MIND Core: the maintainer's, or DRV's DEXP or Ryzen.

## Related

[000-KRN-0201](../issues-done/000-KRN-0201-the-idle-halt-window-holds-a-loaded-hosts-frame-copy.done), [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md) (the compare one pixel at a time), [000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done) and [000-KRN-0186](../issues-done/000-KRN-0186-a-frame-copied-while-it-was-drawn.done) (whole frames, the hold).
