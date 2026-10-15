# 000-APP-0081 — Highlight the window in keyboard placement mode

**Type:** tools / window manager · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer's Ryzen notes request a visible highlight for the window being
moved or resized with Alt+M. Mouse dragging already uses an accent frame and
inverted title, but keyboard placement only changes the status line: rendering
does not consult `Mode::Move`.

## Plan

Use the existing placement highlight for the window selected by Alt+M, deriving
it from the current mode when drawing. Keep it through pointer motion and
keyboard movement/resizing, and remove it when Enter finishes or Esc restores
the previous position. In full screen, overlay the frame while retaining the
content's size and coordinates; cancellation also restores its full-screen
state. Preserve normal focus and mouse-drag behavior.

## Acceptance criteria

- Alt+M highlights its target before any movement, in text and pixel windows.
- Pointer motion and Shift+arrows retain the highlight.
- Enter and Esc restore ordinary frame styling without stale highlighting;
  Esc still restores the frame and Enter still applies snapping.
- Full-screen placement is visible immediately without a resize request;
  Enter without movement preserves full screen, and Esc restores it after a move.
- Existing window-manager host tests and a placement transition regression pass.

## Related

- [211-APP-0037](../issues-done/211-APP-0037-wm-marks-the-window-it-drags.done), mouse-drag feedback.
- `wm/src/desk.rs`; `tests/wm_host.rs`.
- [213](213-amd-ryzen-pc.md), the source of the maintainer's report.

## Progress

**2026-10-15, ASTRA:** `Wm::draw` applies the existing accent frame and inverted
title to the keyboard placement target. It does not change pointer-drag state.
The highlight follows the current mode, so pointer motion cannot clear it and
Enter/Esc leave no stale highlight.

Full-screen windows show the same frame over the edge of their content without
resizing it. Pixel ownership and text-cursor clipping keep the frame visible;
Enter without movement preserves full screen, and Esc restores the saved
full-screen state after movement.

Validation on ASTRA: all 40 `tests/wm_host.rs` tests pass, including new
regressions covering text and pixel windows, entry before movement, pointer
motion, keyboard movement/resizing, Enter and Esc. Existing drag and snapping
tests remain green. The full-screen regression also checks unchanged geometry,
no resize notification on entry, interior content and exit cleanup. The full
branch gate remains with the coordinating auditor.
