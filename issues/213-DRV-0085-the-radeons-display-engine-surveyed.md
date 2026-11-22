# 213-DRV-0085 — The Ryzen's display engine surveyed from Linux: its tables, the state of both outputs, and whether DMUB is needed

**Type:** survey (`hwdocs/`; read-only on the machine) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-21) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md) needs a driver for the Cezanne Radeon's display engine (DCN 2.1) to light the Ryzen's second monitor. Nothing is known yet of what that driver must write. Linux on the same machine drives **both** monitors (`DP-2`, 2560x1600, and `HDMI-A-1`, 3440x1440), so it holds a working configuration of the very output MIND Core lacks.

This task reads and writes no driver. **Reading is not harmless on this device:** on 2026-11-21 a survey script's register read hung the Ryzen in Linux and a person had to power-cycle it (Progress, below). Since then nothing reads its registers under Linux (BENCH rule); what Linux's own driver hands out as finished files is read instead, and a register read is only proposed, for MIND Core with KRN's grant.

## Plan

1. **What Linux knows of the engine** (as root, through the Effector agent on the Ryzen's Linux):
   - the VBIOS image (`/sys/kernel/debug/dri/*/amdgpu_vbios`) and its ATOM tables: the display paths (connectors, encoders, PHYs, the HPD and DDC lines), the integrated system information, the command tables that Display Core still calls;
   - the firmware versions (`amdgpu_firmware_info`), DMUB's among them;
   - the engine's state with both monitors lit: DC's own dump (`amdgpu_dm_dtn_log`), which names HUBP, DPP, MPC, OPP, OTG, the stream and link encoders and the clocks for each pipe;
   - ~~the registers behind that state, read from the MMIO BAR, for the blocks the dump names~~: tried on 2026-11-21, it hung the machine; withdrawn (Progress);
   - each monitor's EDID and the mode in use.
2. **What the firmware leaves for MIND Core:** the same registers with one output set by the GOP and no `amdgpu`. Two ways, to be chosen and said: Linux started once without `amdgpu`, or a read-only probe under MIND Core with a device grant from KRN.
3. **The DMUB question,** from Linux's Display Core source (pinned, as the Wi-Fi tables' sources are) and from the state read: on DCN 2.1, can a second output be set without DMUB's firmware?
   - If not: that firmware is proprietary. It would go the way of the Wi-Fi's (fetched into `proprietary/`, copied onto the disk, never committed), and its load through the PSP is a task of its own.
4. **A route to first light,** the smallest the evidence supports: which pipe, which encoder and PHY for `HDMI-A-1`, which clock, which mode of its EDID, which of these go through ATOM command tables; and the engine's own test pattern as the first picture, which needs no framebuffer.
5. **What it needs of KRN:** the device's grant for a new service, memory the engine can scan out, and the compositor's part. Sent to KRN as a request with the survey's facts.

## Acceptance criteria

- [ ] `hwdocs/` holds the survey with its sources: the ATOM tables named, the state of both outputs under Linux, the state the firmware leaves, and how each was read.
- [x] The DMUB question is answered with its evidence, or what is still missing to answer it is named.
- [x] A route to first light is written as register blocks and tables, each tied to a line of the survey. It is a plan, not a guarantee (MC-12.3).
- [ ] Nothing on the Ryzen was changed: no package installed, no register written, the machine back in MIND Core. **Not met as written:** no package was installed and no register written by this task, and the machine is back in MIND Core, but the register read hung it and it needed a power cycle (Progress).

## Progress (2026-11-21)

**The survey is in [hwdocs/cezanne-display.md](../hwdocs/cezanne-display.md)** (it continues the owner session's notes of
2026-11-20, merged on this branch): the device and its BARs, the VBIOS's tables read with the new
`scripts/hardware/amd_vbios_tables.py`, the display paths with their PHY, DDC and HPD lines, which Linux connector is
which, both monitors' EDIDs and modes, Linux's state of both pipes from Display Core's dump, what the firmware leaves
(known and not), the DMUB question, a route to first light on `HDMI-A-1`, the register map, a register read proposed as
safe for MIND Core, and the memory a second framebuffer needs. In short:

- **HDMI-A-1 is path 0**: HDMI type A on UNIPHY enum 1 = **PHY A**, DDC on I2C line 0 (`DC_GPIO_DDC1`), HPD pin 1
  (`DC_GPIO_HPD_A` bit 0, HPD 1). **DP-2, the firmware's output, is path 4, PHY E.** Linux drives HDMI-A-1 from OTG 0
  (pipes 0 and 3 combined) and DP-2 from OTG 1 (pipes 1 and 2), scanning both out of system RAM through the GART.
- **The monitors**: HDMI-A-1 a 3440x1440 HDMI 2.0 sink preferring 324 MHz at 60 Hz (below 340 MHz: no scrambling),
  offering CTA 1080p60; DP-2 2560x1600 at 268.5 MHz. Their EDIDs' timings give exactly the OTG values Display Core
  dumped.
- **DMUB: not needed for a TMDS mode set, by Linux's source** (with early DMUB firmware Linux itself ran DCN 2.1's four
  command tables through its host ATOM interpreter; the DMUB otherwise serves PSR, ABM and those tables). Not shown on
  this machine; first light's step 3 will be the test. It needs an ATOM interpreter in `display` and the VBIOS image
  from the ACPI VFCT table.
