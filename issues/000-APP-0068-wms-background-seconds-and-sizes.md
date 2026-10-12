# 000-APP-0068 — `wm`'s background: the seconds apart from the time, and the sizes of the time and of the other lines

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the maintainer, 2026-10-12: "in the clock on the screen add settings: the time and the seconds shown separately; add a way to control the size of the elements."

## Problem

`wm`'s background ([000-APP-0064](000-APP-0064-wms-background-information-in-one-column.md)) shows the time always with its seconds. Its digits and the other lines have fixed sizes: six and two screen pixels a font pixel at wm's scale.

## Plan

- **The seconds:**
  - `show` takes `seconds`, in the default; the time is `HH:MM:SS` with it, `HH:MM` without;
  - a `wm.conf` that never names it was written before it could be chosen, and shows them, as from 000-APP-0064 on;
  - Settings' background page gets the row "Seconds".
- **The sizes,** in percent of the first size, at wm's scale:
  - `time-size` (50, 75, 100, 125, 150, 200, 250, 300) for the time's digits;
  - `text-size` (50, 100, 150, 200, 250, 300) for the date, the CPU line and graph, the memory and the network;
  - Settings' rows "Time size" and "Text size" step them with ← →.
- The CPU graph is 24 glyphs of the other lines wide (as wide as `HH:MM:SS` at the first sizes), so it follows `text-size`.

## Acceptance criteria

- `tests/wm_host.rs`:
  - `seconds` and the sizes are read and written, and an old file shows the seconds; a size not offered is refused;
  - the time without its seconds is five glyphs wide; each size gives the glyph scale asked; the graph follows `text-size`;
  - Settings' rows step and tick them.
- The QEMU `wm` suite: the background's line names `seconds`, `time-size` and `text-size`, and Settings' rows walk includes the three new rows.
- `docs/tools` (EN, RU).

## Related

[000-APP-0064](000-APP-0064-wms-background-information-in-one-column.md), [000-APP-0061](../issues-done/000-APP-0061-wms-scale.done) (wm's scale), 000-APP-0069 (an analog face and gauges).
