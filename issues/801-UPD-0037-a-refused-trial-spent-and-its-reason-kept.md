# 801-UPD-0037 — A refused trial spent and its reason kept: the updater confirms only on `init`'s word

**Type:** update (`updater`) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** 801-KRN-0242 · **Main task:** [801](801-the-bench-stays-manageable.md) (S1, S2) · **Roadmap:** stage V · **Constitution:** MC-9.4, MC-9.9

## Problem

**The updater confirms a trial on the disk on its own.** When `init` does not answer, it writes the confirmed record past the kernel's deadline (351-UPD-0021). With 801-KRN-0242 `init` lifts that deadline and holds the trial for the Effector server, so that rule would confirm a release that never reached the server.

**A trial `init` refused costs its remaining tries.** The bootloader boots it again until its tries are used up, and nothing on the disk says why it fell back. S2 wants the reason reported by the slot that had the connection.

## Plan

- **On the disk, a trial is confirmed only on `init`'s word.**
  - While `init` says `deciding`, the updater asks again every second.
  - When `init` cannot say, the trial stays unconfirmed and the bootloader counts its tries.
- **On a refusal:**
  1. the reason is kept in `system/updater/refused`: the version and slot, the fallback, `init`'s `why`, and the trial's `log:bootNNNN.log`;
  2. the trial is spent (`plan::spend`: the same slot and fallback, no tries left), so the next boot falls back at once;
  3. the updater asks `init` to restart.
- **At every start** the updater says a reason kept there. It gives it as the update state's failure, which the agent reports (EFF's S2 part), until a trial is confirmed; then the file goes.

## Acceptance criteria

- **The host test** `a_trial_init_refused_is_spent_and_the_next_boot_falls_back`: a spent trial leaves the bootloader's plan on the fallback, not on trial.
- **The `boot` suite's refused trial** (with 801-KRN-0242): two tries, one used. The next boot is slot A's, which names the reason, and `system/updater/refused` holds it. The newer record is B's with no tries left.
- The updater groups pass on both arches on the maintainer's machines, then in a gate.

## Related

[801](801-the-bench-stays-manageable.md), [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md), [351-UPD-0021](../issues-done/351-UPD-0021-the-trial-confirmed-once-init-says-so.done), [docs/update/updater.md](../docs/update/updater.md).
