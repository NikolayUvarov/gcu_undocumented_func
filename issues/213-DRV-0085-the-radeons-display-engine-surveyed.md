# 213-DRV-0085 — The Ryzen's display engine surveyed from Linux: its tables, the state of both outputs, and whether DMUB is needed

**Type:** survey (`hwdocs/`; read-only on the machine) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-21) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md) needs a driver for the Cezanne Radeon's display engine (DCN 2.1) to light the Ryzen's second monitor. Nothing is known yet of what that driver must write. Linux on the same machine drives **both** monitors (`DP-2`, 2560x1600, and `HDMI-A-1`, 3440x1440), so it holds a working configuration of the very output MIND Core lacks.

This task reads; it changes nothing on the machine and writes no driver.

## Plan

1. **What Linux knows of the engine** (as root, through the Effector agent on the Ryzen's Linux):
   - the VBIOS image (`/sys/kernel/debug/dri/*/amdgpu_vbios`) and its ATOM tables: the display paths (connectors, encoders, PHYs, the HPD and DDC lines), the integrated system information, the command tables that Display Core still calls;
   - the firmware versions (`amdgpu_firmware_info`), DMUB's among them;
   - the engine's state with both monitors lit: DC's own dump (`amdgpu_dm_dtn_log`), which names HUBP, DPP, MPC, OPP, OTG, the stream and link encoders and the clocks for each pipe;
   - the registers behind that state, read from the MMIO BAR, for the blocks the dump names;
   - each monitor's EDID and the mode in use.
2. **What the firmware leaves for MIND Core:** the same registers with one output set by the GOP and no `amdgpu`. Two ways, to be chosen and said: Linux started once without `amdgpu`, or a read-only probe under MIND Core with a device grant from KRN.
3. **The DMUB question,** from Linux's Display Core source (pinned, as the Wi-Fi tables' sources are) and from the state read: on DCN 2.1, can a second output be set without DMUB's firmware?
   - If not: that firmware is proprietary. It would go the way of the Wi-Fi's (fetched into `proprietary/`, copied onto the disk, never committed), and its load through the PSP is a task of its own.
4. **A route to first light,** the smallest the evidence supports: which pipe, which encoder and PHY for `HDMI-A-1`, which clock, which mode of its EDID, which of these go through ATOM command tables; and the engine's own test pattern as the first picture, which needs no framebuffer.
5. **What it needs of KRN:** the device's grant for a new service, memory the engine can scan out, and the compositor's part. Sent to KRN as a request with the survey's facts.

## Acceptance criteria

- [ ] `hwdocs/` holds the survey with its sources: the ATOM tables named, the state of both outputs under Linux, the state the firmware leaves, and how each was read.
- [ ] The DMUB question is answered with its evidence, or what is still missing to answer it is named.
- [ ] A route to first light is written as register blocks and tables, each tied to a line of the survey. It is a plan, not a guarantee (MC-12.3).
- [ ] Nothing on the Ryzen was changed: no package installed, no register written, the machine back in MIND Core.

## Related

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md) (the driver), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light), [213-DRV-0074](../issues-done/213-DRV-0074-the-ryzens-second-monitor.done) (the firmware gives one output).
