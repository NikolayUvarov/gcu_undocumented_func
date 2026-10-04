# 088 — `wm`: window manager for text and pixel programs

**Type:** tools (application, `mind::tui` and `gfx::Screen` backends, launch grants) · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** 157 (window broker); 156 (PS/2 mouse) for dragging with the mouse · **Roadmap:** track G · **Constitution:** MC-2.6, MC-2.11, MC-3.3, MC-3.11

## Problem

Every program has a whole screen of its own, and only the focused one is shown. There is no way to see the clock, the file manager and `top` side by side, or to move and resize what is shown.

## Plan

- **`wm` is an application, not a service.**
  - The shell starts it like `fm`, and it can run in any virtual console (155).
  - It holds only what the shell lends it and passes on only part of that (MC-3.11).
  - It is the *presentation* of windows. The windows themselves are kept by the window broker (157), so programs outlive `wm`. A graphical window manager later is another client of the same broker and protocol.
  - The window protocol (`idl/window.wit` and `mind::window`) is independent of `wm`. Programs do not depend on which manager shows them.
- **Desktop.** `wm` draws on its own screen. Windows sit on the cell grid (8×16): text frames, a title, a close mark and a resize corner, with z-order and keyboard focus. Two kinds of window content:
  - **text:** a cell grid the program draws with `mind::tui`;
  - **pixels:** a buffer of the window's size in the screen's pixel format, which the program draws with `gfx::Screen`.
  
  `wm` copies only the changed rectangle, which the surface header names with a change counter. A full 1280×800 screen is about 4 MiB a frame.
- **Moving and snapping:**
  - Move and resize with the keyboard (a move/resize mode with the arrows) and, with 156, by dragging the title or the corner.
  - A window brought within two cells of a screen edge sticks to it. At a corner it takes that quarter, at a side half the screen, at the top the whole screen.
  - Keys: Alt+arrows for halves, Alt+1…4 for quarters, Alt+Enter maximize/restore.
  - Every change of geometry is saved in the broker.
- **Programs in windows:**
  - A program started in window mode (`REQUEST_WINDOW`) gets a client of the broker in a fixed application slot. It creates its own surface and input endpoint and registers them (157).
  - The backends of `mind::tui::Terminal` and `gfx::Screen` draw into the surface, and `mind::input` reads the window's endpoint. So `fm`, `top`/`memmap`/`load`/`hw`, `view`, `edit` and `clock` run in a window unchanged.
  - A resize tells the program its new size.
  - `wm` forwards keys (and mouse events) only to the focused window's program. A program sees no other window's surface or keys.
  - `wm` starts programs through its own launch session, with only what it holds and the program asks for. It asks the shell for those grants when it starts, for example `REQUEST_FILES` to run `fm` with the user's files.
- **Stopping `wm`:** two ways, the same in a later graphical manager (157):
  - **detach** (the default way out, and what a crash does): `wm` ends, its programs keep running hidden, the broker keeps the layout, and the next `wm` shows the same windows where they were.
  - **close all:** every program gets a close event. Programs that do not end are listed, and `wm` stops them through a lifecycle client only if it holds one.
  
  Closing one window sends its program the close event. A program that ends loses its window.
- **Fixed slots.** The broker client of a program in a window needs one application slot (8 is free in applications; in the shell it is the input privilege). The loader accepts it in a launch session. This is an ABI change in `common/abi.rs`, done with the kernel owner.

## Acceptance criteria

QEMU suite (keys over the UART and `sendkey`, the mouse with `mouse_move`/`mouse_button` once 156 is done):
- `wm` opens `top`, `fm` and `clock` in three windows. A `screendump` shows the text frames, the text content and the clock's pixels.
- Moving a window to the left edge snaps it to the left half, to a corner a quarter. With 156, dragging a title moves and snaps it.
- Keys go only to the focused window.
- Detach: after `wm` ends (and after it is killed), `top` and `clock` still run. A new `wm` shows them at the same places.
- Close all: the programs end (`ps`).
- A program in a window gets no capability `wm` does not hold.
- Host tests:
  - window geometry (move, resize, snap, z-order);
  - the text and pixel surface backends render into a grid or buffer;
  - the changed-rectangle copy.

## Related

[157](157-window-broker.md), [089](089-text-clock-faces.md) (text faces of `clock` and `dzen-clock`), [156](156-ps2-mouse.md), [155](155-virtual-consoles.md) (full-screen consoles with Alt+F1…F4, which complement windows), [063](../issues-done/063-file-manager-read-only.done), [054](../issues-done/054-tui-library.done), [docs/tools](../docs/tools/README.md).
