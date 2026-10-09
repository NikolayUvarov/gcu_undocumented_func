# 211-APP-0037 — `wm` marks the window it drags or resizes

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) (for [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md)) · **Roadmap:** track G · **Constitution:** MC-11.5

Numbered from the kernel track's request in `requests-APP.md` (2026-10-09, at the maintainer's request after a run on the MacBook Pro). That file went once every request in it was numbered.

## Problem

The trackpad drags a window by its title in two ways: a press held while a finger moves, and a double tap that keeps the button down until the next tap (211-DRV-0018). While a window moves, nothing shows that `wm` holds it, so the user cannot tell whether the drag took.

## Plan

- While `wm` drags or resizes a window by the pointer, the window is marked until the button is released: its frame drawn double, and its title bar in the focus colour inverted.
- The mark is quiet, and the same for a mouse and a trackpad.
- `wm`'s state line names the drag (`DRAG=1`), so tests can follow it.

## Acceptance criteria

- **Host tests (`tests/wm_host.rs`):** the frame and title of a window being dragged and resized, and both again after the release.
- **The `wm` suite:** it starts a drag by a title with the tablet, sees the mark while the button is held and its absence after the release.

## Related

[211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md), [088](../issues-done/088-text-window-manager.done), [u002](../issues-done/u002-restore-and-unsnap-windows.done).
