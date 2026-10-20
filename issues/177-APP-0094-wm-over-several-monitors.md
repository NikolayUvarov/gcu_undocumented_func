# 177-APP-0094 — `wm` over several monitors, and the `displays` tool

**Type:** tools (`wm`, `displays`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open (planned) · **Blocked by:** `KRN`: [177](177-display-output.md)'s steps 0–1 (every output and its EDID, every screen the firmware lit, in `BootInfo`), and the interface versions its later steps need · **Main task:** [177](177-display-output.md) · **Roadmap:** track G · **Constitution:** MC-12.4

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
