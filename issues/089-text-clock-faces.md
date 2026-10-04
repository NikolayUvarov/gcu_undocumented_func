# 089 — Text faces for `clock` and `dzen-clock`

**Type:** tools (applications) · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — (the window mode needs 088 and 157) · **Roadmap:** track G · **Constitution:** —

## Problem

`clock` and `dzen-clock` draw pixels on a screen of their own (`gfx::Screen`). They cannot run where only text works:
- in a text window when pixel surfaces are not available (088);
- as a console program over COM1;
- in a virtual console's line-oriented view (155).

For compatibility each needs a text face too.

## Plan

- **`clock --text`:** the date and the time with seconds in large digits built from box-drawing and block characters of MIND Mono, sized to the grid it gets, with the weekday and the RTC's date. It runs through `mind::tui`, so it works:
  - on its own screen (text instead of pixels);
  - in a text window of `wm` (resizing picks a smaller or larger digit size);
  - as a console program (`mind::request!(REQUEST_CONSOLE)`, one line updated in place).
- **`dzen-clock --text`:** the five color indicators of the pixel face as text rows. The keys keep their meaning: **D** shows the digital time, **C** and **P** select the orbit (drawn with ring characters on the grid), **H** hides or shows the title and the hints. The display uses colored cells and fills every cycle of the face (`dzen-clock/src/cycle.rs` and `face.rs` stay the shared logic; only `view.rs` gets a text renderer).
- **Choosing the face:** the face is chosen at start (`--text`, or automatically when the program has no pixel screen: in a text window, or as a console program). The pixel faces stay the default on a screen of their own.

## Acceptance criteria

- QEMU `dzen` suite:
  - `dzen-clock --text` shows the indicators and reacts to D/C/P/H like the pixel face; a `screendump` shows the text cells;
  - `clock --text` shows the RTC time and changes every second;
  - as console programs, both print the time to the shell's console.
- Host tests: the text renderers of both faces draw the expected cells for fixed times (the dzen-clock logic is already host-tested).

## Related

[088](088-text-window-manager.md), [155](155-virtual-consoles.md), [157](../issues-done/157-window-broker.done).
