# 000-KRN-0233 — The compositor compares a whole frame each time the shell wakes, so an idle machine's CPU 0 seldom halts

**Type:** kernel (the screen's dirty mark, `kernel/src/scheduler.rs`) and the compositor · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-1.2, MC-5.1, MC-12.2

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

## Related

[000-KRN-0201](../issues-done/000-KRN-0201-the-idle-halt-window-holds-a-loaded-hosts-frame-copy.done), [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md) (the compare one pixel at a time), [000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done) and [000-KRN-0186](../issues-done/000-KRN-0186-a-frame-copied-while-it-was-drawn.done) (whole frames, the hold).
