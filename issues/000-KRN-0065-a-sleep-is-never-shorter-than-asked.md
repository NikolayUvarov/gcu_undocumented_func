# 000-KRN-0065 — A sleep is never shorter than asked

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made and tested in QEMU x86; the gate and the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-12.3 (a stated behaviour must hold)

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

## Progress

**2026-10-10: made.**

- **`WAIT`.** The deadline is the monotonic clock at the call, plus the time asked, plus the clock's resolution (a tick where the clock counts ticks). The boot CPU's tick wakes a sleeper on the first tick after it (`kernel/src/scheduler.rs`, `task_state.rs`). At least 1 ms, at most 60 s.
- **A wait in a window.** `mind::windowed::wait` measures on the monotonic clock, not on the tick's uptime.
- **IPC timeouts are left as they were.** They count from the tick's start and may end up to a tick early. Drivers poll with them once a tick: `usb_hid` waits 10 ms for a request between polls. Under the new rule that would turn every 20 ms, and input would lag. `docs/profile/clocks.md` says so.
- **What changes for programs.** A loop that sleeps 10 ms just after a tick now turns every 20 ms, not 10:
  - the shell's idle and network waits (`shell/src/main.rs`, `net.rs`, `msh.rs`);
  - `init`'s wait for a stopped service, up to 200 turns;
  - `usb_host`'s port resets and `rtc`'s settling;
  - `ahci`'s slow wait.

  None of these is on the input path: input still wakes a sleeper at once. A program that wants one turn a tick asks for 1 ms. Short settling pauses now last as asked, such as `usb_host`'s two EHCI frames (`sleep(2)`), which could end at once before.
- **The tests.**
  - The `bench` suite (x86): `kbench` reports no sleep shorter than asked; `sleep 10 ms` has a median of 19.9 ms.
  - `check` passes "sleeps last as asked", so its test now requires every check to pass or be skipped.
  - `docs/api`, `docs/profile` (`clocks.md`, the MC-5.6 row) and `docs/tools` (EN, RU) say the same.

## Related

[176-KRN-0062](176-KRN-0062-kbench.md) (the measurement), [211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done) (the tick).
