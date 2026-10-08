# 171-KRN-0009 — 16 CPUs at the peak: the scheduler lock

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [171](../issues-done/171-limits-from-the-hardware.done) · **Constitution:** MC-1.7, MC-5.1, MC-12.1

## Problem

This is the rest of the tools track's request that [171-KRN-0008](../issues-done/171-KRN-0008-wake-ipis-with-many-cpus.done) began. 171-KRN-0008 removed the stall: with 16 CPUs, clocks now start until the frame pool refuses one. At that peak, though, 16 emulated CPUs (TCG, 4 host cores) answer 10–40 times slower than 4. The tools track's check waits 8 s for each prompt, so it still runs only up to 8 CPUs, and no CI check starts clocks with 16 CPUs.

### Measured cause

The kernel was built with temporary timers (not committed). They counted the time spent waiting for and holding the scheduler lock, the parts of that time, and the wake latency.

In one 5 s window on x86 with 60 clocks:

| | 4 CPUs | 16 CPUs |
|---|---|---|
| lock acquisitions | 105 000 | 34 000 |
| time the lock is held | 62 % | about 100 % |
| time CPUs spent waiting for it | 2.6–3.0 s | 83 s, summed over CPUs |
| one hold, on average | 28 µs | 92–155 µs |
| wake IPI to `select` | 0.2 ms | 3–5.5 ms |

- At the 60-clock peak with 16 CPUs, the lock is held almost all the time, and every CPU spends almost all its time waiting for it. So commands are slow even though most CPUs sit in `hlt`: 345 of 400 samples at 60 clocks.
- One hold costs 3–5 times more with 16 CPUs than with 4: even the trivial `UPTIME` system call measured 28 µs under the lock. The emulated CPU that holds the lock shares 4 host cores with 15 others spinning for it. That makes this a property of this test configuration (16 emulated CPUs, 4 host cores) as much as of the kernel. The single lock turns it into a collapse.
- What the holder spent its time on, with 4 CPUs:
  - `wake_idle`, a pass over all tasks after every event: 41 %;
  - system calls: 33 %. Of these, `STAT` cost 1.4 ms a call. `sysmon` reads it every 100 ms, and `STAT_ENDPOINTS` passed over every task's capabilities once per endpoint;
  - `select`, which passes over all tasks: 15 %.

## Plan

1. **Done in this step.** Three changes cut the lock's work:
   - `wake_idle` looks only at CPUs marked when a task became ready for them. The full pass runs once per tick of the boot CPU, which also bounds a missed mark to one tick.
   - `STAT_ENDPOINTS` makes one pass over the capabilities for all endpoints.
   - `receivable` looks at the endpoint's creator first.
2. **Done in this step.** A check in the "16 CPUs" groups, x86 and aarch64 (`clocks_on_many_cpus`): 40 clocks with screens, every command answered within the harness's 8 s wait, the clocks on more than 8 CPUs.
3. **Open.** The lock itself:
   - per-CPU run queues, so that `select` and `wake_idle` do not pass over every task;
   - system calls that need no scheduler state (`UPTIME`, `CLOCK`, `RDTSC`) answered without the lock;
   - `STAT_TASKS` without its per-task passes (`used_tasks`, `used_endpoints`).

   This is a larger change to the scheduler. It is to be planned as its own step.
4. **Open, for other tracks.**
   - Each clock asks the RTC service for the time 10 times a second. Each answer is about 14 port accesses, each a system call under the lock: 8 700 a second with 60 clocks.
   - `sysmon` reads every task and endpoint every 100 ms.

   Both are the tools track's to judge (`requests-APP.md`).

## Acceptance criteria

The request's criteria still hold:

- With 16 CPUs on x86 and aarch64, clocks start until the frame pool refuses one, and the shell answers throughout within the harness's waits.
- The tools track then runs its check with every CPU count.
- A check in CI starts clocks with 16 CPUs.

## Progress

**2026-10-08, steps 1–2.** Measured on x86 at the peak, with commands sent with no time limit, before and after steps 1–2:

| | 16 CPUs before | 16 CPUs after | 4 CPUs before | 4 CPUs after |
|---|---|---|---|---|
| clocks started | 78 | 78 | 79 | 79 |
| slowest `run clock &` | — | 4.6 s | — | 0.5 s |
| `ps` | 8.9–12.7 s | 1.0–1.7 s | 0.5 s | 0.1 s |
| `uptime` | 4.9–7.3 s | 3.3 s | 0.6 s | 0.3 s |
| `date` | 1.0–2.5 s | 0.1–0.2 s | 0.1 s | 0.1 s |

The lock with 60 clocks and 4 CPUs went from 62 % to 37 % held.

Still to measure for steps 1–2:
- aarch64 at the peak;
- the `normal` suite with `--cpus 16` and the new check on x86 and aarch64;
- that the check fails without 171-KRN-0008.
