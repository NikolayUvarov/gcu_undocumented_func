# 000-APP-0071 — `Alt+S settings` stays on `wm`'s top bar longest, `more ▼` left of it

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the tools track, 2026-10-14. The maintainer: "When items are pushed out of the top menu into the drop-down, Alt+S has to stay visible as long as possible, and the menu of the pushed-out items has to be left of it."

## Problem

When the top bar has no room for every item (a larger scale, or the title of the window in front), the items with a key leave from the right into `more ▼` ([000-APP-0061](../issues-done/000-APP-0061-wms-scale.done)). `Alt+S settings` is the rightmost, so it leaves first, and Settings are reached through the menu whenever a window is in front.

## Plan

- `wm/src/desk.rs`, `bar_layout`: the other items with a key leave first, from the right; `Alt+S settings` leaves last. On the bar, `more ▼` comes after the items that stay and before `Alt+S settings`, at the bar's end (left of the window's title and the battery's item).
- `docs/tools` (EN, RU).

## Acceptance criteria

- `tests/wm_host.rs`: at 160 cells with a title and at 106, 84 and 60 cells, Settings stay on the bar right of `more ▼`, and the menu holds the others; a click on Settings opens them; `Alt+0 fit` from the menu still fits a window.
- The `wm` suite: with a window in front, `more ▼` is left of `Alt+S settings`; its menu holds `Alt+0 fit` and not Settings; a click on Settings opens them.

## Related

[000-APP-0061](../issues-done/000-APP-0061-wms-scale.done) (the scale and `more ▼`), [000-APP-0048](000-APP-0048-wm-settings.md).
