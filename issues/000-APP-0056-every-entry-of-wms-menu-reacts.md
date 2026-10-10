# 000-APP-0056 — Every entry of `wm`'s menu and Settings reacts

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** in progress (written; its checks in the `wm` suite next) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

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

## Progress

- **The audit** found these silent:
  - the menu's `Looking for programs…` and `No programs found`;
  - the top bar's next, move, close and full items, and their Alt keys, with no window (Alt+Tab with one);
  - Enter on the system pages that are only to read;
  - a background row that cannot change: the image's file not typed, a number at its end.

  Program entries already said `Started …` or why not on the status line, and the date page's rows set the clock.
- **Each of them now says why** on the status line or on its page:
  - menu entries that start nothing carry their reason (`menu::Item::note`);
  - `wm`'s window keys name the missing window;
  - the pages to read begin with `To read:` and answer Enter and a click;
  - a background row that stays says why under the rows.
- **Host tests** (`tests/wm_host.rs`) walk every entry of the menus, every item of the top bar and the window keys, and every page and row of Settings, with Enter and with a click.
- **The `wm` suite's walks** are written:
  - `every_entry_reacts` starts each entry of the menu on the disk and closes its windows;
  - `settings_react` changes each background row and back, presses Enter on each page to read, and declines a clock set from the date page.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
