# 213-DRV-0095 — `display`: the second output plugged and unplugged

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3, MC-12.4

## Problem

Steps 3 and 4 run once a boot, at start, and assume HDMI-A-1's monitor stays. If it is unplugged, pipe 1 keeps
scanning, and the compositor keeps drawing into a screen nobody sees. If one is plugged in after start, nothing lights
it until the next boot.

## Plan

- **The events:** `display` polls HPD 1's sense (`HPD1_DC_HPD_INT_STATUS`, which step 1's batch 1 reads), since the
  display engine's interrupt is not `display`'s.
- **HPD 1 going off:**
  - `screen_gone` first, then stages 16 to 9 taken back (pipe 1 blanked and off);
  - then first light's 8 to 1, as their undo does now.
- **HPD 1 coming on:** step 1's reads of what changed, the EDID again (213-DRV-0096), then the stages, without a boot.
- **The once-a-boot mark** (`OTG1_OTG_VERTICAL_INTERRUPT2_POSITION`) becomes a mark of "lit": set when the stages
  have run, cleared when they are taken back for an unplug.

## Acceptance criteria

- [ ] Host cases: an unplug after stage 16 gives `screen_gone`, then every register as before step 3; a plug in again
  runs the stages once more; a short bounce of HPD runs neither.
- [ ] On the Ryzen: the maintainer unplugs and plugs HDMI-A-1's monitor with MIND Core running. The screen goes and comes
  back, and DP-2 stays as it is.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (HPD 1, step 1), [213-DRV-0096](213-DRV-0096-display-the-second-outputs-mode-from-its-edid.md).
