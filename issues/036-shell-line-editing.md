# 036 — Shell: line editing, history, Cyrillic, scrollback

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 033, 035 · **Roadmap:** track G, tools plan §4.8

## Problem

The shell console draws only upper-case ASCII with the 8×8 font, edits a line only with Backspace and keeps no history; Cyrillic typed at the UART is drawn as `?`.

## Plan

- Console on the 8×16 font: lower case, Cyrillic, box drawing.
- Line editor: ←/→, Home/End, Delete, Backspace by character, Ctrl+←/→ by word, insert in the middle; history of 32 lines on ↑/↓; Tab completes the command or the program name (loader `LIST`); Esc clears the line.
- Scrollback of the last screens with Shift+PgUp/PgDn.
- The UART mirror keeps working: the line is redrawn on COM1 with VT100 sequences only when the cursor is not at the end.

## Acceptance criteria

- QEMU suite `keys` (035) edits a command with arrows and recalls it from the history, both from the UART and PS/2.
- A screendump shows Cyrillic in the shell.

## Related

[docs/tools](../docs/tools/README.md) §2.4, §4.8.
