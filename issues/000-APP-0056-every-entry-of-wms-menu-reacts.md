# 000-APP-0056 — Every entry of `wm`'s menu and Settings reacts

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request: "all commands in `wm`'s menu must react adequately."

## Problem

An entry that does nothing on Enter or a click leaves the user guessing whether the system hung. The date page above is one case, and others may be like it.

## Plan

- **An audit.** Go through every menu entry, every Settings page and row, and the right-click menu. Each must do one of these on Enter and on a click:
  - open its program (in a window, or in `console` for a console program);
  - change its setting visibly;
  - say why it cannot (the device is missing, the program is not on the disk).
- **The `wm` suite** walks them all. For each entry it checks that something visible happened: a window, a line in `console`, a changed row, or the reason.
- **Text-only pages** say so in their title, or become settable as above.

## Acceptance criteria

- The `wm` suite opens each menu entry and each Settings row and finds its reaction.
- No entry is silent.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
