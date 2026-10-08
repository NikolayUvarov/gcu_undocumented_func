# 171-APP-0008 — `applications_until_memory_ends` with every CPU count

**Type:** tools · **Owner:** `APP` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [171](../issues-done/171-limits-from-the-hardware.done) · **Constitution:** MC-12.1, MC-12.2

Numbered by the kernel session at the maintainer's request (2026-10-08). It replaces the first request in `requests-APP.md`.

## Problem

`normal_suite` runs `applications_until_memory_ends` only with up to 8 CPUs (`if memory <= 1 and vm.cpus <= 8`). The condition and its comment date from the 16-CPU stall in `requests-KRN.md`. Two kernel tasks have since dealt with it:

- [171-KRN-0008](../issues-done/171-KRN-0008-wake-ipis-with-many-cpus.done) removed the stall;
- [171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done) cut the work under the scheduler lock.

The kernel session ran the `normal` suite with 16 CPUs and this condition removed in a local copy. Both runs passed:

- x86: 78 clocks, 133 s for the suite;
- aarch64: 171 clocks, 547 s for the suite.

At the peak, x86 answered `ps` in 0.2 s and aarch64 started a clock in about 4.6 s. Both are inside the harness's 8 s waits. CI still does not run the check with more than 8 CPUs, so a regression there would go unseen.

## Plan

1. Drop `and vm.cpus <= 8` and its comment in `normal_suite`.
2. Run the "16 CPUs" groups of `scripts/ci_local.sh` on x86 and aarch64. On aarch64 they take about 7 minutes longer.
3. If a host is too slow for the 8 s waits at the peak, report it to the kernel track with the timings. Do not raise the wait for these commands, and do not keep fewer clocks: either would hide what the check is for.
4. Update the row of `docs/profile/evidence.md` that says the check is not run with more than 8 CPUs.

## Acceptance criteria

- The check runs in CI with every CPU count the `normal` suite runs with (4 and 16), on x86 and aarch64.
- It passes there.

## Related

[171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done), `requests-APP.md` (the clocks' polling of the RTC service, still a request).
