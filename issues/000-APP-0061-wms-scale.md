# 000-APP-0061 — `wm`'s scale: auto on each start, 100 to 600% from Settings

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5, MC-12.3

**Recorded by:** the maintainer, 2026-10-11: "Scaling wm's output. On high-resolution screens everything is tiny and hard to hit with a trackpad. Add scaling and its configuration to wm. The default is auto (pick a sensible scale on the first start); offer the user 100 150 200 250 300 400 (and higher if reasonable)."

## Problem

`wm` draws its whole screen as a grid of 8 × 16-pixel cells, and its pointer is 8 × 12 pixels. On the MacBook Pro's 2880 × 1800 screen that is a grid of 360 × 112 cells: text one can hardly read and targets a trackpad hardly hits. Nothing in `wm` changes the size.

## Plan

- **The cell grows.** At scale `s` a cell is `8·s/100 × 16·s/100` pixels, and the 8 × 16 font is stretched over it. `libmind`'s `Terminal::scaled` and `Screen::glyph_cell` do this, and the pointer's cell follows it in `input`. `Terminal::new` stays at 8 × 16 for every other program.
- **What `wm` draws grows with it.** A pixel window keeps its content of 8 × 16 pixels a cell, and `wm` stretches it, so programs do not change. The pointer and the background's lines grow too.
- **The scales.** They are 100, 150, 200, 250, 300, 400, 500 and 600%, each a whole number of pixels a cell. Settings offers the ones that leave 84 × 26 cells or more, room for its own window and the bars around it.
- **`auto`, the default.** At each start it takes the largest scale that leaves at least 1280 × 720 pixels at 100%:
  - 1280 × 800 gets 100%;
  - 1920 × 1080 gets 150%;
  - 2880 × 1800 gets 200%;
  - 3840 × 2160 gets 300%.
- **The configuration.** `scale = auto | N` lives in `data/wm.conf`. Settings' Screen page steps through auto and the offered scales. The change is used at once:
  - the grid is made anew;
  - each frame keeps its place and share of the screen;
  - the change is saved.

  A scale chosen on another screen that does not fit here falls back to auto.
- **The log.** `wm` logs the scale it uses.

## Acceptance criteria

- `tests/wm_host.rs` covers:
  - `auto` on the screens above;
  - the scales offered on each;
  - `resolve`, and its fallback for a scale that does not fit;
  - the configuration's `scale` key, read and written;
  - `Desk::rescale` keeping the frames' places and shares;
  - the Screen page's steps and its text;
  - `Wm::rescale`.
- The QEMU `wm` suite (1280 × 800) checks:
  - `[WM] SCALE 100% (AUTO)` at the start;
  - from the Screen page, the steps auto → 100% → 150%, with the log line, a grid of 106 × 33 cells of 12 × 24 pixels read off the screen, and the frames keeping their share;
  - the steps back to auto, each saved in `data/wm.conf`.
- `docs/tools` (EN and RU) describe the scale and the `scale` key.

## Related

- 000-APP-0047, 000-APP-0050: the background and `data/wm.conf`.
- 000-APP-0048: Settings.
- The maintainer's MacBook Pro (2880 × 1800), where the grid was too fine.
