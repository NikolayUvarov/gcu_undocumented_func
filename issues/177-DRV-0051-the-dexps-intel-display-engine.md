# 177-DRV-0051 — The DEXP's Intel display engine: both outputs lit, a mode set without a restart

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** `KRN`: [177](177-display-output.md)'s step 1 (several display capabilities, the compositor holding them all), asked on `internal-dev-messages` in `KRN/2026-11-08T1208Z-DRV-a-mode-chosen-by-hand-and-two-screens-on-the-dexp.md` · **Main task:** [177](177-display-output.md) (step 3b) · **Roadmap:** tracks A and G · **Constitution:** MC-3.3, MC-12.1, MC-12.3

## Problem

**The maintainer's report (2026-11-08):**
- **The setup.** The DEXP N100 has two identical projectors, LINGBO T10 Max, one on each of its two video outputs.
- **The resolution.** Each projector's EDID names a Full HD mode as preferred, and the loader sets it ([211-PRT-0011](211-PRT-0011-the-monitors-preferred-mode-from-its-edid.md)). The projector shows less.
- **The second projector.** Only one projector can be on.

**What the maintainer asked for:**
1. a resolution chosen by hand, as well as automatically;
2. two screens placed relative to each other.

**What already covers part of it:**
- **The screen today** is the one framebuffer the firmware's GOP leaves at boot (177). So the loader can choose a mode only before it leaves boot services, and only on the output the firmware lit.
- **A mode chosen by hand at boot** covers request 1 for that output. It is `KRN`'s part (the message above).
- **The screens' layout** is 177's step 2: `KRN`'s compositor, `APP`'s `wm` and `displays` ([177-APP-0094](177-APP-0094-wm-over-several-monitors.md)).

**What needs a driver of the display engine** (177's step 3b):
- lighting the output the firmware leaves dark;
- changing a mode without a restart.

**The GPU:** `00:02.0`, `8086:46D1`, Intel UHD Graphics of the N100 (Alder Lake-N), subsystem `8086:7270`. The source is the DEXP's `hw0007.txt` of 2026-11-08, build `d83c98c5`.

## Plan

1. **Read only:**
   - What the firmware left: the display engine's pipes, transcoders, DDI ports, PLLs and power wells, and which port drives which connector.
   - Each connector's EDID, read over its own DDC/AUX.
   - Logged on the DEXP with both projectors attached; nothing is written.
2. **A mode on the lit output:** that output's timing changed to the mode chosen in `KRN`'s file or to its EDID's, on the framebuffer the compositor holds. A failure keeps the firmware's mode, with a line that says why.
3. **The second output:**
   - its power well, PLL, pipe, transcoder and DDI port enabled;
   - lit with a framebuffer of its own;
   - handed to the compositor as a second display capability (177's step 1).
4. **Hot plug and sleep:** a monitor plugged in or out is seen and logged; a monitor is put to sleep and woken.

**Sources:**
- Intel's public PRMs for Alder Lake, the display volumes;
- Linux's `i915` as the reference. Any table taken from it is recorded in `THIRD_PARTY.md`.

## Acceptance criteria

- **Step 1's log** names, on the DEXP, every connector, what drives it and its EDID.
- **With both projectors attached:**
  - both are lit at once, each at the mode the file chooses;
  - one desktop spans both, placed as `data/displays.txt` says (with `KRN`'s and `APP`'s parts);
  - the machine and the build are named (MC-12.1).
- **A mode the hardware refuses** leaves the firmware's mode and a line; no output is left dark without one.
- **Host tests** cover the timing and PLL arithmetic against the PRMs' worked values.

## Related

[177](177-display-output.md), [211-PRT-0011](211-PRT-0011-the-monitors-preferred-mode-from-its-edid.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), [800-LNK-0037](800-LNK-0037-boot-switch-from-windows.md) (the DEXP's boot from Windows).
