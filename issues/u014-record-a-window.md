# u014 — `record -w`: recording one window of `wm`

**Type:** tools · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — (093 done) · **Roadmap:** track G · **Constitution:** MC-3.3, MC-10.2

## Problem

`record` (issue 093) records the whole screen in front. A demo or a bug report of one program in `wm` would rather have just its window, at the window's size, whatever else is on the screen.

The window's pixels are in its surface, the window broker's memory (157). The program that drew it and `wm` hold leases of it; the shell does not. Text windows hold cells, which need to be drawn (the 8×16 font) to become pixels.

## Plan

- `wm` starts `record` for a window: from its run line (`record -w`, the window in front) or from a key of its own.
  - `wm` lends `record` a read-only lease of that window's surface, minted from its own, and nothing else of the window.
  - No program records another's window unless the user starts it.
- `record` draws the frames itself:
  - a pixel window's content as it is;
  - a text window's cells with `mind::font16`, as `wm` draws them.
  
  The frame follows the surface's size (a window resized while it records is scaled to the first size, or the recording ends; to be decided).
- The compositor's dot shows the recording as for the screen. `record` captures through the surface, not the compositor, so `wm` draws a red dot in the window's title while it lends a lease to a recorder.

## Acceptance criteria

QEMU `wm` suite: `record -w -t 2 data/win.avi` for the clock's window. The AVI's frames have the window's content size (320 × 176), and the dot is in the clock window's title while it records. For a text window, `top`, the frames are its cells drawn (the title text found in a decoded frame).

## Related

[093](../issues-done/093-screen-recording.done), [088](../issues-done/088-text-window-manager.done), [157](../issues-done/157-window-broker.done), [165](../issues-done/165-display-client-for-programs.done).
