# 157 — `windows`: window broker, windows that outlive the window manager

**Type:** service + IDL · **Owner:** services track (this branch), with the tools track for 088 · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-2.6, MC-2.11, MC-3.11, MC-6.4

## Problem

Programs shown in windows must keep running when the window manager (088, and a graphical one later) stops or fails, and must be shown again when a window manager comes back. So the windows cannot belong to the window manager. The program's surface, its input endpoint and the window's place on the screen must be kept by something that outlives the manager. That something must hold no user authority and must not see the screen.

## Plan

- **`windows`, a small boot service** (`idl/window.wit`). It keeps a registry of windows and nothing else:
  - per window: the owner PID, its surface, its input endpoint, its title, and its last geometry, z-order and state (shown, minimized, maximized), saved by the manager;
  - no screen, no input privilege, no files, no network.
  
  init restarts it like every service. A restart loses the registry, so programs register again when their client sees `ERR_PEER`.
- **Program side** (through `mind::window`, used by the surface backends of `mind::tui` and `gfx::Screen` from 088):
  - A program that runs in a window creates its own surface, text (cells) or pixels, and its own input endpoint. It registers both with `windows`.
  - The surface is the program's memory, lent to the broker and manager (`SHARE_RW` adapter, MC-2.6). The manager only copies out of it, after checking sizes (MC-2.11).
  - The program draws whether or not a manager shows it. The broker tells it when it is hidden, so it may draw less.
  - A program started in window mode while `windows` does not run falls back to a full screen.
- **Manager side.** One manager at a time attaches with the broker's manager badge (the shell lends the badged client to a program that asks for it):
  - it gets the windows with their saved geometry;
  - it receives registrations and removals while attached;
  - it saves geometry as windows move.
- **Two ways for a manager to stop, which the broker records:**
  - **detach** — the windows stay with their programs, which keep running hidden. The layout stays in the broker. A manager that dies is detached. The next manager that attaches shows the windows where they were.
  - **close all** — the broker sends every window's program a close event on its input endpoint and removes the windows. A program that does not end within a few seconds is reported to the manager. The manager may then stop it through its lifecycle client (`init.wit` `stop-task`), only if it holds one.
- **The broker ends a window when its program ends**, checking with `process::alive` (it cannot watch tasks it did not start).

## Acceptance criteria

QEMU suite with a test manager and two test programs (or `wm` from 088 once it exists):
- The manager attaches and gets both windows.
- After the manager is killed, the programs still run and update their surfaces.
- A new manager gets the same windows at their saved positions.
- "Close all" ends both programs.
- A program that ends loses its window.
- A second manager is refused while one is attached.
- A program cannot read another program's surface through the broker.

## Related

[088](088-text-window-manager.md), [155](155-virtual-consoles.md), [156](156-ps2-mouse.md).
