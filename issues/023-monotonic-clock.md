# 023 — Monotonic clock with defined resolution (roadmap K3)

**Type:** feature · **Priority:** P0 · **Status:** open · **Roadmap:** step 2, K3 · **Constitution:** MC-5.6

## Problem

The only monotonic time is `UPTIME`, counted from the 100 Hz PIT tick: 10 ms resolution, no statement of its properties in the ABI. Durations shorter than a tick cannot be measured, and `RDTSC` is raw and uncalibrated.

## Plan

- Calibrate the TSC against the PIT tick at boot; use it as the monotonic clock when it runs at a constant rate (CPUID invariant TSC, or a hypervisor that provides a constant-rate TSC), otherwise fall back to the tick.
- New system call `CLOCK`: nanoseconds since boot, resolution in nanoseconds, TSC frequency; `mind::time::monotonic_ns`, `mind::time::clock_info`. `UPTIME` stays (milliseconds).
- The clock never goes backwards for a task (tasks are pinned to one CPU; the kernel also keeps the last value per CPU).
- Shell command `clock`; profile `clocks.md` updated.

## Acceptance criteria

- `clock` reports a resolution below 1 ms in QEMU and increasing values; a QEMU test checks both.
- All suites pass.

## Related

[docs/profile/clocks.md](../docs/profile/clocks.md), [ROADMAP](../ROADMAP.md) K3.
