# 000-KRN-0276 — Timed waits that end late on an idle machine: counted by where the time goes, then mended

**Type:** kernel (scheduler) · **Owner:** `KRN` (the kernel session), for DRV's report · **Priority:** P1 · **Status:** in progress (counters made, 2026-11-23) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-5.1, MC-12.1, MC-12.2

## Problem

DRV measured this on the Ryzen (`KRN/2026-11-22T1835Z-DRV-on-the-idle-ryzen-a-2-ms-wait-returns-after-5-to-20-ms-400-times-a-minute-never-while-the-machine-is-busy.md` on `internal-dev-messages`):
- `rtw89_wifi`'s `recv_timeout(2 ms)` came back after more than 5 ms 280 to 480 times a minute, and after up to 18 to 21 ms, while the machine was quiet;
- in the minutes when two cameras streamed it came back on time, and the CPU it shares with `compositor` and `clipboard` (CPU 4) was busiest then;
- no request arrived in the late waits, and the driver's own work stayed under 5 ms.

A late end costs every task that polls with a timeout, and each hop of an exchange that crosses CPUs.

**How a timed wait ends today:**
- The boot CPU's 1 ms tick finds it (`timers`: `expire` for IPC deadlines, the sleep list for `SYSCALL_WAIT`) and marks its CPU.
- `wake_idle` sends that CPU a wake IPI if it is idle, one until it selects again (`woken`).
- An idle CPU has no tick of its own.

The reading does not name the fault. The 18 to 21 ms ceiling is two of `QUANTUM_MS`, which fits either cause:
- a wake that waits for a time slice's end;
- a wait behind another task's slice on the same CPU.

## Plan

1. **Count it (this step).** For each timed wait that ends (an IPC deadline that expired, or a sleep), the kernel keeps its end and when the boot CPU's tick saw it. When the task next runs, the time since its end is counted. If over `LATE_MS` (2 ms), the wait is put in one of four kinds:
   - found late by the boot CPU's tick;
   - on an idle CPU after a wake IPI, with the time from the end to the IPI and from the IPI to the run;
   - on an idle CPU without one;
   - behind another task on that CPU, with its PID.

   Wake IPIs sent, and those held back because one was pending, are counted too.

   Once a minute, only when any wait was late, the kernel writes one line:
   `MIND CORE KERNEL: TIMED WAITS IN MINUTE m: … ENDED, … RAN OVER 2 MS LATE (MOST … MS, PID … ON CPU …): …`
   - It goes into its ring.
   - It also goes into init's log, which the shell's `logs 1` drains (the last 4096 bytes, init's own output among them). After boot nothing reads the ring: init gives up the platform privilege the report needs before it says READY.
   - It does not reach the system log (`dmesg`): logd takes lines that tasks send it, and the kernel sends none.
2. **Mend what the counts name,** on the Ryzen with DRV's instrument, before the machine leaves the bench.
3. **Not in this task:** a tick for idle CPUs, unless the counts say the wake path cannot be mended.

## Acceptance criteria

- [x] Builds for x86 and aarch64; the counters cost a few fields and one line a minute at most.
- [x] QEMU: a quiet boot keeps the line only when waits are late, and its counts add up (late = the four kinds).
- [ ] **On the Ryzen** (DRV, MC-12.1): the line names where the late waits' time goes.
- [ ] The mend, measured there: DRV's 400 a minute become none, or the issue says what is left.

## Progress

**2026-11-23, QEMU x86 (TCG, no KVM), commit after 8252087160.**
- A sleep's end was first taken as the tick that found it, so a late tick was never counted. It is now the wait's end in milliseconds' terms (now less how far the clock is past it).
- A quiet boot on 4 CPUs, 130 s past the prompt: no line, as no wait was late.
- On 2 CPUs with two copies of `tests/busy_app.rs` running, read with `logs 1` (a test build that wrote the line every minute): two lines. In each, late = the four kinds (5721 = 65 + 3 + 0 + 5653; 5641 = 55 + 0 + 0 + 5586).
  - About 13 000 waits ended a minute.
  - Almost all the late ones were behind a busy task, up to 60 and 68 ms. This is TCG under a load made for the test, not a measurement of the Ryzen (MC-12.1).
- `dmesg -s init` did not show the line: see the plan's first step.

## Related

DRV's 213-DRV-0104 (the instrument), 000-KRN-0083 (timers on the boot CPU's tick), 171 (wake IPIs), 000-KRN-0231 (applications moved off a CPU a system task holds).
