# 000-APP-0091 — `wm`'s moving background drawn by rows, paced to a deadline

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: in `main` since 835c31c8 (2026-10-20); the MacBook Pro with 000-KRN-0081 left · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1

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

## Progress (2026-10-20)

- **By runs, not by pixels.** `Backdrop::blocks` (`wm/src/background.rs`) gives a rectangle of the screen as one fill for each run of frame pixels of one colour in a band of rows: two divisions a band, none a pixel. `paint` (`wm/src/main.rs`) joins neighbouring cells of a row into one rectangle and fills through `Screen::fill`.
- **To the deadline.** The main loop sleeps until the pattern's next frame is due, 30 ms at most for keys and the mouse, in place of a fixed 30 ms after the work. The phase still moves by the time that passed (`advance`).
- **The frame time** is logged at the first frame and then once a minute: `[WM] BACKGROUND FRAME <avg> MS ON AVERAGE, <max> MS AT MOST, <n> FRAMES IN <s> S (<w>X<h> BLOCKS OF <u> PX)`, from the start of the frame's drawing to its last pixel on the screen.
- **A mouse move** already repaints only the cells under the pointer (since 000-APP-0047's later steps): nothing to change.
- **Host test:** `tests/wm_host.rs`, `the_background_by_runs_is_the_background_by_pixels`: units 1 to 5, a frame narrower than the screen, rectangles that start and end inside blocks, each pixel painted once and as `pixel` gives it.
- **The `wm` suite** reads the frame line after the aurora pattern at speed 20.
- **Not done:** the cheaper pattern (per-column and per-row terms, no square root), a finer phase, and skipping cells whose colour did not change. They change what the pattern draws, so they wait for the MacBook Pro's frame time with this step: if it is even at full contrast, they are not needed.
- **In `main`** since 835c31c8 (2026-10-20). The gate: every x86 group on i9 and every aarch64 group on un-rizen-service (Effector); the host tests, TLC and the x86 groups boot, services, scheduling, files and keys locally. The batch's later commits changed only the suites, and each group ran on a commit that has the suites it runs. The `wm` suite's frame line was read in the keys, shell and tools group locally.
- **Left:** the MacBook Pro with 000-KRN-0081.

## Related

[000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md), [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done) (the background).
