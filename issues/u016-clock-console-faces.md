# u016 — Console faces of the clocks: the time on one line, updated in place

**Type:** tools · **Owner:** tools track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-10.2

Moved out of [155](../issues-done/155-virtual-consoles.done) (virtual consoles), which is done without it; left from [089](../issues-done/089-text-clock-faces.done).

## Problem

`clock` and `dzen-clock` have a pixel face (a screen, a `wm` pixel window) and a text face (089: a screen or a `wm` text window). Started as console programs of a console — in a console of the shell, in `console` — they have neither: they need a screen of their own.

The shell's console has no way to rewrite a line: `\r` is dropped, so a time printed every second fills the scrollback.

## Plan

- `clock --line` and `dzen-clock --line`: no screen (they print); the time (and for `dzen-clock` the indicators as text) on one line, rewritten every second with `\r`.
- The shell's console and `console` move the cursor to the start of the line on `\r` (the next characters overwrite it); the serial line gets the `\r` as it is.
- How a program asks for no screen when it can also have one: a second program name, or a request read from its arguments (to be decided with the loader's `needs`).

## Acceptance criteria

QEMU `shell` suite: `clock --line` for 3 s leaves one line of time in the shell's text, not three; the serial line shows the updates. Host test: `\r` in the shell's console and `console`'s screen.

## Related

[089](../issues-done/089-text-clock-faces.done), [155](../issues-done/155-virtual-consoles.done), [u004](../issues-done/u004-console.done).
