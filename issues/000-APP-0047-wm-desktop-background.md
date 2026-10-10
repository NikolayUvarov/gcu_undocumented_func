# 000-APP-0047 — `wm`: a configurable desktop background

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 (the maintainer's request, 2026-10-10) · **Status:** open · **Blocked by:** — (the network traffic: a read-only source of a card's counters, see Plan) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

The maintainer's request (2026-10-10): "`wm` needs a configuration of the desktop background: clocks over a picture, abstract drawings that move slowly, a static image, the date, the time, the CPU load in the background, the network traffic. Configurable through a configuration file. By default a slowly moving abstract low-contrast picture with the date, the time and the CPU load over it. The current variant, without anything, must stay." And: "can we have the background in text mode too?"

## Problem

`wm` fills the desktop with a dim `░` pattern and nothing else. The user wants a background chosen in a configuration, with a few kinds of picture and of information over it.

**Text mode.** `wm` is a cell grid drawn with the 8×16 font on the framebuffer (there is no hardware text mode under UEFI), and it already draws pixels between the cells (pixel windows). So a background can be drawn in both ways: pixels on the desktop's free cells (a picture, an abstract pattern), and characters in cells (large block digits, graphs of braille dots, shades).

## Plan

- **The configuration:** `data/wm.conf`, lines of `key = value`, read when `wm` starts and when Settings changes it (000-APP-0048); a missing or unreadable file means the default. `wm` writes it through the user's files client it holds.
  - `background = none | abstract | image <file>`:
    - `none`: the current desktop (the dim `░` pattern), unchanged;
    - `abstract` (the default): a slowly moving, low-contrast pattern in the theme's dark colours, drawn in pixels on the cells no window covers, a few frames a second at most;
    - `image <file>`: a BMP (24-bit, as `screenshot` writes) scaled to the desktop; a file that cannot be read falls back to the default with a notice.
  - `show = date, time, cpu, net` (any of them, or `none`; default `date, time, cpu`): large block digits for the time, the date under them, the CPU load as a graph of the last minute (from the system information `wm` holds, as `load` reads it), and the network traffic.
  - `place = center | top-right | bottom-right | …` for the information (default the lower right part of the desktop).
- **Drawing:** only on the desktop's cells no window covers and not the top bar; the windows are drawn as now. The background is drawn again only when its picture moves, the minute changes or the load is sampled (once a second), so `wm` stays idle otherwise.
- **The network traffic** needs a card's counters. Today they come only through a full client of the driver (`net.wit` `counters`), which `wm` does not hold and should not. A read-only source (a badge of `net.wit` that answers `info` and `counters` alone, lent by the shell for a request) is asked of the track that owns the driver; until then `net` is shown as not available.
- **Docs:** `docs/tools` (EN, RU).

## Acceptance criteria

- **Host tests:** the configuration's parsing (each key, defaults, errors); the abstract pattern's frames stay within the low-contrast range and move; the layout of the information on desktops of several sizes.
- **The `wm` suite:** with no configuration, the desktop shows the abstract pattern (pixels change between two screenshots), the time and the date as the RTC gives them and a CPU graph; with `background = none` it is the `░` pattern as before; with `image` a test BMP's colours are on the desktop.
- `none` keeps every check of the `wm` suite that looks at the desktop as it was.

## Related

[000-APP-0048](000-APP-0048-wm-settings.md) (Settings: the background switched from the top bar), [088](../issues-done/088-text-window-manager.done), [u009](../issues-done/u009-pixel-windows-follow-their-frame.done).
