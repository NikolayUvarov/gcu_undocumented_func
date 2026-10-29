# 000-KRN-0186 — The compositor copied a frame its program was drawing

**Type:** kernel track (`compositor`, the isolation suite's check) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2

## Problem

- **What the check asks** ([000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done)): `app2` numbers its frames in their colour and draws the bottom half 3 ms before the top. A screen whose bottom half is newer than its top shows a frame copied while it was drawn.
- **It failed twice on 2026-10-29:**
  - in round 45's gate, on `un-rizen`'s second lane while the first ran other groups: top 238, bottom 241;
  - in this session's container, with the guest paused for each screen (QMP `stop`, `screendump`, `cont`): top 9, bottom 12.
- **What the second run shows.** A paused screen is what the video memory holds, so the newer bottom was copied there. QEMU's reading of a changing screen did not cause it.
- **Rarely seen.** The same group passed when run again alone on `un-rizen` (34 s), and in every earlier gate.

## Plan

- Find when the compositor copies a program's surface against when the program has finished a frame: the order of `present`, its reply and the copy, and whether a program can draw into the surface while it is copied.
- Find why a slow or loaded machine shows it, and a fast one does not.
- Fix the protocol or the compositor, not the check.

## Acceptance criteria

- The check passes 20 times in a row under load: two lanes on `un-rizen`, and the guest paused for each screen.
- The cause and the fix recorded here.

## Related

[000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done); round 45's gate (LANDING/ on `internal-dev-messages`).
