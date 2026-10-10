# 176-KRN-0062 — `kbench`: the kernel's performance on the screen and in a log

**Type:** kernel · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track A, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

Nothing measures what the kernel's basic operations cost on a given machine, so neither a change's effect nor a machine's difference can be seen.

## Plan

`kbench [group …] [--quick]`, a console program. Each measurement is repeated, and the minimum, median, 99th percentile and maximum are kept, in nanoseconds of the monotonic clock.

| Group | What it measures |
|---|---|
| `syscall` | an empty system call (the uptime), a clock read |
| `ipc` | a call and its reply between `kbench` and a copy of itself it starts (the copy holds the endpoint and answers); the same with a page lent for each call and revoked after it |
| `caps` | minting and dropping a capability; creating and dropping an endpoint |
| `memory` | allocating and freeing one page; allocating, touching and freeing 1 MiB (MiB/s) |
| `timer` | how long a sleep of 1 ms and of 10 ms takes |
| `process` | starting a program and its exit, seen by the parent |

- **The screen.** The machine (CPUs, the clock's resolution), then one row a measurement: the median with a bar (log scale), the minimum and the 99th percentile.
- **The log.** `log:kbenchNNNN.txt` gets every measurement's count, minimum, median, mean, 99th percentile, maximum and a histogram in powers of two.
- `--quick` runs a tenth of the repetitions.

## Acceptance criteria

1. Every group runs on QEMU x86 and aarch64, with plausible orders of magnitude: a system call below 10 µs, an IPC round trip below 100 µs under TCG.
2. The log has every measurement.
3. The QEMU suite checks the table's rows and the log.
4. On the MacBook Pro the numbers are recorded with the configuration.

## Related

[176-KRN-0063](176-KRN-0063-check.md), [176-KRN-0064](176-KRN-0064-bench.md).
