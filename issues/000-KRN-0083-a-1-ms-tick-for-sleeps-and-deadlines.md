# 000-KRN-0083 — A 1 ms tick for the clock, sleeps and deadlines; time slices stay 10 ms

**Type:** kernel (`kernel/src/arch/*/interrupts.rs`, `context.rs`, `scheduler.rs`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the tests and the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-5.6, MC-12.1, MC-12.3

## Problem

The maintainer (2026-10-11): `wm`'s moving background stutters on the MacBook Pro. [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md) dealt with the framebuffer. The other part is time: every sleep ended on the boot CPU's 10 ms tick.

`kbench` on the MacBook Pro (`kbench0002.txt`, the same build as 000-KRN-0065's):

| | median | shortest | longest |
|---|---|---|---|
| sleep 1 ms | 9.99 ms | 1.67 ms | 10.0 ms |
| sleep 10 ms | 20.0 ms | 19.9 ms | 20.0 ms |

`wm`'s loop sleeps 5 ms while busy and 30 ms otherwise, so it ran every 10 or 40 ms. A frame came 10 or 20 ms after the last one, depending on where the tick fell, never at a steady pace. IPC timeouts had the same 10 ms step: `usb_hid` polls with them.

## Plan

- **The tick.** The boot CPU's timer ticks every millisecond:
  - x86: the LAPIC timer as measured on the ACPI PM timer, or the PIT, now with a 1 ms divisor;
  - aarch64: the virtual timer, its compare value set a step after the last tick, not after the time it was taken. Late ticks are counted rather than lost, and the tick does not drift.
- **The time slice stays 10 ms.** Only every tenth tick ends a time slice. That tick is the old `Event::Tick`: it is forwarded to the CPUs that run a task, runs `select`, counts task ticks and enforces the budgets (C7).
- **The ticks in between.** They are a new `Event::Clock`, on the boot CPU only:
  - the sleeps due end;
  - the IPC deadlines due fail;
  - idle CPUs whose tasks became ready get a wake IPI;
  - the running task goes on.
- **A cheap pass.** The scheduler keeps the earliest sleep end and the earliest IPC deadline set since the last pass. It looks at the tasks only once one of them is due.
- **Uptime on x86.** Once the TSC is calibrated, `UPTIME` (and the kernel's milliseconds) reads the monotonic clock, so a tick taken late, as under QEMU's TCG, does not slow the uptime down.
- **Not done here.**
  - A tickless idle: the boot CPU wakes every millisecond, even when nothing runs.
  - Preemption within a time slice: a task made ready on a busy CPU still waits up to 10 ms.

## Acceptance criteria

- In QEMU, in the `bench` suite on x86 and aarch64, `kbench`'s "sleep 1 ms" has a median below 8 ms (it was a tick, 10 ms), and no sleep is shorter than asked.
- These checks pass as before:
  - the `busy` suite's budget, 20 ms per 100 ms near 20 %;
  - the idle checks (HLT on x86, QEMU's processor time on aarch64);
  - every other suite.
- On the MacBook Pro, `kbench timer`:
  - "sleep 1 ms" is between 1 and 2 ms;
  - "sleep 10 ms" is between 10 and 11 ms.

## Related

[000-KRN-0065](000-KRN-0065-a-sleep-is-never-shorter-than-asked.md), [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md), [176-KRN-0062](176-KRN-0062-kbench.md), [clocks.md](../docs/profile/clocks.md).
