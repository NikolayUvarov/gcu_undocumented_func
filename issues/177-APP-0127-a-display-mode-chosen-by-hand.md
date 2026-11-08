# 177-APP-0127 — A display mode chosen by hand for each output, as well as `auto`

**Type:** tools (`displays`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (the maintainer will say when to start) · **Blocked by:** `KRN`: the file's format, the list of modes the loader reports, and the grant to write that file; 177's steps 1–2 · **Main task:** [177](177-display-output.md) · **Constitution:** MC-12.3

**Asked by:** DRV, for the maintainer, 2026-11-08 (`APP/2026-11-08T1210Z-DRV-a-mode-chosen-by-hand-and-two-screens-on-the-dexp.md` on `internal-dev-messages`). The two screens' places are [177-APP-0094](177-APP-0094-wm-over-several-monitors.md)'s, now wanted for the DEXP. Numbered on 2026-11-08.

## Problem

The DEXP N100 has two identical projectors (LINGBO T10 Max), one on each output.
- Each one's EDID names Full HD as its preferred mode, and the loader sets it (211-PRT-0011), but the projector shows less.
- Nothing lets the user choose another mode.

## Plan

- **In `displays` and on `wm`'s Settings, Screen page:** a mode for each output, chosen from the modes the loader reports, or `auto`.
- **The choice goes into the file the loader reads** (`data/displays.txt`, as 177 names it, in KRN's format).
- **The tool says that the choice applies after a restart.** That holds until a driver changes modes at run time (177-DRV-0051).
- **There is always a way back to `auto`.**
- **Each output is keyed by its connector,** not only by the monitor's EDID identity, since the two projectors are the same.

## Acceptance criteria

- **Host tests:** the file's lines written and read back, `auto`, and two identical monitors kept apart by connector.
- **QEMU:** with a second display device, a mode chosen in `displays` is in the file, and the next boot's loader sets it.
- **docs/tools**, English and Russian.

## Related

[177](177-display-output.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), 177-DRV-0051, 211-PRT-0011.
