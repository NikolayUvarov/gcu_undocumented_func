# 085 — `keymap`: keyboard layout and switch key

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** [151](151-shell-grant-slots-13-15.md) · **Roadmap:** track G, T3

## Problem

The PS/2 layouts (US, Russian ЙЦУКЕН) and the switch (Ctrl+Shift or Alt+Shift) are fixed in `ps2_kbd` ([docs/tools](../docs/tools/README.md) §2.3). Nothing can change them: `ps2_kbd` serves no requests.

## Plan

- `idl/keyboard.wit` 1.0, served by `ps2_kbd` on the endpoint `init` gives it (151): `layout() -> layout`, `set-layout(layout)`, `switch() -> switch-key`, `set-switch(switch-key)` (Ctrl+Shift, Alt+Shift, Caps Lock, none), `layouts() -> list<string<16>, 8>`.
- `keymap [us|ru] [--switch ctrl-shift|alt-shift|caps|none]`: without arguments prints the state; a shell command using the shell's keyboard client (slot 14, 151).
- QEMU `keys` suite: `keymap ru` makes the next PS/2 key give Cyrillic; `keymap --switch caps` switches with Caps Lock.

## Acceptance criteria

- The test above passes; README and `docs/tools` describe `keymap`.

## Related

[151](151-shell-grant-slots-13-15.md), [055](../issues-done/055-key-events.done).
