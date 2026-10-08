# 000-KRN-0011 — System calls that read only clocks, without the scheduler lock

**Type:** kernel · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** stage II · **Constitution:** MC-5.2, MC-5.6

## Problem

Every system call took the one scheduler lock, even those that read only a clock: `UPTIME`, `CLOCK` and `RDTSC`.

With 16 CPUs under load the lock was held almost all the time ([171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done)). A task that only wanted the time waited behind scheduling work it does not touch. With 60 clocks on x86, `UPTIME` was called 1 500 times a second, most of it from `libmind`'s `ERR_BUSY` retries, which [000-KRN-0010](../issues-done/000-KRN-0010-ipc-back-pressure-without-starvation.done) removed. Each call measured 28 µs under the lock with 16 CPUs.

## Plan

- Each CPU publishes the mailbox of the task it runs (`MAILBOX`), set at `select`.
- A system call entry reads the number from there before taking the lock. `RDTSC`, `UPTIME` and `CLOCK` are answered at once from the clocks, through the same function the locked path uses (`clock_syscall`).
- The running task's mailbox stays valid while it runs, because `reap` skips current tasks.
- The calls answered this way are counted per CPU (`FAST_CALLS`). They are added to the task's `calls` and the CPU's `interrupts` at the CPU's next `select`.
- A task killed while it runs stops at the CPU's next tick or wake IPI, as before. The locked path did not stop it at an `UPTIME` either: that call returned without `select`.
- Port I/O stays under the lock. Its capability check reads a capability table that another CPU may grow or change.

## Acceptance criteria

- The three calls give what they gave before. The suites that use them pass: `normal`, `busy`, `smp` and `isolation` on x86 and aarch64, with 1, 4 and 16 CPUs as the gate runs them.
- `STAT`'s per-task `calls` and per-CPU `interrupts` still count them.

## Related

[171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done), [000-KRN-0010](../issues-done/000-KRN-0010-ipc-back-pressure-without-starvation.done).
