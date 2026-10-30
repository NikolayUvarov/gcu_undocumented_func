# 000-APP-0118 — Settings: Enter on the picture's own file, or a file wm cannot show, offers the choice of another

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open (written and host-tested; the maintainer's check on a machine left) · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-3.1, MC-12.3

**Recorded by:** the tools track, 2026-10-30, from the maintainer's report.

## Problem

- **The report (the maintainer, 2026-10-30):** "Choosing the wallpaper, when a file name is chosen already, Enter again gives an error. A picture can be chosen only after erasing the whole name, which is inconvenient. If there is an error, offer the choice of a file."
- **What it was.** On the background page of `wm`'s Settings, the Image file row shows the picture's file. Enter there with that name answered `… is the picture already`. The picture browser of [000-APP-0072](../issues-done/000-APP-0072-browse-for-the-background-picture.done) opened only on an empty row or with F3.
- **A file wm could not show** was named only on the status line (`…; the pattern instead`).

## Plan

1. **Enter on the picture's own file** opens the browser in its directory, on that file. The browser's line says why: `<file> is the picture already: choose another, or Esc`.
2. **A click on that row** does the same. A name being typed there still waits for Enter.
3. **A file wm could not read or show,** taken from the row or the browser, opens the browser again while Settings is open. It says what went wrong. A kind stepped on the Picture row keeps the notice alone, so the arrows go on stepping (`wm` suite).
4. **A host test** in `tests/wm_host.rs`.

## Acceptance criteria

- [x] Enter and a click on the picture's own file open the browser in its directory, on the file, with the reason shown; another picture is taken from there (host test `the_picture_already_shown_offers_the_choice_of_another`, 2026-10-30).
- [x] A file wm could not show, from the file's row, offers the browser with its reason; from the Picture row it does not (same test, `Settings::not_shown`).
- [ ] The maintainer checks it on a machine.

## Related

[000-APP-0072](../issues-done/000-APP-0072-browse-for-the-background-picture.done) (the browser), [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done) (the background).
