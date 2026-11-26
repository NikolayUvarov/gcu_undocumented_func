# 213-DRV-0078 — A display driver for the Ryzen's Radeon (DCN 2.1): its second monitor lit

**Type:** driver (a new `display` service; `compositor` with `KRN`) · **Owner:** `DRV` · **Priority:** P2 (the maintainer, 2026-11-21) · **Status:** open (2026-11-20; split on 2026-11-21 into [213-DRV-0085](../issues-done/213-DRV-0085-the-radeons-display-engine-surveyed.done) and [213-DRV-0086](../issues-done/213-DRV-0086-display-the-second-output-lit.done)) · **Blocked by:** a run of `main`'s own `display` on the Ryzen (criterion 2) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

The Ryzen drives two monitors in Linux, and MIND Core only one. The firmware sets up **one** graphics output (213-DRV-0074): GOP 0, 2560x1600, the monitor on `DP-2`. The monitor on `HDMI-A-1` (3440x1440) stays dark. MIND Core has no driver for the display engine and draws into the firmware's framebuffer only.

**The engine,** as Linux's `amdgpu` names it:
- the Cezanne APU's Radeon at 05:00.0 (1002:1638), VBIOS `113-CEZANNE-021` (fetched from the ACPI VFCT table);
- Display Core on **DCN 2.1**;
- the DMUB microcontroller's firmware loaded **through the PSP**, version `0x0101002B`.

## What this takes

This is a large driver. Nothing of it exists yet, and none of it is promised by a date (MC-12.3).

1. **A read-only survey, from Linux on the Ryzen:**
   - the VBIOS image (`/sys/kernel/debug/dri/0000:05:00.0/amdgpu_vbios`) and its ATOM tables: the connectors and their encoders and PHYs, the integrated system information, the command tables DC still calls (encoder and transmitter control, pixel clocks);
   - the DCN registers the firmware left for `DP-2`: the pipe from HUBP to OTG, the DIG, the PHY, DCCG's clocks. This is read under Linux with `amdgpu`'s register files in debugfs. It shows a working configuration of this very engine.
2. **What DMUB is needed for on DCN 2.1.** Is a plain mode set on a second output possible without it? If DMUB is required, its firmware is proprietary. It would then go the way of the Wi-Fi's: fetched into `proprietary/`, copied onto the disk, never committed. And its load through the PSP becomes a sub-task of its own.
3. **A first light,** the smallest route that the survey supports:
   - the firmware's own `DP-2` pipe is left untouched;
   - a second pipe scans out a second framebuffer: HUBP, DPP, MPC, OPP and OTG;
   - the HDMI side gets its TMDS encoder and PHY, through the VBIOS command tables where DC uses them, and its pixel clock;
   - a mode from the monitor's EDID that the engine can clock.
4. **The compositor:** the second screen as a mirror first, then extended. That is `KRN`'s share, behind a versioned interface.

## Acceptance criteria

- [x] The survey recorded in `hwdocs/` with its sources: the ATOM tables named, the DCN state the firmware leaves, and the DMUB question settled. In `main`: [hwdocs/cezanne-display.md](../hwdocs/cezanne-display.md) (213-DRV-0085, done 2026-11-26).
- [ ] On the Ryzen, the HDMI monitor shows MIND Core's screen, and the DP monitor keeps it, both at a stated mode. This holds for that machine and its two monitors (MC-12.1).

## Its tasks (2026-11-21)

- [213-DRV-0085](../issues-done/213-DRV-0085-the-radeons-display-engine-surveyed.done): steps 1 and 2 above, the survey and the DMUB question.
- [213-DRV-0086](../issues-done/213-DRV-0086-display-the-second-output-lit.done): step 3, first light, with the engine's own test pattern before a framebuffer.
- Step 4, the compositor, is KRN's and is asked of it with the survey's facts.

## Progress (2026-11-26)

- **Criterion 1 is met:** the survey is in `main`, the firmware's state read under MIND Core, and the DMUB question settled on this machine: its DMCUB is held in reset and the second output is lit without it ([213-DRV-0085](../issues-done/213-DRV-0085-the-radeons-display-engine-surveyed.done)).
- **Criterion 2 is shown on bench builds, not yet on `main`'s:**
  - what was seen on the Ryzen (2026-11-22, boots 84 to 95, the bench branch with its build switches set): MIND Core's screen mirrored on `HDMI-A-1`, by the log at the monitor's own 3440x1440, with `DP-2` kept at 2560x1600 and not blinking by the maintainer's eye; the plug taken out and put back ([213-DRV-0096](../issues-done/213-DRV-0096-display-the-second-output-plugged-and-unplugged.done));
  - since then the code landed with a setting in place of the switches (213-DRV-0099, 213-KRN-0281): `main`'s `display` lights the second output only when `data/displays.txt` says `output second light`. That very build has not run on the machine (MC-12.1);
  - **the criterion is ticked after one boot of `main`'s build on the Ryzen with that line written,** read from the log and seen by the maintainer. The monitor's own menu (3440x1440 at 60 Hz) is [213-DRV-0095](213-DRV-0095-display-the-second-outputs-mode-from-its-edid.md)'s last criterion.
- **Not this task's:** the cold path of table 12 ([213-DRV-0097](213-DRV-0097-display-table-12s-cold-path-on-the-ryzen.md)); the extended desktop (177-KRN-0274, a plan).

## Related

[213-DRV-0074](../issues-done/213-DRV-0074-the-ryzens-second-monitor.done) (one firmware output: the finding), [213-KRN-0098](../issues-done/213-KRN-0098-black-screen-when-the-shell-takes-it.done) (the shell's screen on this machine).
