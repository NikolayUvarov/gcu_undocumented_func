# 176-KRN-0181 — `check`'s "the RTC advances" counts its window from before the first read

**Type:** `check` (`bench/src/bin/check.rs`, the clock group) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (made; the bench suite's run left) · **Blocked by:** — · **Main task:** [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

**Reported by:** the tools track (`APP`), `KRN/2026-10-28T0504Z-APP-checks-rtc-advances-races-with-its-first-read.md` on `internal-dev-messages`: seen on i9 under WSL2, aarch64 with 4 CPUs, once in three runs of the `bench` suite, and once in `diag/quick.msh` step 16.

## Problem

- **The order of the reads.** The row read the RTC's seconds first and took the monotonic time `t0` after the answer came. It then waited 1.2 s from `t0`, read the seconds again, and passed only for 1 or 2 s moved.
- **What a late answer does.** When the first answer was slow, the first reading was older than `t0` by that delay, so the RTC could move more than the 1.2 s window allowed. The run that failed showed `3 s in 1.21 s`.

## What was done

- **The window starts before the first read.** `t0` is taken before the first request, and the 1.2 s wait counts from its answer, so the two readings are at least 1.2 s apart.
- **The pass bound.** The window ends after the second answer. The row passes for at least 1 s moved and at most the window's whole seconds rounded up, which is the most a seconds counter can move within it.
- **The row's text** gives the moved seconds and the whole window.

## Acceptance criteria

- The `bench` suite passes on x86 and on aarch64 with 4 CPUs.
- `check kernel clock files sound` in `diag/quick.msh` passes in the same suites.

## Related

[176](176-test-and-performance-utilities.md), [176-KRN-0147](176-KRN-0147-check-kernel-waits-long-enough.md).
