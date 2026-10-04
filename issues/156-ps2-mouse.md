# 156 — PS/2 mouse: pointer events for the focused program

**Type:** driver + kernel · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (needed by 088) · **Constitution:** MC-3.3, Appendix B.6

## Problem

There is no pointing device. The text window manager (088) needs to drag windows by their title and corner, and `fm` and `edit` could use clicks.

## Plan

- **`ps2_kbd` drives the auxiliary port of the i8042 too.** It enables the mouse, takes IRQ 12 from `init` as a second line on its service endpoint, and decodes the 3-byte packets (buttons and relative movement; the wheel with the IntelliMouse handshake when present).
- **Pointer events go to the focused task** like keys, through the kernel's input path: a new event kind in the input event word (buttons, dx, dy, wheel). Programs that do not ask for pointer events never see them. `mind::input` and `mind::keys::Event` gain a pointer variant.
- **Positions.** A text program gets cell positions from `mind::tui`, which accumulates movement within its grid. `wm` gets raw movement and keeps its own pointer.
- **Legacy.** The i8042 is legacy hardware, so the code is marked `LEGACY:` and listed in `docs/legacy.md`. USB HID through xHCI is the modern path, later.

## Acceptance criteria

- QEMU `keys` suite: mouse movement and clicks sent with QEMU's `mouse_move` and `mouse_button` arrive at a focused test program as pointer events with the right buttons and deltas.
- The keyboard keeps working.
- An unfocused program gets nothing.

## Related

[088](088-text-window-manager.md), [085](../issues-done/085-keymap.done), [docs/legacy.md](../docs/legacy.md).
