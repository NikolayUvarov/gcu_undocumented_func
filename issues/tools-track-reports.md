# Reports for the tools track (not numbered)

**Owner:** tools track · **Status:** open · **Recorded by:** the porting track, 2026-10-05

The user reported these while the porting track was working and asked that the tools track take them. The tools
track numbers its own issues (`uNNN`, a counter no other track uses), so they are not numbered here: the tools track
turns each into a `uNNN` issue (or fixes it) and removes it from this file; the file goes when it is empty. What the
porting track found while looking is kept under each.

## beep started from wm's desktop menu does not work

### Problem

Reported by the user (2026-10-05, screenshot): in `wm`, the desktop menu → **Sound and voice** → **beep** does nothing audible. `beep` asks for a console (`REQUEST_CONSOLE`), so the menu runs it as `console beep` (`wm::menu::catalogue`); without arguments it plays the gateway demo (a chord and a sweep, about 1 s) and ends.

What is known (porting track, while merging main):
- `beep 440 100` from the shell plays (`[BEEP] DEVICE=true RATE=48000`, `PLAYED 1 NOTES`).
- The loader always lends `SLOT_AUDIO` (`loader/src/main.rs`, the standard grants), so a child of `console` has the audio client too.
- `run console beep 440 100 &` from the shell: `console` started; its child's lines were not found with `dmesg -s beep` afterwards. Not yet checked: whether the sound plays, whether `console` shows beep's output, whether the console window closes at once when the program ends (the user may see nothing because it opens and closes within a second).

### Plan

- Reproduce through the menu (the tablet suite's `tablet_at`/`tablet_click`, or Alt+P and keys), with `-audiodev wav` to see whether samples reach the device.
- Find which link drops it: wm's `launch("console beep")`, console's start of its child, beep's audio calls, or the window going away before anything is visible.
- A program that ends quickly should leave its console window with its output and an "ended" line, so the user sees that it ran.

### Acceptance criteria

The tablet or wm suite starts `beep` from the menu and checks that it played (the WAV file has the tones) and that its console window shows its lines.

### Related

[u003](../issues-done/u003-desktop-programs-menu.done), [u004](../issues-done/u004-console.done), [u005](../issues-done/u005-beep-without-a-screen.done).

## The Quit item of a program's menu does not work

### Problem

Reported by the user (2026-10-05): the **Quit** button in a program's menu does nothing. The report does not name the program; the candidates with a Quit entry:
- `edit`: the File menu (F9) has `Quit  F10` (`FILE_ITEMS` in `edit/src/editor.rs`);
- the key bars with `10 Quit` (fm, edit, the viewer), clicked with the mouse (u001), in a window and on a full screen.

### Plan

Check each: chosen with the keyboard (Enter) and with the mouse (a click), in a wm window and on a full screen, with and without unsaved changes in `edit` (where Quit should ask first). Fix what does not end the program, and make the item and the key bar's button do the same as F10.

### Acceptance criteria

The tools or wm suite chooses Quit from edit's File menu and clicks `10 Quit` in fm and in the viewer, in a window and on a full screen; each program ends (or `edit` asks about unsaved changes).

### Related

[u001](../issues-done/u001-mouse-in-windows-and-fm.done), [088](../issues-done/088-text-window-manager.done).
