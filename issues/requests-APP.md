# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## `applications_until_memory_ends` with more than 8 CPUs

### Problem

[171-KRN-0008](../issues-done/171-KRN-0008-wake-ipis-with-many-cpus.done) fixed the 16-CPU stall from `requests-KRN.md`. [171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done) then cut the work under the scheduler lock. Updated 2026-10-08.

With 16 CPUs (TCG, 4 host cores), the `normal` suite with this check switched on passed locally on both architectures (the condition removed in a local copy only):
- x86: 78 clocks, 133 s for the suite;
- aarch64: 171 clocks, 547 s.

At the peak, x86 answers `ps` in 0.2 s and aarch64 starts a clock in about 4.6 s. Both are inside the 8 s waits.

### Plan (a proposal; the tools track decides)

Drop the `vm.cpus <= 8` condition and its comment in `normal_suite`. The "16 CPUs" groups then take about 7 minutes longer on aarch64.

### Acceptance criteria

The check runs with every CPU count in CI.

## Clocks ask the RTC service 10 times a second

### Problem

`clock` calls `mind::rtc::seconds_since_midnight()` after every `wait_or_exit(100)`: 10 IPC calls a second per clock.

On x86 each answer is about 14 CMOS port accesses, each a system call. With 60 clocks that came to 8 700 port system calls a second. On aarch64 with 120 clocks, the RTC service's queue was full all the time. A `date` from the shell waited 8.5 s for a place (`000-KRN-0010` covers the kernel's side).

`sysmon` reads `STAT_TASKS` and `STAT_ENDPOINTS` every 100 ms. 171-KRN-0009 made both one pass, but they still cost about a millisecond each under the lock with 170 tasks.

### Plan (a proposal; the tools track decides)

- A clock could read the RTC once and count seconds from `CLOCK` (monotonic nanoseconds), reading the RTC again every minute or so.
- `sysmon` could sample endpoints less often than tasks.

### Acceptance criteria

The RTC service's load does not grow by 10 calls a second with each clock.
