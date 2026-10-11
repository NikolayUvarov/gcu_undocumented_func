# 000-KRN-0084 — An IPC round trip between two CPUs: 60 µs on the MacBook Pro

**Type:** kernel (`scheduler.rs`, `arch/x86_64/cpu.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (the report reads `MSR_POWER_CTL`; the cause to be found) · **Blocked by:** — · **Roadmap:** track A, main task [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

`kbench` on the MacBook Pro (`kbench0002-2.txt`, 2026-10-11, `--quick`):

| | median | min | p99 |
|---|---|---|---|
| call and reply | 60.4 µs | 7.09 µs | 68.4 µs |
| … with a lent page | 110 µs | 109 µs | 207 µs |
| empty system call | 417 ns | 416 ns | 427 ns |

The fastest round trip took 7 µs, the usual one eight times as long. `wm`, `compositor`, the window broker and the drivers all talk by calls, so each call pays this.

**Our reading, not yet tested.** `kbench --child` runs on another CPU, as every new application is placed on the CPU with the fewest. A call then:

1. wakes that CPU out of `HLT` with an IPI;
2. its reply wakes the caller's CPU, which went idle meanwhile.

On this Ivy Bridge, `HLT` may become C1E (`MSR_POWER_CTL` bit 1, not in the report yet). Leaving C1E and taking the IPI could cost tens of microseconds each way. The 7 µs runs would be those where the other CPU had not reached `HLT` yet.

## Plan

1. **Measure first.**
   - Add `MSR_POWER_CTL` (0x1FC) to the hardware report.
   - Give `kbench` a row for a round trip with both tasks on one CPU, if placement allows, beside the one between two CPUs.
   - Add a row for the wake IPI alone: the time from `wake` to the woken CPU's `select`.
2. **Then one of these, by what the numbers say:**
   - an idle CPU polls its run queue for a short time, about 20–50 µs, before `HLT`, so a reply that comes back at once needs no IPI;
   - an application's server-side child (`kbench --child`, a program's helper) is placed on its parent's CPU;
   - `MWAIT` with a shallow C-state hint instead of `HLT`, as part of 174's power step.
3. **Measure again** on the MacBook Pro, and record the power cost (an idle machine's package power from RAPL).

## Acceptance criteria

- The report shows `MSR_POWER_CTL`.
- `kbench` shows the round trip on one CPU and between two.
- On the MacBook Pro, the median round trip between two CPUs is within twice the one-CPU figure, or the task says why it cannot be.
- An idle machine's package power is no more than 10 % above today's.

## Progress

**2026-10-11.** The hardware report lists `MSR_POWER_CTL` (0x1FC) with the other Intel MSRs; bit 1 is C1E. The MacBook Pro's next report says whether `HLT` there becomes C1E.

## Related

[176-KRN-0062](../issues-done/176-KRN-0062-kbench.done), [174](174-full-use-of-pc-hardware.md) (step on C-states), [000-KRN-0083](000-KRN-0083-a-1-ms-tick-for-sleeps-and-deadlines.md).
