# 034 — Text UI library `mind::tui`

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 033, 035 · **Roadmap:** track G, tools plan F3

## Problem

Every program draws pixels by hand. The file manager, editor, viewer and monitors need a common text UI: a cell grid, colours, frames, lists, menus, dialogs, an input line and graphs.

## Plan

- `libmind/src/tui/`: `Grid` of cells (character, foreground, background) in a page block, a shadow copy and a renderer that draws only changed cells into `Screen` with the 8×16 font; cursor.
- Palettes: classic blue (Norton style) and dark.
- Drawing helpers: text with clipping, frames (single/double lines), fill, horizontal and vertical bars, a time-series graph from block elements.
- Widgets: scrollable list with selection, menu bar with drop-down menus, message/confirm/input dialogs, input line with editing and history, status line and F-key bar, progress bar.
- `tui::App` loop helper: draw, wait for a key event or a timeout, dispatch.
- Host tests render into a grid in memory and compare text.

## Acceptance criteria

- Host tests for grid diffing, clipping, frames, list scrolling and the input line.
- Used by `view` (037).

## Related

[docs/tools](../docs/tools/README.md) F3.
