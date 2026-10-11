# 000-APP-0061 — `wm`'s scale and pointer: auto on each start, 100 to 600% from Settings, the pointer's size and colour

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5, MC-12.3

**Recorded by:** the maintainer, 2026-10-11: "Scaling wm's output. On high-resolution screens everything is tiny and hard to hit with a trackpad. Add scaling and its configuration to wm. The default is auto (pick a sensible scale on the first start); offer the user 100 150 200 250 300 400 (and higher if reasonable)." And then: "about the mouse pointer: its size and colour must change too."

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
- **The pointer.** Its size and colour are set on the same page and kept as `pointer` and `pointer-colour`:
  - `auto`, the default, draws the 8 × 12 arrow at the scale;
  - 100 to 800% draw it at that size whatever the scale;
  - the colours are white (the default), black, yellow, orange, red, green, cyan and magenta, each with an edge that
    shows on the desktop.

  `wm` logs the size and colour at the start and after each change.
- **The top bar.** Its items take 160 cells with their keys, and the title of the window in front used to cover the
  last ones. The maintainer's further request: "when there is no room, the top bar can fold into a drop-down menu
  `more`, into which the commands with hotkeys are pushed first." Now:
  - when every item fits beside the title, all are on the bar;
  - when not, the items with a key leave first, from the right, into the menu of the bar's last item, `more ▼`;
  - the menu does what the item's key does (↑↓ and Enter, or a click);
  - the title takes at most 24 cells.

## Acceptance criteria

- `tests/wm_host.rs` covers:
  - `auto` on the screens above;
  - the scales offered on each;
  - `resolve`, and its fallback for a scale that does not fit;
  - the configuration's `scale` key, read and written;
  - `Desk::rescale` keeping the frames' places and shares;
  - the Screen page's steps and its text;
  - `Wm::rescale`;
  - the pointer's sizes, colours and pixels, its keys in `data/wm.conf` and its rows on the Screen page;
  - the top bar on 160 cells with and without a title, on 106 and on 84: what is on it and what is in `more ▼`'s
    menu;
  - the menu's keys and clicks;
  - every item reached from the bar or the menu with 0, 1 and 4 windows.
- The QEMU `wm` suite (1280 × 800) checks:
  - `[WM] SCALE 100% (AUTO)` at the start;
  - from the Screen page, the steps auto → 100% → 150%, with the log line, a grid of 106 × 33 cells of 12 × 24 pixels read off the screen, and the frames keeping their share;
  - the steps back to auto, each saved in `data/wm.conf`;
  - the pointer set to 200% and yellow, its fill (108 pixels in a 12 × 18 box) read off the screen, then back to auto
    and white;
  - at 150%, `more ▼` on the top bar.
- `docs/tools` (EN and RU) describe the scale, the pointer and their keys.

## Related

- 000-APP-0047, 000-APP-0050: the background and `data/wm.conf`.
- 000-APP-0048: Settings.
- The maintainer's MacBook Pro (2880 × 1800), where the grid was too fine.
