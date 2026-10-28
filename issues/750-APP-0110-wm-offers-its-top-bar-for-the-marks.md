# 750-APP-0110 — `wm` offers the compositor cells of its top bar for the marks; the shell's consoles later

**Type:** tools (`wm`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** `display.wit` 1.3 `offer-marks` and the compositor's part (`KRN`, from EFF's request; not numbered yet) · **Main task:** [750](750-effector-agent.md) · **Roadmap:** track G · **Constitution:** MC-3.8, MC-10.2, MC-12.4

**Recorded by:** the tools track, 2026-10-28, from EFF's request `ALL/2026-10-24T1654Z-EFF-a-request-the-program-in-front-offers-a-place-for-the-marks.md` on `internal-dev-messages`, which passes on the maintainer's choice of 2026-10-24.

## Problem

- **The fault.** The compositor draws the remote mark ([800-KRN-0156](800-KRN-0156-the-remote-marks-in-the-compositor.md)), the capture dot and the camera mark at fixed pixels. In some of `wm`'s scales they lie over the battery chip ([211-APP-0070](211-APP-0070-the-battery-in-wm-and-the-shell.md)).
- **The maintainer's choice.** The program in front leaves room for the marks, and the compositor draws them there; with no room offered, in the corners over the picture. A fixed shift and a strip kept for the marks were turned down.
- **What `wm` has to do.** It alone knows its bar's layout and scale, so it says where the room is.

## Plan

**1. `wm`'s top bar.**

- **Two empty cells kept** in the bar:
  - at the left, before `wm`, for the remote mark;
  - at the right, between the window title and the battery chip, for the capture dot and the camera mark side by side.
- **The offer.** `wm` gives the compositor those cells' pixel rectangles with `offer-marks(remote, capture)` (`display.wit` 1.3):
  - at its start;
  - again whenever the bar's layout or the scale changes (the battery chip going from `87%` to `100%` or to the minutes; a new scale from Settings);
  - `none` when its screen leaves the front is not needed: the compositor ends the offer itself.
- **A refusal** (`invalid-place`) is logged with the rectangles, and `wm` goes on: the compositor then uses the corners.
- **Programs in `wm`'s windows** are on `wm`'s screen and need nothing.

**2. The shell's full-screen consoles** (not urgent, as EFF says): the last cells of the first row kept and offered while the console is in front; until then the marks stay in the corners over its text.

## Acceptance criteria

- **Host tests** (`wm`'s): the cells' rectangles from the layout at every scale `wm` offers (100 to 600 %), with and without the battery chip, the chip's own cells never inside them.
- **The `wm` suite** (x86; aarch64 where it runs): the compositor's log says `MARKS IN THE PLACES PID <wm> OFFERED`; a screenshot shows the battery chip's pixels unchanged with a mark lit; after a scale change, a new offer.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

What `wm` offers is only where the marks go: the compositor still draws every lit mark (MC-3.8).

## Related

[800-KRN-0156](800-KRN-0156-the-remote-marks-in-the-compositor.md), [750-EFF-0015](750-EFF-0015-the-agents-mark-on-the-screen.md), [211-APP-0070](211-APP-0070-the-battery-in-wm-and-the-shell.md), [000-APP-0061](../issues-done/000-APP-0061-wms-scale.done) (`wm`'s scale).
