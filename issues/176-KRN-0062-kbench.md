# 176-KRN-0062 — `kbench`: the kernel's performance on the screen and in a log

**Type:** kernel · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and run in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track A, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

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

1. Every group runs on QEMU x86 and aarch64, with plausible orders of magnitude under TCG: a system call below 10 µs, an IPC round trip below 1 ms.
2. The log has every measurement.
3. The QEMU suite checks the table's rows and the log.
4. On the MacBook Pro the numbers are recorded with the configuration.

## Progress

**2026-10-10: made, and run in QEMU (x86 and aarch64, 4 CPUs, TCG).**

- **What there is.** `bench/src/bin/kbench.rs`, with the crate's shared `report` (statistics, units, bars, tables; host-tested in `tests/bench_host.rs`) and `out` (the log file, the machine's description from `sysmon`).
- **How it measures.**
  - Calls below a microsecond or so are timed in batches of 8–32, a sample being the batch's mean.
  - The IPC group starts `kbench --child`, which holds the endpoint and echoes each call. The page variant lends a page for each call, as `ping` does, and revokes it after.
  - The process group times the loader's launch, the start to the child's first message, and the exit until the kernel reports the task gone.
- **The screen.** A table of 79 columns. The log goes to `log:kbenchNNNN.txt`, or to `ram:kbench-NNN.txt` without the log volume (the QEMU suites' disk). It holds the same table, then every measurement's statistics and histogram.
- **The `bench` suite** (`tests/qemu_smoke.py`, CI groups "x86: keys, shell, tools" and "aarch64: programs, shell and four CPUs") checks:
  - every row and its order (min ≤ median ≤ p99);
  - the orders of magnitude, the table's width and the log;
  - a single group, and an unknown group refused.
- **First numbers (TCG, `--quick`, not evidence of any machine).**

  | | x86 | aarch64 |
  |---|---|---|
  | empty system call | 1.8 µs | 1.0 µs |
  | IPC round trip | 181 µs | 216 µs |
  | the same with a lent page | 319 µs | — |
  | process start | 105–133 ms | 187 ms |
- **Found on the way.**
  - **Sleeps.** A sleep can be shorter than asked: up to a tick, because the deadline counts from the tick's start. This is [000-KRN-0065](000-KRN-0065-a-sleep-is-never-shorter-than-asked.md).
  - **Dropped endpoints.** A dropped endpoint counts against its creator's quota until the boot CPU's idle loop reclaims it (`reap`). Creating and dropping endpoints quickly reaches the limit; `kbench` waits 10 ms each time and says how often. On a machine whose boot CPU is never idle, they would not come back. That is worth a task if a real program meets it.

Left: the MacBook Pro's numbers, recorded with the configuration (criterion 4).

## Related

[176-KRN-0063](176-KRN-0063-check.md), [176-KRN-0064](176-KRN-0064-bench.md).
