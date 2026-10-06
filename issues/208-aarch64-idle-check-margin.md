# 208 — aarch64: the idle check of the normal and smp suites has a margin

**Type:** porting (tests) · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.2

From the tools track's report (2026-10-06, formerly `porting-track-reports.md`).

## Problem

On aarch64 the `normal` and `smp` suites check that the CPUs wait in WFI by QEMU's processor time: an idle `virt` with 4 CPUs under TCG had to use less than 0.6 s in 1 s. Measured idle samples were 0.46–0.58 s; one `normal` run failed with 0.61 s, and main's CI failed the GICv2 job with 0.63 s. The limit sat inside the normal spread: the check failed on a busy runner without a regression.

The scheduler's idle time (`STAT` `idle_ns`) cannot replace it: a CPU that spins in its idle loop instead of waiting in WFI is idle for the scheduler too. The host's processor time is what shows the difference.

## Plan

- The lowest of three 1 s samples, against 0.85 s: one vCPU that spins costs about a whole second by itself under multi-threaded TCG, so a spinning CPU still fails, and a sample that meets a busy host or a burst of wakeups does not.
- Show it: the check fails with a kernel whose idle loop spins (a mutation run, recorded below).
- Fewer wakeups at idle (the shell's 10 ms poll, netstack's) is separate work for the tools and kernel tracks.

## Acceptance criteria

The check passes with the host busy (a parallel build at `nice 19`) and fails with a kernel whose idle CPUs spin instead of executing WFI.

## Related

[201](../issues-done/201-aarch64-boot.done), [203](../issues-done/203-aarch64-smp-and-power.done), [204](../issues-done/204-aarch64-profile-and-ci.done).
