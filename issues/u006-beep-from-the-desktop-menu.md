# u006 — beep started from wm's desktop menu does not work

**Type:** bug · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-3.11

## Problem

Reported by the user (2026-10-05, screenshot): in `wm`, the desktop menu → **Sound and voice** → **beep** does nothing audible. `beep` asks for a console (`REQUEST_CONSOLE`), so the menu runs it as `console beep` (`wm::menu::catalogue`); without arguments it plays the gateway demo (a chord and a sweep, about 1 s) and ends.

What is known (porting track, while merging main):
- `beep 440 100` from the shell plays (`[BEEP] DEVICE=true RATE=48000`, `PLAYED 1 NOTES`).
- The loader always lends `SLOT_AUDIO` (`loader/src/main.rs`, the standard grants), so a child of `console` has the audio client too.
- `run console beep 440 100 &` from the shell: `console` started; its child's lines were not found with `dmesg -s beep` afterwards. Not yet checked: whether the sound plays, whether `console` shows beep's output, whether the console window closes at once when the program ends (the user may see nothing because it opens and closes within a second).

## Plan

- Reproduce through the menu (the tablet suite's `tablet_at`/`tablet_click`, or Alt+P and keys), with `-audiodev wav` to see whether samples reach the device.
- Find which link drops it: wm's `launch("console beep")`, console's start of its child, beep's audio calls, or the window going away before anything is visible.
- A program that ends quickly should leave its console window with its output and an "ended" line, so the user sees that it ran.

## Acceptance criteria

The tablet or wm suite starts `beep` from the menu and checks that it played (the WAV file has the tones) and that its console window shows its lines.

## Related

[u003](../issues-done/u003-desktop-programs-menu.done), [u004](../issues-done/u004-console.done), [u005](../issues-done/u005-beep-without-a-screen.done).
