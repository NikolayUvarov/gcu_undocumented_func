# 801-EFF-0039 — No rewrite of the agent's journal on a trial boot

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-21) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S3) · **Constitution:** MC-12.1

## Problem

KRN's request of 2026-11-19, "No rewrite of the agent's journal on a trial boot" in `issues/requests-EFF.md`, taken here.

- **What round 149's gate showed** (x32, the x86 `rollback` suite): `[('the trial changed what slot A reads', ['system/effector/journal.new'])]`. It passed when it ran again alone.
- **Why:**
  - the agent compacted `system/effector/journal` at each start, writing `journal.new`, removing `journal` and renaming the new one into its place;
  - on slot B's trial, `system/effector` holds slot A's journal too, so a compaction there rewrites what slot A reads, and a cut one leaves its pieces;
  - `recover()` at the start also renames a lone `journal.new` and removes a leftover one.

## Plan

1. **At its start the agent asks `init`'s `boot`.** On a boot on trial that is not confirmed yet (`trial` and not `confirmed`, which a refused trial also is):
   - the journal is not compacted, and the log says so: `EFFECTOR: JOURNAL: NOT COMPACTED ON A TRIAL BOOT; THE FIRST START AFTER THE TRIAL IS CONFIRMED COMPACTS IT`;
   - `recover()` reads the journal where it lies (a lone `journal.new` in place of a missing `journal`), and renames and removes nothing.
2. **The first start after the trial is confirmed** compacts the journal and tidies a leftover, as before.
3. **What the trial still adds** are the answers it must keep: the kept update's, once `init` decides (801-EFF-0034).

## Acceptance criteria

- [ ] The `effector-update` suite, x86 and aarch64: slot B's agent, started on trial, logs that its journal was not compacted.
- [ ] UPD's `rollback` suite, x86 and aarch64, in the gates that follow: no `journal.new` among the files the trial changed.

## Related

[801-EFF-0034](801-EFF-0034-the-kept-update-waits-for-inits-word.md), 801-UPD-0041, [801-KRN-0242](../issues-done/801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.done).
