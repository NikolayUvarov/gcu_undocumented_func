# 213-DRV-0098 — `display`: both screens at start, in seconds

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3

## Problem

The stages run as a survey. Each is logged in full before its first write, with 1 s before its writes and 3 s after
it. Step 1 reads ten batches with pauses between them. On boots 75 and 76, step 3 began about 56 s into the boot. Boot 76's
stage 16 offered the screen at 130.6 s. For a machine in use, the second screen should come up in seconds.

## Plan

- **A production order:** the same writes and checks in the same order, without the pauses and without the per-write
  lines:
  - step 1 reads only what steps 3 and 4 need;
  - the log keeps one line a stage, and the full detail on a failure.
- **How long each part takes:**
  - the tables' run times (table 12's short path, 76, the others);
  - the waits: OTG status, frames, stage 15's watch;
  - the reads.

  Measured on the Ryzen, then summed: the target is under two minutes from start, and the measure says by how much.
- **The watch** (stage 15's 300 frames) may shorten, if the runs so far allow it: a decision with the evidence.
- **The survey order** stays, behind a switch, for bring-up.

## Acceptance criteria

- [ ] Host cases: the production order makes the same writes as the survey's, in the same order.
- [ ] On the Ryzen: the second screen offered within the measured time, logged with each part's share.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0099](213-DRV-0099-display-both-screens-by-default.md).
