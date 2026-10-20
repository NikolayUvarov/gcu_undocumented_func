# 000-APP-0091 — `wm`'s moving background drawn by rows, paced to a deadline

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1

**Recorded by:** the kernel track (KRN), 2026-10-11, in `requests-APP.md`, at the maintainer's request: "the moving background jerks when the bright screen is on, and redrawing is slow". The kernel's part is [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md). Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

On the MacBook Pro's 2880 × 1800 screen, by KRN's reading of the code:

- **`paint()`** copies every background cell one pixel at a time, through `backdrop.pixel()` with a division and bounds checks per pixel: about 50–90 ms a frame;
- **`pattern()`** computes 324 000 blocks a frame, with divisions per block, and a square root per centre for rings: about 5–85 ms;
- **pacing:** a fixed sleep after the work, so a slow frame makes the phase jump;
- **mouse moves** redraw everything;
- at high contrast more pixels change each frame.

So `wm` runs at about 5–11 frames a second, unevenly.

## Plan

- **`paint()` by rows:** runs of background cells filled row by row, one loop for each unit size, no division.
- **A deadline:** sleep until the next frame's time, not a fixed 30 ms after the work; a finer phase, so every term moves every frame.
- **A cheaper pattern:** per-column and per-row terms once a frame, and incremental distances instead of the square root.
- **Only what changed:** cells whose colour did not change are not repainted; a mouse move redraws the cursor's surroundings only.
- **`wm` records the time a frame takes** (`[WM] BACKGROUND FRAME … MS`, at most once a minute).

## Acceptance criteria

- **A host test:** the row painter gives the same pixels as the cell painter, for every unit size and pattern.
- **The `wm` suite** reads the frame time in QEMU.
- **On the MacBook Pro** with 000-KRN-0081, the moving background runs evenly at full contrast.

## Related

[000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md), [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done) (the background).
