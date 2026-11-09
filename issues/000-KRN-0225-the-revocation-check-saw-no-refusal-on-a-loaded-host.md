# 000-KRN-0225 — The revocation check saw no refused mint on a loaded host (round 103)

**Type:** tests (the `isolation` suite) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (the check takes up to three walks, on `claude/youthful-mendel-mf1soy`; 20 runs beside a busy host passed; the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.1, MC-12.2

## Problem

- **Round 103's gate** (candidate a927f175) failed the group "x86: x2APIC" once, on x32-128's second lane (x32-2), while the first lane ran other groups there.
  - `revocation_refusal_check` (000-KRN-0104): `REVOKED 3001 IN 12 STEPS; THE CHILD MADE 5386 MINTS AND WAS REFUSED 0 WHILE THE WALK RAN`.
  - The check asks, with more than one CPU, for at least one of the child's mints to be refused (`ERR_REVOKING`) while app2's walk runs.
  - The group passed when it ran again alone (186 s).
- **What the candidate changed:** `get`, the `net` suite's checks and the shell suite's fixtures. Nothing in the kernel or `app2`.
- **What it suggests:** in QEMU each vCPU is a host thread. While the host is loaded, the child's vCPU may not run during the whole walk of 12 steps, so the walk ends before any mint meets it. The child made its 5386 mints before or after.
- **Not shown yet:** whether the walk or the child also behaves otherwise. The failing run's QEMU log is in the lane's logs (`land-a927f175/x32-2-*`).

## Plan

1. Read the failing run's log, and how `app2` times the child's mints against the walk's steps.
2. **If it is the host's scheduling:** the check should rest on what the kernel guarantees, not on a vCPU being run in time (MC-12.2).
   - One way: the child signals that it is minting before the walk starts, and the walk's steps are counted while it mints.
   - Another: the refusal is required only when the child is known to have run during the walk.
   - The kernel's behaviour does not change, and the check is not weakened to "anything goes" (MC-12.5): one mint refused while both ran stays its point.
3. **If it is the kernel:** a fix, with the check kept.

## Acceptance criteria

- The cause found in the failing run's log, and written here.
- The check passes 20 times in a row on x32-128 beside a busy lane, and on one CPU.

## Progress (2026-11-09)

- **The cause, from the kernel's side:**
  - the scheduler puts a new task on a CPU with none of its family (`scheduler.rs`, the new task's CPU), so the child and `app2` run on two CPUs;
  - between the walk's steps the lock is let go, and a waiter marked next goes first (000-KRN-0219). So a mint made while the walk runs meets it, and is refused.
  - **The walk is short:** 3001 nodes in 12 steps, about 2 ms in that run (its own line: 1000 nodes in 595 µs).
  - **So no refusal means** that the child's CPU did not run the child during those 2 ms: its vCPU thread was not run by the loaded host, or another task had that CPU. Neither is something the kernel promises.
- **The change** (`tests/qemu_smoke.py`, `revocation_refusal_check`):
  - with more than one CPU, `app2`'s case `R` runs up to three times, one after another, until a walk has refused a mint;
  - one refusal is still required, and each walk must still remove 3001 (or 3002) in at least 11 steps, with mints made and no fault;
  - the PASS line says which walk it was when it was not the first.
  - With one CPU it runs once, as before.
- **Tested** in the kernel session's container (QEMU 8.2.2, TCG): the `isolation` suite on 4 CPUs (the first walk refused 3) and on 1 CPU.
- **On x32-128** through Effector, at 125a87a6, with the x2APIC group's kernel and CPU model (`--cpu-model max`), beside 24 threads that kept 24 of its 32 threads busy:
  - the `isolation` suite 20 times on 4 CPUs: 20 passed, each at its first walk (8 to 16 mints refused);
  - 5 times on 1 CPU: 5 passed.
  - **So round 103's failure was not reproduced:** under that load the child's CPU ran during every walk. The cause above is inferred from the kernel's code and the walk's length, not seen again. With up to three walks, a failure means three walks in a row without the child running, or a fault in the kernel's refusal.

## Related

[000-KRN-0104](../issues-done/000-KRN-0104-revocation-bounded-by-the-profile.done), [000-NET-0063](000-NET-0063-the-wifi-checks-dmesg-waited-8-s-on-a-loaded-host.md) (round 102's slow `dmesg`, another check on a loaded host).
