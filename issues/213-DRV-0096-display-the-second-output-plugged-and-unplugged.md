# 213-DRV-0096 — `display`: the second output plugged and unplugged

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

## Progress (2026-11-22)

**Built, behind `HOTPLUG`** (committed false; it needs `STAGES_ALLOWED` above 0 and not `UNDO_AFTER_STAGES`), on
213-DRV-0098's production order. After steps 1 to 4, `display` follows HPD 1 (`hpd.rs`):
- **The poll:** HPD 1's sense (`HPD1_DC_HPD_INT_STATUS.DC_HPD_SENSE`, which step 1's batch 1 reads) every 50 ms
  (`hpd::POLL_MS`), once `DC_HPD_EN` shows HPD 1 enabled; otherwise it says so and follows nothing.
- **A change counts once it has held 500 ms** (`hpd::SETTLE_US`); a shorter one, a bounce, does nothing.
  - This is longer than the HPD pulse of at least 100 ms by which an HDMI sink says its EDID changed, so such a pulse
    is a bounce here and the mode stays.
  - It is short against a person's plugging.
  - It is display's own choice, not Display Core's.
- **The output's state** after a run (`hpd::state_after`):
  - **lit** while stages are in place (stopped after its last allowed stage, or stage 9, 10 or 16 not ready);
  - **stopped** after a failure or the build's own undo, when nothing more is tried this boot;
  - **dark** when none ran (no monitor at start, a refusal).
- **An unplug of a lit output** (`light::take_back`):
  - stages `done` to 1 are undone, last first, as a failure's undo takes them back; stage 16's undo comes first and lets
    the compositor go (`screen_gone`);
  - then the mark is put back as it was before step 3, and the journal emptied;
  - whether the firmware's pipe reads as when step 3 began is logged. If it does not, the output is stopped, not dark.
- **A monitor plugged in on a dark output:** steps 1 to 4 run again on the same mapping, step 1 and the EDID read anew,
  then the stages.
- **The mark** (`OTG1_OTG_VERTICAL_INTERRUPT2_POSITION`) is now a mark of stages in place:
  - it is set before stage 1 as before;
  - a failure's undo leaves it, so a failed stage is not tried again;
  - an unplug's taking back clears it;
  - a start of display again while it is set (init restarting a faulted `display`) runs no stage, as before. That start
    follows nothing either: its journal is gone, and it leaves the output as the first start left it.
- **The journal** keeps the mark with its value before, and the firmware's pipe as step 3 began, for the taking back.
- **`light::prepare`** (assess, the plan, the reference) is what display's start and a plug both call.

**Host cases** (MC-12.2: a model, not the hardware):
- **`display_hpd_host.rs`**, 4 cases:
  - an unplug that holds takes a lit output back once, and a plug that holds lights it again, each at the first poll
    past the settle;
  - a bounce shorter than the settle, and a sense flickering at every poll for seconds, do neither, lit or dark;
  - a change to what the output already is does nothing, nor any change on a stopped output;
  - a late read still counts;
  - the state after each outcome.
- **`display_light_host.rs`**, 5 new cases (43 in all):
  - an unplug after stage 16 lets the screen go before anything else the device sees, undoes 16 to 1 and leaves every
    register as before step 3, the mark included; a plug runs all 16 stages again with the same writes and offer as the
    first time; a second unplug leaves everything as before again;
  - a start of display again while lit runs no stage and writes nothing; after an unplug, a start runs all 16;
  - after a build of 8, and after stage 16 refused, the taking back undoes what ran and calls no `screen_gone`;
  - with `MIND_AMD_VBIOS`, the Ryzen's tables:
    - warm: taken back whole, the off calls (tables 12 and 76) run to their end, and a plug makes the same writes again;
    - cold: taken back whole but PHY A's control register bus, which the undo leaves by design;
  - **end to end on the model:** step 1 in the production order, `prepare`, the 16 stages and `hpd::Follow` polling the
    model's HPD 1 every 50 ms on its clock:
    - a 300 ms bounce does nothing;
    - two seconds unplugged take it back once, `screen_gone` first, every register as before;
    - a bounce while dark does nothing;
    - eight seconds plugged in run steps 1 to 4 again, with the first start's writes and offer.

**Not shown:**
- **On the hardware, nothing of it.** It needs the maintainer's hands, and the owner session arranges the run.
- **The sense on the Ryzen:** what HPD 1 reads while a monitor is unplugged, or switched off by its own button.
- **The settle:** whether 500 ms suits the Ryzen's monitor.
- **The compositor** taking `screen_gone` and then a second `offer_screen` in one boot. Its host cases are KRN's.
- **`display`'s glue in main.rs** (the loop and the run kept): it is not built on the host. Its parts are: `hpd`,
  `take_back`, `prepare`, and step 1 on the same engine.

## Acceptance criteria

- [x] Host cases: an unplug after stage 16 gives `screen_gone`, then every register as before step 3; a plug in again
  runs the stages once more; a short bounce of HPD runs neither. `display_light_host.rs` (on the model, end to end with
  `hpd::Follow`, and with the Ryzen's tables) and `display_hpd_host.rs`.
- [ ] On the Ryzen: the maintainer unplugs and plugs HDMI-A-1's monitor with MIND Core running. The screen goes and comes
  back, and DP-2 stays as it is.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (HPD 1, step 1), [213-DRV-0095](213-DRV-0095-display-the-second-outputs-mode-from-its-edid.md).
