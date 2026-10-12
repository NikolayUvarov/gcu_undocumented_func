# 000-KRN-0086 — The compositor copies whole frames: no pointer missing, no cleared cells on the screen

**Type:** kernel (`scheduler.rs`, `common/abi.rs`), `compositor`, `libmind` `dev` · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and tested in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer (2026-10-11, build 8223d24 on the MacBook Pro):

- the pointer flickers as it moves over the desktop;
- now and then a black square shows under it;
- the same happens over the graphical clock.

**The cause, as read from the code** (the MacBook Pro's run is to confirm it). The compositor copied the focused task's screen every 15 ms whenever the kernel had marked it dirty. The kernel marks it on any `WAIT`, any IPC and the end of every time slice of the task, which says nothing about where the task is in a frame. `wm` draws a frame in steps:

1. it draws the cells again, which wipes the pointer drawn the last time;
2. it copies the pixel windows (the clock) over their cells;
3. it draws the pointer.

A copy taken after step 1 shows no pointer: the flicker. A copy taken while cells are cleared but not yet filled, or before step 2, shows a black square. The pixel windows are copied again on every move of the pointer, so the gap is widest over the clock.

## Plan

- **A frame mode for `COMPOSITOR_PULL`** (an addition to the ABI; mode 0 works as before):
  - `COMPOSITOR_FRAME`. A changed screen whose task is ready or running gives `COMPOSITOR_BUSY`, since it may be mid-frame. One whose task waits (a sleep, IPC, an interrupt) gives "dirty", and the kernel holds that task: it is not selected, even when its wait ends.
  - `COMPOSITOR_RELEASE` lets it run again. The kernel lets it go by itself after 50 ms, so a compositor that stops cannot stop the task.
- **The compositor**:
  - pulls in frame mode;
  - copies, then releases;
  - on `BUSY` it tries again 1 ms later;
  - after 250 ms of `BUSY` it copies the screen as it is, so a program that draws without ever waiting still shows.
- **`wm` is unchanged.** It sleeps after each frame, so its frames are copied whole.
- **Not done here.**
  - A frame a program declares itself (a present call).
  - Copying at the display's refresh (vsync).
  - A screenshot (`display.capture`) still copies the screen whenever it is asked.

## Acceptance criteria

- In QEMU (x86, the isolation suite), `app2 F` draws numbered frames, the bottom half 3 ms before the top.
  - No screen dump shows a bottom half newer than its top.
  - The frames reach the screen.
  - With the compositor in mode 0 the same check fails.
- The display, `wm`, windows, tablet and clock suites pass as before.
- On the MacBook Pro the pointer moves over the desktop and over the graphical clock without flicker or a black square.

## Progress

**2026-10-11, QEMU x86 (TCG).**

- With the old pull (mode 0) the check failed at its first screen dump: top frame 73, bottom 74.
- With frame mode it passed: 10 screen dumps, frames 58 to 527.
- These suites passed with it on 4 CPUs: boot, display, dzen, isolation, windows, wm and tablet. The aarch64 image builds.

## Related

[000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md), [000-KRN-0083](../issues-done/000-KRN-0083-a-1-ms-tick-for-sleeps-and-deadlines.done), [docs/profile/README.md](../docs/profile/README.md) (the compositor's shared memory).
