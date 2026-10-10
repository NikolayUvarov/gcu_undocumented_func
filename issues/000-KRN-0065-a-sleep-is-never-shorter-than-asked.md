# 000-KRN-0065 — A sleep is never shorter than asked

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-12.3 (a stated behaviour must hold)

## Problem

`kbench` (176-KRN-0062) found it on its first run, in QEMU on x86 and aarch64: `sleep 10 ms` took 6.0 to 13.7 ms, 3 of 5 sleeps shorter than asked. `sleep 1 ms` took 4.7 to 13.6 ms.

The cause is in `SYSCALL_WAIT` (`kernel/src/scheduler.rs`):

- the deadline is `interrupts::milliseconds()` plus the time asked, rounded up to 10 ms;
- `milliseconds()` counts whole ticks of 10 ms (`TICKS * TICK_MS`), so "now" is the start of the current tick, up to 10 ms in the past;
- so a sleep of 10 ms wakes at the next tick boundary, anywhere from 0 to 10 ms later.

`mind::time::sleep` says it "sleeps up to `ms` milliseconds (10 ms granularity)". A program that waits 10 ms for a device to settle can wake almost at once, and so can a protocol timer or a debounce.

## Plan

- **The deadline.** Count it from the monotonic clock (`clock::now_ns`, the TSC or the generic timer). Wake a sleeper at the first tick at or after it. A sleep is then never shorter than asked and at most one tick longer.
- **The ABI.** The result stays the time slept, in milliseconds. No ABI change.
- **Who depends on today's behaviour.** Check the programs that poll with `sleep(10)` and expect about 10 ms a turn: they would turn every 10–20 ms. List what changes.

## Acceptance criteria

1. On QEMU x86 and aarch64, `kbench timer` shows no sleep shorter than asked; the `bench` suite checks it.
2. The suites that time polling loops still pass.
3. On the MacBook Pro, `kbench timer` is recorded before and after.

## Related

[176-KRN-0062](176-KRN-0062-kbench.md) (the measurement), [211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done) (the tick).
