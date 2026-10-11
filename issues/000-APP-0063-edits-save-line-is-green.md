# 000-APP-0063 — `edit`'s line under the text: green for a save, red for a problem

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the maintainer, 2026-10-11: "the editor inside fm shows a red status line at the bottom when it saves, but that is a successful operation and is better highlighted in green."

## Problem

`edit`, on its own and inside `fm` (F4), draws every notice on the line under the text in the theme's `error` style, white on red. That includes `Saved 12 bytes` and `Replaced 3`, which report something done. A save then looks like a failure.

## Plan

- **The theme.** `mind::tui::Theme` gets an `ok` style for what was done:
  - `CLASSIC`: black on green;
  - `DARK`: dark on light green.
- **The editor.** It records whether its notice says what was done:
  - saved and replaced are drawn in `ok`;
  - not saved, not found, read-only, too large and a wrong line number stay in `error`.

## Acceptance criteria

- `tests/edit_host.rs`: after a failed save the line is red, after a save it is green, and a search that finds nothing is red again.
- `edit` and `fm` build for x86 and aarch64.

## Related

- 000-APP-0062: `fm`'s new directories, reported at the same time.
