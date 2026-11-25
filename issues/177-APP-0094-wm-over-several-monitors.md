# 177-APP-0094 — `wm` over several monitors, and the `displays` tool

**Type:** tools (`wm`, `displays`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (the desk's layout over two screens written, 2026-11-22; the windows over it, 2026-11-25; the rest on KRN's 177-KRN-0274) · **Blocked by:** `KRN`: [177](177-display-output.md)'s steps 0–1 (every output and its EDID, every screen the firmware lit, in `BootInfo`), and the interface versions its later steps need · **Main task:** [177](177-display-output.md) · **Roadmap:** track G · **Constitution:** MC-12.4

**Recorded by:** the kernel track (KRN), 2026-10-12, in `requests-APP.md`, for main task 177, at the maintainer's request (several monitors, their positions, and the GPU for the picture). Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

`wm` draws on one screen. The maintainer asked for several monitors and their positions.

## Plan

177's step 2, `wm`'s side:

- each output's rectangle in `wm`'s desk;
- windows placed per output, and moved between them;
- a `displays` tool that lists the outputs, their modes and positions;
- `wm`'s Settings, Screen page, writing `data/displays.txt` (positions, the primary output).

## Acceptance criteria

With QEMU's two outputs (or two screens in `BootInfo`), `wm` shows a window on each, and `displays` lists both with their positions; a position set in Settings is read back after a restart.

## Related

[177](177-display-output.md) (display output).

## Progress (2026-11-22)

- **The Ryzen's second monitor.** DRV asked for `wm` on two screens, an extended desktop, from the maintainer's ask after he saw 213-KRN-0272's mirror (`APP/2026-11-22T0829Z-DRV-request-wm-on-two-screens-an-extended-desktop-with-the-second-placed-relative-to-the-first.md`). Taken as this task.
- **KRN's plan,** [177-KRN-0274](177-KRN-0274-an-extended-desktop-on-two-screens.md): `display.wit` 1.5, `extend` and a side, and a surface the task in front lends.
- **APP's answer** (`KRN/2026-11-22T1240Z-APP-re-the-second-screens-fit-213-APP-0158-and-177-KRN-0274s-plan.md`):
  - one surface for the whole desktop, which the compositor cuts into the two screens, so `wm`'s grid, pointer and windows stay one space;
  - the pointer's crossing is `wm`'s, through `mind::input::pointer_area_cells` set to the whole desk;
  - `wm` reads the layout from `mirror`.
- **Written, with no system calls:** `wm/src/screens.rs`, tested in `tests/wm_host.rs` (`the_desk_over_two_screens`):
  - the desk for the first screen and the second on a side, aligned at the first's top or left edge;
  - which screen a cell is on, the corner neither covers going to the nearest;
  - the screen beside another;
  - a window moved to another screen at the same place in proportion, its size kept where it fits.

## Progress (2026-11-25): the windows over two screens

- **Written, with no system calls,** in `wm/src/desk.rs` and `wm/src/screens.rs`:
  - `Desk` lies over a `Layout`, one screen until the compositor offers a second. `arrange` lays the windows out on a new layout, each keeping its screen, its place and its share of it; `rescale` is `arrange` with one screen.
  - The bars are the first screen's. A window goes on the area of the screen under its middle: the first below the top bar and above the status line, another screen whole.
  - A window across the edge stays across it while every cell of it is on a screen's area; one over the top bar or in the corner neither screen covers is moved inside its screen.
  - Maximize, halves and quarters are within the window's screen. A window snaps to the outer edges of its screen; an edge with another screen beyond is crossed, not stuck to.
  - **Alt+Shift+arrows** move the window to the screen on that side, at the same place in proportion, its size kept as far as that screen holds it. One that fills a half, a quarter or the whole area fills the same share there. The frame `[⇕]` gives back goes with it. With no screen there the status line says `To another screen: no screen is to the right of this one`.
  - A full-screen window covers its own screen. The other keeps the desk, its windows and the mouse.
- **Tested** in this container, x86_64: `tests/wm_host.rs`'s `the_windows_on_two_screens`, and the 50 tests before it unchanged, so one screen behaves as it did.
- **Left:**
  - 1.5 in `main` (177-KRN-0274's plan: a surface per extra screen, `extend-surface`; APP asked for one surface for the whole desk, not answered yet);
  - the bars, the dialogs, the menu and Settings drawn on the first screen, and the pixels, the background and the pointer over both; the run window and the list on the pointer's screen;
  - the pointer's area over the whole desk;
  - the layout read from `mirror`, and the shell's `screen second extend <side>`;
  - the Ryzen's run.

