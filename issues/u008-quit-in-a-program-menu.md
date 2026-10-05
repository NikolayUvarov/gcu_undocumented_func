# u008 — the Quit item of a program's menu does not work

**Type:** bug · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** —

## Problem

Reported by the user (2026-10-05): the **Quit** button in a program's menu does nothing. The report does not name the program; the candidates with a Quit entry:
- `edit`: the File menu (F9) has `Quit  F10` (`FILE_ITEMS` in `edit/src/editor.rs`);
- the key bars with `10 Quit` (fm, edit, the viewer), clicked with the mouse (u001), in a window and on a full screen.

## Plan

Check each: chosen with the keyboard (Enter) and with the mouse (a click), in a wm window and on a full screen, with and without unsaved changes in `edit` (where Quit should ask first). Fix what does not end the program, and make the item and the key bar's button do the same as F10.

## Acceptance criteria

The tools or wm suite chooses Quit from edit's File menu and clicks `10 Quit` in fm and in the viewer, in a window and on a full screen; each program ends (or `edit` asks about unsaved changes).

## Related

[u001](../issues-done/u001-mouse-in-windows-and-fm.done), [088](../issues-done/088-text-window-manager.done).
