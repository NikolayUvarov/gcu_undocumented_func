# 042 — TUI library `mind::tui` (tools F3)

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Roadmap:** track G, T0 · **Blocked by:** 040, 041

## Problem

Every tool would otherwise draw cells, frames, lists and dialogs by hand.

## Plan

- A cell grid with colours, frames, lists, panels, an input line and dialogs, rendered into a screen buffer; damage tracking.

## Acceptance criteria

- Host tests render into a grid and compare cells.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F3.
