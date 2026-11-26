# 213-DRV-0078 — A display driver for the Ryzen's Radeon (DCN 2.1): its second monitor lit

**Type:** driver (a new `display` service; `compositor` with `KRN`) · **Owner:** `DRV` · **Priority:** P2 (the maintainer, 2026-11-21) · **Status:** open (2026-11-20; split on 2026-11-21 into [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md) and [213-DRV-0086](../issues-done/213-DRV-0086-display-the-second-output-lit.done)) · **Blocked by:** 213-DRV-0085, 213-DRV-0086 · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

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

- [ ] The survey recorded in `hwdocs/` with its sources: the ATOM tables named, the DCN state the firmware leaves, and the DMUB question settled.
- [ ] On the Ryzen, the HDMI monitor shows MIND Core's screen, and the DP monitor keeps it, both at a stated mode. This holds for that machine and its two monitors (MC-12.1).

## Its tasks (2026-11-21)

- [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md): steps 1 and 2 above, the survey and the DMUB question.
- [213-DRV-0086](../issues-done/213-DRV-0086-display-the-second-output-lit.done): step 3, first light, with the engine's own test pattern before a framebuffer.
- Step 4, the compositor, is KRN's and is asked of it with the survey's facts.

## Related

[213-DRV-0074](../issues-done/213-DRV-0074-the-ryzens-second-monitor.done) (one firmware output: the finding), [213-KRN-0098](../issues-done/213-KRN-0098-black-screen-when-the-shell-takes-it.done) (the shell's screen on this machine).
