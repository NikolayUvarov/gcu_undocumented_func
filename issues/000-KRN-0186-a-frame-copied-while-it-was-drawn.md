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

## Cause (2026-10-29)

- **The hold ran out mid-copy.** The kernel holds the task for the copy until `COMPOSITOR_RELEASE` or 50 ms (000-KRN-0086). The compositor compared and wrote each pixel to the framebuffer from the top, under the hold.
- **Framebuffer writes are slow where the check failed.** Under TCG, writes to QEMU's video memory are far slower than to memory. In the paused run one copy spanned some 50 screens with its top done and its bottom old: `(9, 6)` many times, then `(9, 12)`.
- **What followed.** Once the 50 ms passed, `app2` ran on and drew the next frames. The copy, still on its way down, took their bottom half. All task selection goes through `ready()`, which honours the hold, so no other path let the task run.

## Progress

- **The compositor** (`compositor/src/main.rs`):
  - under the hold it only reads the screen into its shadow buffer and notes each row's changed columns, then releases;
  - it writes the framebuffer from the shadow after the release;
  - its watch line names the longest hold of each second.
- **The kernel:** the hold's bound is 250 ms (`HOLD_NS`), against a compositor that never releases; a copy no longer needs it.
- **The check:** each screen is taken with the guest paused (QMP `stop` and `cont`). QEMU otherwise reads the screen while the guest writes it, so a copy that overtakes the read also shows the bottom newer.
- **The docs:** `common/abi.rs`, `docs/api/README.md` and the profile's row on task screens.
- **Left:** the acceptance runs.

## Acceptance criteria

- The check passes 20 times in a row under load: two lanes on `un-rizen`, and the guest paused for each screen.
- The cause and the fix recorded here.

## Related

[000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done); round 45's gate (LANDING/ on `internal-dev-messages`).
