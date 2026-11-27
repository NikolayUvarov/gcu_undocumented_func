# 213-DRV-0097 — `display`: table 12's cold path on the Ryzen

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (2026-11-22) · **Blocked by:** the maintainer's decision to start MIND Core not through Linux · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.9

## Problem

Every MIND Core start on the Ryzen so far came through Linux, so PHY A was already set up, and table 12 (SetPixelClock)
took its short path (308 steps, 33 writes). Its long path has not run on this hardware: 530 steps, 111 writes, 37 of
them through PHY A's control register bus. Stage 2's line names the path each boot. Boots 70, 71 and 73 to 76 all logged `S2
THE PHY WAS SET UP ALREADY`.

## Plan

213-DRV-0086's procedure, "Table 12's cold path, and what a test of it would take". In short:
- **The start:** MIND Core not through Linux. `BootNext` is set from Linux, the machine is powered off, and a person
  switches it on.
- **What to watch:**
  - stage 2's path line;
  - the long path's bounded waits;
  - HDMI-A-1 waking at stage 8;
  - DP-2 through every stage.
- **The undo:** it takes back every register, PHY A's mark too. The 17 writes into PHY A's internal registers stay.

## Acceptance criteria

- [ ] On the Ryzen, after a cold start: stage 2 logs the long path, first light comes up, and the firmware's pipe stays
  unchanged. This holds for that start only (MC-12.9).

## Not reached by taking the cables out (2026-11-27)

A start through Linux with no monitor in either HDMI socket does not give a cold PHY. On boot 117 (release 202611271414,
`display` of 213-DRV-0113's branch) Linux had both HDMI connectors `disconnected disabled`, and still each PHY was set up
already when its monitor was plugged in under MIND Core: `THE PHY SET UP ALREADY: THE SHORT PATH` on PHY A (321 steps,
33 writes) and on PHY B (331 steps, 32 writes). Most likely the VBIOS sets them up when the machine starts. So the long
path still needs a start where nothing did, as this task says.

## Related

[213-DRV-0086](../issues-done/213-DRV-0086-display-the-second-output-lit.done) (the procedure), [213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md).
