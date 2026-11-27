# 000-KRN-0283 — A wake IPI that never came is sent again

**Type:** kernel (`kernel/src/scheduler.rs`, the wake of an idle CPU) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 179, 9d6feac696, 2026-11-26; the cause under x2APIC and a real machine's line left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-5.1, MC-12.1, MC-12.3

## Problem

`x86: x2APIC` failed in two rounds running: round 177 on x32-2 and round 178 on x32. Each time it stopped at the same step, `run memtest endpoints &`, with the shell's prompt never back. Each passed when run again alone.

**Round 178's dump of the stuck machine** (x32, `logs/r178/x32-09/mind-core-stuck-2689098-1.log`):
- all four CPUs halted (`HLT=1`) with interrupts on;
- at 63001 ms the shell waited for the loader's reply, the loader for `vfs_server`, and `vfs_server` for `ata`;
- a second later all of those had answered. The shell (PID 28, CPU 3) was ready (`NONE`) and still not running, with CPU 3 halted.

**What the code does.**
- An idle CPU gets no tick; it sleeps until a wake IPI (`cpu::tick_others`).
- `wake_idle` sends one IPI and sets `woken[cpu]`. Until that CPU selects again, every later wake is held back (`late.held`), and so is the boot CPU's tick pass over every task.
- `select` clears the mark, but only when that CPU runs.

**So one IPI that does not make its CPU select** (lost, or taken by a path that does not reschedule) leaves that CPU asleep for good, with its ready tasks never run. That cause is inferred, not shown: which of these happened under x2APIC is not known.

## Plan (made)

- **`wake_idle`:** a CPU whose mark is older than `WAKE_RESEND_MS` (10 ms) without a select gets the IPI again. The boot CPU's tick pass reaches it within a tick, so a lost wake costs at most about 10 ms and one tick, not the machine.
- **The minute's line (000-KRN-0276)** counts them: `… {n} SENT AGAIN AFTER 10 MS WITHOUT A SELECT`. A minute with a resend is reported even if no wait ran late.
- **Left:** why the first IPI did not lead to a select under x2APIC. The stuck dump is to carry each CPU's `info lapic` (IRR and ISR) next time. That is the harness's, `tests/qemu_smoke.py`.

## Acceptance criteria

- The kernel builds on both architectures within its budget, and the gate's groups pass.
- [x] `x86: x2APIC` run ten times on two lanes at once with no stall (648fa9270b, x32 and x32-2, 2026-11-26), and in round 179's gate. Whether any run needed a resend was not read.
- On a real machine (the Ryzen's idle minutes, DRV's 000-KRN-0276 run), the minute's line says how many wakes were sent again (MC-12.1).

## Related

[000-KRN-0233](000-KRN-0233-the-compositor-compares-every-frame-of-an-idle-shell.md), [000-KRN-0276](000-KRN-0276-late-ends-of-timed-waits-counted.md), 171-KRN-0009 (one IPI until the CPU selects).
