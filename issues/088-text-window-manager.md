# 088 — `wm`: text window manager

**Type:** tools (application + `mind::tui` + launch grants) · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — for keyboard control; 156 (PS/2 mouse) for dragging with the mouse · **Roadmap:** track G · **Constitution:** MC-3.3, MC-3.11, MC-2.6, Appendix B.2

## Problem

Every program has a whole screen of its own, and only the focused one is shown. There is no way to see a clock, the file manager and `top` side by side, or to move and resize what is shown.

## Plan

- **`wm` is an application**, started from the shell like `fm`. It is not a service and not part of the compositor. It owns one screen (its own task screen) and draws a desktop of text windows on it with `mind::tui`. Each window has:
  - a frame, a title, a close mark and a resize corner;
  - a place in the z-order and the keyboard focus.
- **Windows:**
  - open, close, focus next/previous;
  - move and resize with the keyboard (a move/resize mode with the arrows) and, once 156 is done, by dragging the title or the corner with the mouse;
  - maximize and restore.
- **Snapping.** A window moved or dragged within two cells of a screen edge sticks to it. At a corner it takes that quarter of the screen, at the top edge the whole screen, at a side edge half of it. Keys: Alt+arrows for halves, Alt+1…4 for quarters, Alt+Enter to maximize.
- **Programs in windows.** `wm` starts programs through its own launch session (`idl/loader.wit`), and a program in a window gets, instead of a screen:
  1. **a text surface:** a cell grid in memory `wm` lends (`SHARE_RW` adapter: `wm` copies the cells out before drawing them and checks the size, MC-2.11), plus a small header with the grid size and a change counter;
  2. **an input endpoint:** `wm` forwards key (and mouse) events to the focused window's program there and nowhere else.
  
  `mind::tui::Terminal` gets a backend for the surface, and `mind::input` gets an event source for the endpoint. Programs built on them — `fm`, `top`/`memmap`/`load`/`hw`, `view`, `edit`, `dmesg`-like console views — run in a window unchanged. A resize tells the program its new grid size.
- **Pixel programs** (`clock`, `dzen-clock`, the `gfx::Screen` demos) cannot be shown in a text window. `clock` gets a text mode (a TUI clock face) for `wm`. Others start full-screen as before, and `wm` says so.
- **Authority (MC-3.11).**
  - `wm` lends a program in a window only what `wm` itself holds and the program asks for. It asks the shell for those grants when it starts (for example, `REQUEST_FILES` to run `fm` with the user's files).
  - Nothing is granted by program name.
  - A program in a window cannot read another window's surface or keys.
- **Fixed slots.** Two application slots for the surface and the input endpoint. Slots 8 and 9 are free in applications (in the shell they hold the input privilege and COM1). The loader accepts them in a launch session. This is an ABI change in `common/abi.rs`, done with the kernel owner.
- **Exit.** A program that ends closes its window. Closing a window ends its program (through `wm`'s lifecycle client, or by revoking its surface and endpoint and letting it exit). Leaving `wm` ends the programs it started.

## Acceptance criteria

QEMU suite (keys sent over the UART, and `sendkey`):
- `wm` opens `top` and `fm` in two windows.
- Moving a window to the left edge with the keyboard snaps it to the left half, and to a corner a quarter (a `screendump` shows the frames where expected).
- Keys go only to the focused window; switching focus moves them.
- With 156: dragging a title with QEMU's mouse moves the window and snaps it at an edge.
- `clock` in text mode updates in its window.
- Closing a window ends its program (`ps`).
- A program in a window gets no capability `wm` does not hold.
- Host tests: the window geometry (move, resize, snap, z-order) and the surface backend of `mind::tui` render into a grid.

## Related

[155](155-virtual-consoles.md) (virtual consoles: full-screen consoles switched with Alt+F1…F4, which complements windows), [156](156-ps2-mouse.md), [063](../issues-done/063-file-manager-read-only.done), [054](../issues-done/054-tui-library.done), [docs/tools](../docs/tools/README.md).
