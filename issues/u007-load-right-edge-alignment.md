# u007 — load: the graphs' right edges and scale labels are not aligned

**Type:** bug · **Owner:** tools track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** —

## Problem

Reported by the user (2026-10-05, screenshot of `load` in a wm window, 4 CPUs): the right side of the graphs is ragged. Each graph ends, and its scale label (`100%`, `50 000/s`, `500/s`, `5 000/s`, `64.0M`, `32`) sits, at a different column: the label is right-aligned to the window's edge, but the graph's right border (`│`) is placed just left of the label, so a longer label (`50 000/s`) pushes its graph's end further left than a short one (`32`, `100%`). The `tasks` graph also starts a column later than the others.

## Plan

One layout for every graph in `monitor/src/load.rs` (or wherever `load` draws): the width of the widest scale label is reserved on the right for all rows; every graph ends at the same column and every label is right-aligned in that reserved area; every graph starts at the same column.

## Acceptance criteria

In `load` at any window or screen width, all graphs start and end at the same columns and the scale labels are right-aligned in one column; a host test (`tests/monitor_host.rs`) checks the columns for labels of different widths.

## Related

[075](../issues-done/075-stat-fields-for-the-monitors.done), [u001](../issues-done/u001-mouse-in-windows-and-fm.done).
