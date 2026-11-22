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

## Related

[213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (the procedure), [213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md).
