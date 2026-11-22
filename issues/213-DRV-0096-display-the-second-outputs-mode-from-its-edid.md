# 213-DRV-0096 — `display`: the second output's mode from its EDID

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3

## Problem

Steps 3 and 4 light HDMI-A-1 at the fixed CTA 1920x1080 at 60 Hz (`timing::CEA_1080P60`), and stage 10 draws at that
size. A monitor that prefers another mode, or does not take this one, gets the wrong picture or none.

## Plan

- **The mode:** taken from the monitor's EDID, which step 1 already reads over the DDC (`edid.rs`). Its preferred
  timing is used, within what DCN 2.1 and PHY A drive at TMDS rates, else the largest CTA mode it lists.
- **What follows from it:**
  - the pixel clock for table 12;
  - the OTG timing;
  - pipe 1's DLG, TTU, sync and QoS (stage 9's formulas);
  - the surface's shape, within slot 5's 20 MiB, and the picture drawn for it;
  - the size offered to the compositor.
- **The EDID's bytes** stay out of the repository, as now.

## Acceptance criteria

- [ ] Host cases: the stages derived for several EDIDs' preferred modes (1080p, 1440p at 60 Hz, a mode past PHY A's
  limit refused with a reason).
- [ ] On the Ryzen: HDMI-A-1 at its monitor's preferred mode, and the mirror at that size.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md), [213-DRV-0095](213-DRV-0095-display-the-second-output-plugged-and-unplugged.md).
