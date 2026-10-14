# 000-APP-0072 — Browsing for the background's picture in `wm`'s Settings

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the tools track, 2026-10-14. The maintainer: "When selecting an image for the background we need a browse menu."

## Problem

The background's `Image file` row in Settings ([000-APP-0048](000-APP-0048-wm-settings.md), [000-APP-0050](../issues-done/000-APP-0050-wm-background-pictures-and-pattern-settings.done)) takes only a typed path. One has to know the file's name and directory, and a typo leaves the pattern with a notice.

## Plan

- `wm/src/settings.rs`: a browser over the file system. Enter on an empty `Image file` row, or F3 on it, opens it in the directory of the file named (else `data/`, else the boot disk's root).
  - It lists `..` below a root, then the directories, then the pictures (`.bmp`, `.png`, `.jpg`, `.jpeg`), each sorted. The boot disk's root also lists `ram:`.
  - ↑↓ PgUp PgDn Home End choose. Enter opens a directory, or takes a picture as the background, saved as any other change. Backspace goes up. Esc goes back to the rows.
  - A click on an entry does what Enter does on it; a click outside the window goes back. A click on the empty row opens the browser.
  - The line under the list says how many pictures are here, or why the directory cannot be read.
- `wm/src/main.rs`: the browser reads directories through `mind::fs::list`. `wm` holds the boot disk's files already (`REQUEST_FILES`), so no new authority is asked for.
- `wm`'s status line names the directory and the entry chosen (`BROWSE=<dir>:<name>`), for the tests.
- `docs/tools` (EN, RU).

## Acceptance criteria

- `tests/wm_host.rs`:
  - Enter on the empty row opens the browser in `data/`, with `..`, a directory and the pictures, without other files;
  - a directory opens, `..` comes back with the cursor on it, and `ram:` is listed at the root;
  - Enter on a picture sets the image and its file, and saves it;
  - Esc goes back; a click on an entry acts as Enter, outside the window goes back, on the empty row browses;
  - a directory that cannot be read says so.
- The `wm` suite: Enter on the empty `Image file` row opens the browser at `data/`; Esc goes back; F3 opens it again; Enter on `sky.jpg` sets `background = image data/sky.jpg`; the picture row puts the setting back.

## Related

[000-APP-0048](000-APP-0048-wm-settings.md) (Settings), [000-APP-0050](../issues-done/000-APP-0050-wm-background-pictures-and-pattern-settings.done) (pictures), [000-APP-0056](../issues-done/000-APP-0056-every-entry-of-wms-menu-reacts.done) (every row reacts).