- **The route** (213-DRV-0086 step 3): read the firmware's state first; then on a free OTG and OPP, DIG front end A,
  PHY A: OTG clocks, VBIOS table 12 (setpixelclock 1.7, PHY A's PLL), the OTG timing of 1080p60, the OPP's pattern
  generator (DCN 2.1's OTG has none), start, VBIOS table 4 (digxencodercontrol 1.5), the DIG wiring, VBIOS table 76
  (dig1transmittercontrol 1.6) in DVI mode. `display/src/dcn.rs` holds it as an ordered plan; its host cases pass.
- **The firmware's own state is not read** (criterion 1 stays open): which OTG, OPP, HUBP and DIG front end the GOP
  uses, its surface address and `DCN_VM_FB_LOCATION_*`, DISPCLK. The two ways: a read under MIND Core by `display` with
  KRN's grant (proposed as seven batches of registers that Display Core itself reads, in the hwdocs file), or Linux once
  without `amdgpu`. The first waits for KRN's grant and the owner session's word.
- **To KRN** (passed on by the owner session): the facts KRN asked for (the command register 0x0006 as the firmware
  leaves it, from Linux's own `enabling device (0006 -> 0007)`; BAR 5 memory, 32-bit, not prefetchable, 512 KiB; BAR 0
  over the carve-out as the place for a second framebuffer, which the engine scans with no page table, confidence
  medium), and a request: BAR 5 uncached, the VBIOS image from the ACPI VFCT table read-only, a BAR 0 range after the
  GOP's framebuffer mapped write-combined, and a versioned interface for the compositor's second screen.
  KRN granted it (213-KRN-0271). The carve-out is out of MIND Core's frame pool: the hardware report of release
  202611211819 shows a reserved region of 541 MiB at the top of RAM, below `TOP_MEM2` (213-DRV-0086, run 0). Its start,
  `DCN_VM_FB_OFFSET`, reads 0x8_1000_0000 under MIND Core (213-DRV-0086 step 1, run 7).

**The hang, as it happened (2026-11-21).** At 13:25 UTC the Ryzen was switched from MIND Core to Linux under the lease;
at 13:27:35 a script ran as root through the Effector agent. In this order it read: `uname`, `/proc/cmdline`,
`lspci -vv`, `-vvv`, `-xxx` and `-xxxx` of 05:00.0 and `lspci -vvv` of every device, the device's `resource` file and
PCI state files, `/proc/iomem`, the DRM class directory, `mem_info_*` and the `ip_discovery` sysfs tree, `dmesg` and the kernel journal, `amdgpu`'s module parameters, the firmware directory and the DMUB
file's header, the ACPI table list and the VFCT table, amdgpu's debugfs files `name`, `clients`, `framebuffer`, `state`,
`amdgpu_vram_mm`, `amdgpu_gtt_mm`, `amdgpu_gem_info`, `amdgpu_firmware_info`, `amdgpu_pm_info`,
`amdgpu_dm_capabilities`, `amdgpu_dm_visual_confirm`, `amdgpu_dm_dmub_trace_mask`, `amdgpu_dm_dcc_en`,
`amdgpu_mst_topology`, `amdgpu_dm_dmub_tracebuffer`, `amdgpu_vbios`, `amdgpu_dm_dmub_fw_state`, `amdgpu_dm_dtn_log`
twice, the connectors' and CRTCs' debugfs files `link_settings`, `phy_settings`, `lttpr_status`, `is_dpia_link`,
`max_bpc`, `vrr_range`, `internal_display`, `odm_combine_segments`, `force_yuv420_output`, `amdgpu_current_bpc` and
`amdgpu_current_colorspace`, the connectors' sysfs `status`, `enabled`,
`dpms`, `modes` and `edid`; then it mapped BAR 5 read-only and read about 3675 registers twice (DCN 2.1's HUBBUB, HUBP,
DPP, MPC, OPP, OPTC/OTG, DIO with DIG, DP, HPD, AUX and I2C control, DCCG, DMU power gating, DMCU, DMCUB, the GPIO
pads, DSC, the DPCS/RDPCS PHY interface, the VGA and interrupt blocks, and three SMU mailbox registers); then it was to
read the stick's log partition and send everything out. The Linux agent stopped answering at 13:28:51 and nothing came
back; the maintainer power-cycled the machine. **Which read hung it cannot be told**: the script sent nothing before
its end and Ubuntu empties `/tmp` at boot. The register read is the likely cause (the files are the kernel's own and
were read before without harm), and among its blocks the likeliest are those Display Core does not read in normal
running (the PHY interface, DMCU/DMCUB, the mailbox, gated pipes). "Read-only" did not hold: reading this device's
registers changed the machine. The owner session then read the kernel's files alone, one step per operation, with no
register read; nothing hung, and the machine was returned to MIND Core (bench log, 15:02 UTC).

## Related

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md) (the driver), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light), [213-DRV-0074](../issues-done/213-DRV-0074-the-ryzens-second-monitor.done) (the firmware gives one output).
