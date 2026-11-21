# The Ryzen's Radeon display engine (DCN 2.1): a survey for a display driver (213-DRV-0085)

A survey of the display engine of the Ryzen 9 5900HX's Radeon (Cezanne, `1002:1638` at 05:00.0) for 213-DRV-0086,
which is to light the HDMI monitor that stays dark under MIND Core. Read on 2026-11-20 and 2026-11-21 from Ubuntu's
Linux on the same machine (7.0.0-43-generic, with `amdgpu`), from MIND Core's boot log, and from Linux v7.0's source.
It is evidence for this machine and its two monitors only (MC-12.1), and its route is a plan, not a guarantee (MC-12.3).

## How it was read, and what went wrong

- **2026-11-20** (the owner session): connector names, the VBIOS image, Display Core's dump (`amdgpu_dm_dtn_log`).
- **2026-11-21, 13:27 UTC: a survey script hung the machine.** Run as root through the Effector agent, it read debugfs
  and sysfs files and then mapped BAR 5 (the register BAR) `PROT_READ` and read about 3675 display-engine registers
  in two passes, plus three SMU mailbox registers. The Linux agent stopped answering at 13:28:51; nothing came back,
  and a person had to power-cycle the Ryzen. **Which read did it cannot be told**: the script sent nothing out before
  its end and `/tmp` did not survive the cycle. The register read is the likely cause: Display Core's own dump reads the
  OTG, HUBP, DPP, MPCC, DSC and DIG blocks and never hung, while the script also read blocks Display Core does not read
  from that path (the PHYs' DPCS/RDPCS, DMCU, DMCUB, DCCG, GPIO, I2C, AUX, the VGA and interrupt blocks, the SMU
  mailbox), some of which may be clock- or power-gated. So "read-only" did not hold: **a register read of this device
  can hang the machine.** The rule since: no register read on the Ryzen under Linux by any route (BENCH.md, section 0).
- **2026-11-21, 14:58 UTC** (the owner session, after the power cycle): only files the kernel hands out, one step per
  operation, each sent out at once: `amdgpu_dm_dtn_log`, `amdgpu_firmware_info`, `amdgpu_dm_capabilities`, the DRM
  `state`, `framebuffer`, `clients`, the `amdgpu` lines of `dmesg`, `amdgpu_vbios` (55,296 bytes), `amdgpu_discovery`
  (the IP discovery table, 10,240 bytes), `amdgpu_pm_info`, the connectors' `status`, `modes` and `edid`, and
  `lspci -vv` of 05:00.0. Nothing hung. `amdgpu_dm_dmub_fw_state` was refused (`Operation not permitted`).
- Kept off the repository: the VBIOS image (the vendor's; SHA-256 `e04f13bed52078a3e04bd973e47c43d0186f3798182b9ebbe8ce82d6d08364c9`),
  the monitors' EDID bytes (they carry serial numbers), the raw dumps. They are in the owner session's scratch area.
- **Source read**: Linux **v7.0** (git.kernel.org, tag `v7.0`), the base of Ubuntu's 7.0.0-43, under
  `drivers/gpu/drm/amd/`; all of it carries AMD's MIT licence. The register headers are the same bytes as v6.12's.
  Files and the first 16 hex digits of their SHA-256:
  `include/renoir_ip_offset.h` 0be1f00100bfbb0e, `include/asic_reg/dcn/dcn_2_1_0_offset.h` 068e063f6801ccab,
  `include/asic_reg/dcn/dcn_2_1_0_sh_mask.h` 4d39f64a3d9f2ab2, `include/atomfirmware.h` be8817aba46ed37a,
  `include/discovery.h` bd64513b3397e3b2, `display/amdgpu_dm/amdgpu_dm.c` 8b7fc61f8a5b41da,
  `display/dc/resource/dcn21/dcn21_resource.c` 8c08c6b9a49439cb, `display/dc/bios/command_table2.c` 855c542d3cd96616,
  `display/dc/dce/dce_clock_source.c` 269443c43b28dfc3, `display/dc/optc/dcn10/dcn10_optc.c` 0555e7f3d0fbd75c,
  `display/dc/opp/dcn20/dcn20_opp.c` 4936196bc57d16b8, `display/dc/hwss/dcn20/dcn20_hwseq.c` 2d1840170cb199dc,
  `display/dc/hwss/dcn10/dcn10_hwseq.c` 28442e863dcfc390, `display/dc/hubbub/dcn21/dcn21_hubbub.c` e1af03a3e1c14d65,
  `display/dc/dio/dcn10/dcn10_link_encoder.c` f6d8f5109720c07d, `display/dc/dio/dcn10/dcn10_stream_encoder.c`
  3322bc2491c2e0e6, `display/dc/clk_mgr/dcn21/rn_clk_mgr_vbios_smu.c` fe95011e0942d513,
  `display/dc/gpio/dcn21/hw_translate_dcn21.c` a98d50bc5906a321, `amdgpu/gmc_v9_0.c` de41517fecd7e9ad,
  `amdgpu/mmhub_v1_0.c` 5fb631a7a26df618, `amdgpu/atom.c` 25614b1dc6724cd2. Nothing of it is copied here; the facts
  below are in this file's own words.

## The device

| | |
|---|---|
| Function | 05:00.0, `1002:1638` rev C4, subsystem `1002:0123`, class 030000 ("RENOIR" to `amdgpu`) |
| BAR 0 | memory, 64-bit, prefetchable, 256 MiB at 0xD0000000: the window onto the frame buffer carve-out |
| BAR 2 | memory, 64-bit, prefetchable, 2 MiB at 0xE0000000 (the doorbells) |
| BAR 4 | I/O, 256 bytes at 0xD000 |
| BAR 5 | memory, 32-bit, **not prefetchable, 512 KiB** at 0xFCB00000: the registers; MSI-X's table and PBA are in it at +0x42000 and +0x43000 |
| Command register | **0x0006 as the firmware leaves it** (memory and bus master on, I/O off): Linux logs `enabling device (0006 -> 0007)` when `amdgpu` takes it, and MIND Core's hardware report of 2026-11-19 read 0006 too. 0x0407 under `amdgpu`. |
| VBIOS | `113-CEZANNE-021`, fetched by Linux from the ACPI **VFCT** table; ATOM ROM header 2.2 at 0x188 |
| Display engine | DCN **2.1.0** (IP discovery: DMU, hw id 271), "Display Core v3.2.369 initialized on DCN 2.1" |
| Its register bases | discovery: **0x12, 0xC0, 0x34C0**, 0x9000, 0x2403C00 dwords: the segments `renoir_ip_offset.h` gives; a register's byte offset in BAR 5 is (segment base + offset) x 4 |
| SMU | MP1 v12.0.1, base 0x16000 dwords; SMC firmware 64.71.0 |
| Memory hub | MMHUB v1.5.0; GFX v9.3.0 |
| DMUB | firmware 0x0101002B, "Loading DMUB firmware via PSP", "DMUB hardware initialized" |
| The carve-out | "VRAM: 512M 0x000000F400000000 - 0x000000F41FFFFFFF", "RAM width 128bits DDR4"; GART 1024M at 0x0 |

**The carve-out's place in the GPU's address space** (0xF4_0000_0000) is not chosen by `amdgpu` on this APU: it reads it
from the memory hub's `MC_VM_FB_LOCATION_BASE/TOP` (`mmhub_v1_0_get_fb_location`), which the firmware set. On x86-64
APUs `amdgpu` reaches the carve-out at its system address rather than through BAR 0 (`gmc_v9_0_mc_init`); BAR 0
shows its first 256 MiB of 512 (to be confirmed: the inference is that BAR 0 starts at the carve-out's offset 0, where
the GOP's framebuffer is).

## The VBIOS's tables

Read with `scripts/hardware/amd_vbios_tables.py` (this repository), from the layouts of `atomfirmware.h`.

**Data tables** (index, name, format): 4 firmwareinfo 3.2, 6 lcd_info 2.1, 8 smu_info 3.1, 11 vram_usagebyfirmware
2.1 (**no reservation**: start 0, 0 KiB by firmware, 0 by driver), 12 gpio_pin_lut 2.1, 14 gfx_info 2.3,
15 powerplayinfo 9.1, 22 displayobjectinfo 1.4, 23 indirectioaccess 1.1 (5 bytes), 27 dce_info 4.3,
30 integratedsysteminfo **1.12**; and 3, 5, 7, 13, 17 that `atomfirmware.h` does not name.

**Command tables** (index, name, format, size): 0 asic_init 2.1 (460), **4 digxencodercontrol 1.5** (788),
**12 setpixelclock 1.7** (1052), **13 enabledisppowergating 2.1** (1230), 34 blankcrtc 1.1, 35 enablecrtc 1.1,
41 getsmuclockinfo 3.1, 42 selectcrtc_source 2.3, 46 setdceclock 2.2, 49 setcrtc_usingdtdtiming 1.1,
50 externalencodercontrol 2.3 (10 bytes: empty), 54 processi2cchanneltransaction 1.1, 60 computegpuclockparam 1.8,
**76 dig1transmittercontrol 1.6** (268), 78 processauxchanneltransaction 1.2; and the unnamed internal 6, 9, 19, 20,
21, 23, 33, 37, 39, 43, 44, 45, 51, 53, 56, 61, 75, 77 (4289 bytes, the largest: by its size likely what the transmitter
table calls for the PHY; not disassembled), 79. The revisions of 4, 12, 13 and 76 are those Display Core's
`command_table2.c` drives on DCN 2.1.

**The display paths** (displayobjectinfo 1.4, six paths; the records follow each):

| Path | Connector | Encoder, PHY | DDC (I2C record) | HPD (record) | Linux's name |
|---|---|---|---|---|---|
| 0 | `330C` HDMI type A, enum 3 | `211E` UNIPHY enum 1: **PHY A** | id 0x90: line 0, HW engine, `DC_GPIO_DDC1` | pin 1: `DC_GPIO_HPD_A` bit 0, HPD 1 | **HDMI-A-1** |
| 1 | `340C` HDMI type A, enum 4 | `221E` UNIPHY enum 2: PHY B | 0x91: line 1, `DC_GPIO_DDC2` | pin 2: bit 8, HPD 2 | HDMI-A-2 |
| 2 | none | `2120` UNIPHY1 enum 1: PHY C | 0x90 | pin 0 | (no connector) |
| 3 | `3113` DisplayPort, enum 1 | `2220` UNIPHY1 enum 2: PHY D | 0x92: line 2, `DC_GPIO_DDC3` | pin 3: bit 16, HPD 3 | DP-1 |
| 4 | `3213` DisplayPort, enum 2 | `2121` UNIPHY2 enum 1: **PHY E** | 0x93: line 3, `DC_GPIO_DDC4` | pin 4: bit 24, HPD 4 | **DP-2** |
| 5 | `7103` (a generic object) | none | its records are the look-up tables: HPD pins 1-5, AUX/DDC ids 0x90-0x94 | | |

- Encoder caps 0x0F on the HDMI paths and 0x10F on the DP ones (bit 8: a USB-C type connector).
- The GPIO table gives each pin's register: HPD pins 1-5 are bits 0, 8, 16, 24, 26 of `DC_GPIO_HPD_A` (dword 0x5DB5,
  BAR byte 0x176D4); DDC lines 0-4 are `DC_GPIO_DDC1_A` to `DC_GPIO_DDC5_A` (dwords 0x5D91 to 0x5DA1, step 4), bit 8.
  `hw_translate_dcn21.c` maps HPD_A's bits to HPD 1-6 and DDC1-5 to the I2C engine's lines.
- The integrated system information 1.12 lists the board's real outputs (`extdispconninfo` 1.1): paths 0, 1, 3 and 4,
  each with AUX/DDC and HPD look-up index equal to its order (0-3), channel mapping 0xE4 (lanes not swapped), no
  inversion, no external encoder or retimer, DP caps 0x0100. The DVI/HDMI tuning sets (TMDS, hdmiCLK5, hdmiCLK8) are
  all zero, the spread spectrum figures too.
- **Which connector is which**: `amdgpu_dm` makes one connector per link in link order, and Display Core makes the
  links from the paths that have a connector (0, 1, 3, 4). The DRM state lists HDMI-A-1, HDMI-A-2, DP-1, DP-2 in that
  order; `HDMI-A-1` is on `crtc-0`, `DP-2` on `crtc-1`; Display Core's dump shows "DP link training complete" on link 3
  only. So **HDMI-A-1 is path 0 (PHY A, DDC line 0, HPD 1)** and **DP-2 is path 4 (PHY E)**. Confidence: high (three
  agreeing observations; no register of the DIG back ends read).

## The monitors and their modes

| | HDMI-A-1 (dark under MIND Core) | DP-2 (the firmware's) |
|---|---|---|
| EDID | `TRG` product 0x3418, 2023, EDID 1.3, "KQ34C144C", 81 x 35 cm | `GSM` product 0x7717, 2022, EDID 1.4, "16MQ70", 34 x 22 cm, 10 bits a colour |
| Range limits | 48-101 Hz, 60-222 kHz, 680 MHz | 56-75 Hz, 30-130 kHz, 340 MHz |
| CTA extension | VICs 1-5, **16** (1080p60), 17-20, 31, 63, 93-97; HDMI 1.4 block (address 2.0.0.0, deep colour 30/36/48, 680 MHz) and HDMI Forum block (680 MHz): an HDMI 2.0 sink | VICs 16, 4, 3, 1, 31, 19, 18, 5, 20; no HDMI block |
| Preferred timing | **3440x1440 at 60 Hz, 324 MHz**: h 3440 + 48 + 32 + 80 = 3600, v 1440 + 3 + 5 + 52 = 1500, hsync +, vsync - | **2560x1600 at 59.97 Hz, 268.5 MHz**: h 2560 + 48 + 32 + 80 = 2720, v 1600 + 3 + 10 + 33 = 1646, hsync +, vsync - |
| Other detailed timings | 2560x1440 at 59.95 Hz (241.5 MHz), 3440x1440 at 100 Hz (540 MHz), 2560x1440 at 119.8 Hz (497 MHz) | 2560x1600 at 75 Hz (338 MHz), 2560x1440 at 59.95 Hz (241.5 MHz), 2560x1600 at 30 Hz (132.25 MHz) |
| Mode in use under Linux | the preferred one (`crtc-0`: 324000, 3440 3488 3520 3600, 1440 1443 1448 1500, +h -v) | the preferred one (`crtc-1`: 268500, 2560 2608 2640 2720, 1600 1603 1613 1646) |

The HDMI monitor's 324 MHz is below 340 MHz, so its preferred mode needs no HDMI 2.0 scrambling at 8 bits a colour, but
it is above one DVI link's 165 MHz: DVI signalling carries 1080p60 (148.5 MHz) and not 3440x1440.

## Linux's state with both monitors lit

From Display Core's dump (`amdgpu_dm_dtn_log`, 2026-11-21; identical in its OTG rows to 2026-11-20's):

- **Pipes.** All four HUBP/DPP pipes are in use, two per monitor by MPC combining: MPCC 0 (DPP 0) over MPCC 3 (DPP 3)
  into OPP 0 for the HDMI monitor (HUBP 0 and 3, 1720x1440 each); MPCC 1 (DPP 1) over MPCC 2 into OPP 1 for DP (HUBP 1
  and 2, 1280x1600 each). Every HUBP: format 8h, swizzle 19h (tiled), DCC on, not blanked; addr_hi **0**. DPP colour
  paths bypassed. DSC off on its three instances. No stream encoder carries DSC or secondary streams.
- **Timing generators** (as the dump prints them):

  | OTG | h_total | h_blank start/end | h_sync end, pol | v_total | v_blank start/end | v_sync end, pol | drives |
  |---|---|---|---|---|---|---|---|
  | 0 | 3599 | 3552 / 112 | 32, 0 (+) | 1499 | 1497 / 57 | 5, 1 (-) | HDMI-A-1 |
  | 1 | 2719 | 2672 / 112 | 32, 0 (+) | 1645 | 1643 / 43 | 10, 1 (-) | DP-2 |

  These are exactly the EDIDs' preferred timings in the OTG's convention (sync from 0, blank end after the back porch,
  totals minus one): `tests/display_host.rs` checks the arithmetic both ways.
- **Clocks** as Display Core calculated them: DISPCLK 328.876 MHz, DPPCLK 162.809 MHz, DCFCLK 400 MHz (deep sleep
  20.25 MHz), FCLK 400 MHz, SOCCLK 400 MHz.
- **Watermarks**: set 0 urgent 16.312 us, self-refresh enter/exit 84.333/19.895 us, DRAM clock change 28.020 us.
- **The framebuffers**: ARGB/ABGR 8888 with an AMD DCC modifier (0x200000440517901), 21,495,808 bytes for 3440x1440
  (pitch 13,824) and 17,301,504 for 2560x1600 (pitch 10,240), double-buffered; a 256x256 cursor; fbcon's linear
  3440x1600 XR24 buffer with pitch 13,824 (3456 pixels: a linear pitch rounded up to 64 pixels).
- **Where Linux scans out from**: HUBP's high address is 0 on all four pipes, so the surfaces are below 4 GiB in the
  GPU's address space, inside the GART (0 to 1 GiB), not in the carve-out (0xF4_xxxx_xxxx): **Linux scans out of system
  RAM through the GART's page tables.** That is not a model for MIND Core's first framebuffer, which needs no GPU page
  tables if it lies in the carve-out (below).

## What the firmware leaves for MIND Core

Known now:
- One output, `DP-2`, at 2560x1600, framebuffer at 0xD0000000 (BAR 0's base), stride 2560 pixels, BGR (the loader's
  lines on every boot, release 202611211156: `GOP 0: 2560x1600 STRIDE 2560 BGR FB 0x00000000D0000000 CONSOLE`). By the
  paths above, that is **PHY E** with DIG back end E.
- The command register 0x0006; BAR 5 512 KiB; MIND Core's kernel leaves display functions' bus mastering on
  (`kernel/src/pci.rs`, `stop_mastering`).
- The carve-out's GPU address, which the firmware set and Linux only read: 0xF4_0000_0000, 512 MiB.

Not known, and needed by the route: which OTG, OPP, HUBP and DIG front end the GOP uses for DP-2 (Linux's assignment
says nothing of the firmware's), its HUBP surface address, `DCN_VM_FB_LOCATION_*`, the pixel clock source it chose
(PHY E's PLL), DISPCLK, which pipes are powered, the HDMI connector's HPD sense under MIND Core, and whether the GOP
left the DMCUB running. **Two ways to close it:**

Closed by way 1 except for the DMCUB, which step 1 does not read (213-DRV-0086 step 1, runs 1 to 7, 2026-11-21).
- **The pipe:** OTG0 from OPP0, DIG front end E into PHY E's back end (DP, `DIG_MODE` 0, HPD 4), HUBP0 at
  0xF4_0000_0000.
- **Clocks:** the pixel clock from OTG0's DP DTO, not a PHY PLL; DISPCLK and DPPCLK at divider id 0x14, settled.
- **Power:** all four pipes' HUBP and DPP powered, HUBP1 to HUBP3 blanked on their own idle OTGs.
- **Aperture:** `DCN_VM_FB_LOCATION` 0xF4_0000_0000 to 0xF4_1FFF_FFFF, `DCN_VM_FB_OFFSET` 0x8_1000_0000.
- **HPD:** sensed on HPD 1 (HDMI-A-1) and HPD 4 (DP-2).

The issue has each value.

1. **A read under MIND Core**, by `display` with KRN's grant of BAR 5 (213-DRV-0086 step 1), in the batches proposed
   below, each sent to the log before the next. Preferred: it is the very state MIND Core inherits.
2. **Linux started once without `amdgpu`** (for example `modprobe.blacklist=amdgpu`), so that the GOP's state stays,
   and the same batches read there. Not done: it changes the boot parameters, and register reads under Linux are now
   forbidden on this bench.

## The DMUB question

**Answer: a TMDS mode set on DCN 2.1 does not need the DMUB, by Linux's source; it needs the VBIOS command tables,
which the host can run itself. Not shown on this machine.** The evidence:

1. Display Core uses the DMUB on DCN 2.1 for: ABM and PSR (`dmub_abm`, `dmub_psr`: eDP panels; this machine has none),
   the hardware lock manager (for PSR and replay links), and four VBIOS command tables when
   `debug.dmub_command_table` is set: encoder control, transmitter control, set pixel clock and display power gating
   (`command_table2.c`: each `if (dmub_srv && dmub_command_table)` sends a DMUB command, else runs the table through
   `amdgpu_atom_execute_table`, the host's ATOM interpreter).
2. `dmub_command_table` is the default on DCN 2.1 (`dcn21_resource.c`, `debug_defaults_drv`), and is turned off when
   Display Core uses the older DMCU instead: `if (!dc->config.disable_dmcu) { ...dcn21_dmcu_create...;
   dc->debug.dmub_command_table = false; }`.
3. `amdgpu_dm.c` sets `disable_dmcu = false` on DCN 2.1 for DMUB firmware versions 0, 0x1 and 0x01000000 (early
   linux-firmware releases), and true for any later one. So Linux itself ran DCN 2.1 with the four tables executed by
   the host's interpreter; with today's 0x0101002B they go to the DMUB.
4. Nothing else in the TMDS path (OTG, OPP, DIG, DCCG writes) goes through the DMUB.

So the route runs the command tables on the CPU, as Linux did with early firmware, and leaves the DMUB alone. Its
firmware (`amdgpu/green_sardine_dmcub.bin`, loaded through the PSP) would only be needed for PSR/ABM, which no monitor
here uses. What is still missing to call it settled: (a) the route's own step 3 on this machine; (b) whether the GOP
left the DMCUB running (not needed for the route, and left out of the proposed reads). Linux also requires the
DMUB file to load its display driver at all (`AMDGPU_UCODE_REQUIRED`); that is Linux's choice, not the hardware's.

What running the tables on the CPU takes: an ATOM bytecode interpreter in `display` (Linux's `amdgpu/atom.c`, MIT, is
some 1500 lines: opcodes for register, parameter, workspace, data-table and I/O accesses, jumps, calls to other
tables, delays), the VBIOS image (from the ACPI VFCT table, as Linux takes it), and a guard: the tables write registers
anywhere in BAR 5 (and, through `indirectioaccess`, indirectly), so every access is checked against the engine's
window (DCN bytes 0x48 to 0x1B000, the SMU mailbox) and logged. Before any table runs on the machine, the same
interpreter can run each table on LEG9 against a register model with the image, to list exactly what it writes for
first light's parameters: that answers "what the tables write" without hardware.

## A route to first light on HDMI-A-1

The smallest the evidence supports. Each step names its source here; none has run (MC-12.3). `display/src/dcn.rs`
holds steps 4 to 10 as an ordered plan, checked against a model by `tests/display_host.rs`.

0. **Read first** (213-DRV-0086 step 1, the batches below): the firmware's OTG/OPP/DIG front end for DP-2, the powered
   pipes, DISPCLK, `DCN_VM_FB_LOCATION_*`, HPD 1's sense. Everything after leaves those instances alone and is checked
   against them after each step ("What the firmware leaves").
1. **The output**: path 0, connector HDMI type A (object id byte 0x0C), encoder UNIPHY (0x1E) on **PHY A**, DIG back
   end A, HPD 1 (HPD block 0, `DC_HPD_INT_STATUS.DC_HPD_SENSE`), DDC on I2C line 0 (`DC_GPIO_DDC1`) ("The display
   paths").
2. **The monitor**: its EDID over DDC (later; Linux's copy gives it now), parsed by `display/src/edid.rs`.
3. **The mode**: CTA 1080p60 (VIC 16, which the monitor lists), 148.5 MHz, in **DVI signalling** (DIG mode 2, every
   HDMI sink takes it; no infoframes): within one TMDS link and far below the DISPCLK the firmware runs for 2560x1600
   ("The monitors", "Clocks"). The monitor's own 3440x1440 at 324 MHz comes after, in HDMI mode without scrambling, and
   may need a higher DISPCLK (an SMU message, `VBIOSSMC_MSG_SetDispclkFreq` = 4 through `MP1_SMN_C2PMSG_67/83/91`,
   `rn_clk_mgr_vbios_smu.c`).
4. **The pipe**: the lowest timing generator the firmware leaves off (by Linux's numbering likely OTG 1 if the GOP took
   OTG 0), its OPP of the same number, DIG front end A. No HUBP, DPP or MPC: their power domains stay as they are.
5. **The OTG's clocks**: `ODMn_OPTC_INPUT_CLOCK_CONTROL` (CLK_EN, GATE_DIS; wait CLK_ON), `OTGn_OTG_CLOCK_CONTROL`
   (likewise) (`optc1_enable_optc_clock`). Plain register writes.
6. **The pixel clock**: VBIOS table **12 setpixelclock 1.7**: 1,485,000 x 100 Hz, PLL `ATOM_COMBOPHY_PLL0` (20, PHY A's
   PLL), encoder 0x1E, mode DVI, CRTC n; then `PHYPLLA_PIXCLK_RESYNC_CNTL.PHYPLLA_DCCG_DEEP_COLOR_CNTL` = 0 (8 bits)
   (`dce112_program_pix_clk`). The table chooses the OTG's pixel rate source (`OTGn_PIXEL_RATE_CNTL`,
   `OTGn_PHYPLL_PIXEL_RATE_CNTL`); TMDS needs no DP DTO.
7. **The timing**: `OTGn_OTG_H_TOTAL` 2199, `H_SYNC_A` 0/44, `H_BLANK_START_END` 2112/192, `V_TOTAL` (and `_MIN`, `_MAX`)
   1124, `V_SYNC_A` 0/5, `V_BLANK_START_END` 1121/41, both polarities positive, no interlace, `VTGn_CONTROL`
   `VCOUNT_INIT` 1121, global sync (`VSTARTUP_PARAM` inside the blank), RGB (`OPTC_DATA_FORMAT` 0), no divide-by-two
   (`optc1_program_timing`). Plain register writes; the arithmetic is `display/src/timing.rs`.
8. **The test pattern**: `OPP_PIPEn_OPP_PIPE_CONTROL.OPP_PIPE_CLOCK_EN`, then the OPP's **display pattern generator**:
   `DPGn_DPG_DIMENSIONS` 1920x1080, `DPG_OFFSET_SEGMENT` 0, `DPG_CONTROL` = EN, mode 0 (RGB colour squares), VESA
   range, 8 bits, VRES/HRES 6 (`opp2_set_disp_pattern_generator`). DCN 2.1's OTG has no pattern of its own (no
   `OTG_TEST_PATTERN_*` register in `dcn_2_1_0_offset.h`); the DPG needs no framebuffer.
9. **Start**: `ODMn_OPTC_DATA_SOURCE_SELECT.OPTC_SEG0_SRC_SEL` = OPP n, `VTGn_CONTROL.VTG0_ENABLE`, `OTGn_OTG_CONTROL`
   `DISABLE_POINT_CNTL` 3 and `MASTER_EN` (`optc2_enable_crtc`); wait for `OTG_CURRENT_MASTER_EN_STATE` and two moves
   of `OTG_STATUS_FRAME_COUNT`.
10. **The encoder and the PHY**: VBIOS table **4 digxencodercontrol 1.5**, stream setup (0x0F) for DIG A, DVI, 4 lanes,
    14,850 x 10 kHz, 8 bits; `DIG0_DIG_FE_CNTL.DIG_SOURCE_SELECT` = OTG n, RGB 8 bits; `DIG0_DIG_BE_CNTL` mode 2 (DVI)
    and front end A; VBIOS table **76 dig1transmittercontrol 1.6**, action ENABLE for PHY A: DVI, 4 lanes, symbol clock
    14,850 x 10 kHz, HPD 1, front end A, connector 0x0C (`dcn10_link_encoder_enable_tmds_output`); then a `DIG_START`
    pulse.
11. **Seen**: the maintainer sees colour squares on the HDMI monitor while the DP monitor keeps MIND Core's screen;
    the firmware's registers read the same after as before.

What goes where: **VBIOS command tables** for the pixel clock (12), the DIG stream setup (4) and the PHY (76), run by
an interpreter on the CPU; **plain register writes** for the OTG, OPTC, VTG, OPP, DPG, DIG front and back end wiring
and the deep colour resync; **SMU messages** only for a higher DISPCLK later; **DMUB** not at all. Not done by the
route: Display Core's "golden init" (`dcn10_bios_golden_init`: table 13 for the whole engine and each pipe), which would
turn the firmware's pipe off too.

## The register map used

- Segments 0x12, 0xC0, 0x34C0 dwords (DCN_BASE; confirmed by the IP discovery table). DCCG's registers are in segment
  1, the rest of the engine in segment 2; the engine spans BAR 5 bytes 0x48 to about 0x1B000, the SMU mailbox is at
  0x58A0C/0x58A4C/0x58A6C.
- Instance strides (dwords): OTG 0x80 (OTG0_OTG_H_TOTAL 0x1B2A), ODM/OPTC 0x10 (0x1ACA), VTG 1 (0x528), OPP_PIPE and
  DPG 0x5A (0x188C, 0x1854), DIG and DP 0x100 (DIG0_DIG_FE_CNTL 0x2068, DIG0_DIG_BE_CNTL 0x20AF), HPD 8 (0x1F14),
  HUBP 0xDC (0x5E5), DPP 0x16B (0xCC5), MPCC 0x1B (0x1271), DCCG's OTGn_PIXEL_RATE_CNTL 4 (0x80); PHYPLLA-D
  resync 0x40-0x43 and PHYPLLE 0x4C.
- Fields as `dcn_2_1_0_sh_mask.h` gives them; those used are named in `display/src/dcn.rs`.

## Registers the route needs that Display Core's dump does not give, and a read proposed as safe

Proposed for 213-DRV-0086 step 1 **under MIND Core, with KRN's grant of BAR 5, and only on the owner session's word**:
one block a batch, each batch's values written to the log before the next starts, so that a hang names its batch.
"Read by Display Core" means Linux's own driver reads it in normal running on this machine, which did not hang.

| Batch | Registers (instance 0's dword; all instances) | Why it is safe to read |
|---|---|---|
| 1. HPD | `HPDn_DC_HPD_INT_STATUS` (0x1F14 + 8n), `HPDn_DC_HPD_CONTROL` (0x1F16 + 8n), n = 0-4 | Display Core reads the HPD sense on every detection; the HPD block is always on |
| 2. OTG state | for n = 0-3, what `optc1_read_otg_state` reads: `OTG_CONTROL`, `OTG_H_TOTAL`, `OTG_H_BLANK_START_END`, `OTG_H_SYNC_A`, `OTG_H_SYNC_A_CNTL`, `OTG_V_TOTAL`, `OTG_V_TOTAL_MIN/MAX/CONTROL`, `OTG_V_BLANK_START_END`, `OTG_V_SYNC_A`, `OTG_V_SYNC_A_CNTL`, `OTG_VERTICAL_INTERRUPT1/2_CONTROL/POSITION`, `ODMn_OPTC_INPUT_GLOBAL_CONTROL`; plus `OTG_CLOCK_CONTROL` (0x1B86) and `ODMn_OPTC_DATA_SOURCE_SELECT` (0x1ACB) | Display Core's dump reads these for all four OTGs, enabled or not, and did three times on this machine without harm |
| 3. Power | `DOMAINn_PG_STATUS`, n = 0-7 (segment 2, 0x81 + 2n) | Display Core polls them whenever it gates or ungates a pipe (`hubp_pg_control`, `dpp_pg_control`) |
| 4. HUBP | for each pipe whose domain batch 3 shows powered only: `HUBPREQn_DCSURF_PRIMARY_SURFACE_ADDRESS` and `_HIGH`, `HUBPn_DCSURF_SURFACE_CONFIG`, `HUBPREQn_DCSURF_SURFACE_PITCH`, `HUBPn_DCHUBP_CNTL` | the dump reads every HUBP, but under Linux all four were powered; a gated one is not read |
| 5. DIG | `DIGn_DIG_FE_CNTL` (0x2068 + 0x100n), n = 0-4; `DIGn_DIG_BE_CNTL` and `DIGn_DIG_BE_EN_CNTL` (0x20AF/0x20B0 + 0x100n) for n = 0, 1, 3, 4 (the board's PHYs A, B, D, E) | Display Core reads every link's back end at start (`is_dig_enabled`, `get_dig_frontend`) and every front end in its dump |
| 6. Address space | `DCN_VM_FB_LOCATION_BASE`, `_TOP`, `DCN_VM_FB_OFFSET`, `DCN_VM_AGP_BOT/TOP/BASE` (segment 2, 0x493-0x498) | the DCHUBBUB's own registers, which Display Core writes at start; its domain is never gated while a pipe runs |
| 7. Clocks | `OTGn_PIXEL_RATE_CNTL`, `OTGn_PHYPLL_PIXEL_RATE_CNTL` (segment 1, 0x80/0x83 + 4n), `PHYPLLx_PIXCLK_RESYNC_CNTL` (0x40-0x43, 0x4C), `DENTIST_DISPCLK_CNTL` (0x64) | DCCG is the clock controller the others are gated by; Display Core writes and reads these in every mode set. The weakest argument here: read last |

**Left out, by the owner session's word and by their risk**: the PHYs' DPCS/RDPCS, DMCU and DMCUB, DSC, every mailbox
(SMU, DMUB inbox/outbox), AUX and I2C data, VGA, audio, interrupt and performance-counter blocks. Values the route
needs from those (the PHY's state, the PLLs' lock) come from the VBIOS tables' own waits, not from reads of ours.

## The memory a second framebuffer needs (for KRN)

- **The carve-out through BAR 0**, after the GOP's framebuffer: the engine scans the carve-out through its own system
  aperture (`DCN_VM_FB_LOCATION_BASE/TOP`, in 16 MiB units, filled by `hubbub21_init_dchub` from the memory hub's
  values), with **no GPU page table, no GART, no IOMMU mapping and no PCI bus mastering of host memory**. A buffer at
  BAR 0 offset X is scanned out at GPU address `FB_LOCATION_BASE + X` (0xF4_0000_0000 + X if batch 6 confirms the
  firmware's base and that BAR 0 starts at the carve-out's offset 0).
- Room: the GOP's framebuffer uses BAR 0 [0, 0xFA0000). 3440x1440 linear at 4 bytes with the pitch rounded to 3456
  pixels (13,824 bytes, as Linux's fbcon did) is 19,906,560 bytes; at offset 0x1000000 (16 MiB) it ends below
  0x2300000, inside BAR 0's 256 MiB. The VBIOS reserves nothing (vram_usagebyfirmware is empty); Linux put its PSP TMR
  and page tables at the carve-out's top, beyond BAR 0.
- System RAM instead would need the GART (VM context 0's page table, which `amdgpu` keeps in the carve-out) or the AGP
  window set up in the memory hub that the graphics engine shares: the reason to prefer the carve-out.
- Confidence: medium. The facts are from Linux's source and boot lines and the GOP's own framebuffer at BAR 0's base;
  the firmware's `DCN_VM_FB_LOCATION_*` (0xF4_0000_0000, 512 MiB) and its HUBP address (0xF4_0000_0000, BAR 0's offset 0)
  were read under MIND Core since (213-DRV-0086 step 1, batches 6 and 4) and agree.
- **The frame pool keeps out of it:** MIND Core's hardware report (hw0054, release 202611211819) lists the firmware's
  memory map ending with a reserved region 0x8_0E30_0000 to 0x8_2FFF_FFFF (541 MiB) below `TOP_MEM2` 0x8_3000_0000,
  room for the 512 MiB carve-out from 0x8_1000_0000; the frame pool (31,982 MiB) takes only conventional memory.
  Its exact start, batch 6's `DCN_VM_FB_OFFSET`, reads 0x810: system address 0x8_1000_0000 (213-DRV-0086, run 7).

## What first light's VBIOS tables write (213-DRV-0086, on LEG9 against a model)

`display/src/atom.rs` interprets ATOM command tables (the bytecode as Linux v7.0's `amdgpu/atom.c` describes it; the
code is this project's). `tests/display_atom_host.rs` runs tables 12, 4 and 76 of the Ryzen's image (given by
`MIND_AMD_VBIOS`; the case says it skips without it) with the parameters `dcn::first_light` builds for 1080p60 in DVI on
PHY A through OTG 1 and DIG front end A, against registers kept as memory. Nothing ran on the machine. Sources added to the list above (Linux v7.0, first 16 hex digits of SHA-256): `amdgpu/atom.h` 6157ba431d5fd2ce, `include/asic_reg/dpcs/dpcs_2_1_0_offset.h` b3d7d93daa72e809, `include/asic_reg/dpcs/dpcs_2_1_0_sh_mask.h` 50c813a5c9290309.

**The interpreter's bounds.** Before a table runs: its index within the master command table, its entry non-empty, its
header and length within the image. While it runs: every opcode and operand fetched from inside that table, every jump
and SWITCH target inside it, every data read (`ID`) inside the image, the work space within the table's own size, the
parameter space within 64 dwords, the scratch within 20 KiB; a register only in direct (MM) mode and only through
`Reg::new` (the engine's bytes and the SMU mailbox, never the MSI-X bytes), with the register block added before the
check; no PCI, system or indirect I/O and no PLL or MC argument; unknown opcodes, REPEAT, SAVEREG and RESTOREREG
refused; at most 1,000,000 instructions, 8 nested calls and 5 s of delays a run. Where Linux carries on silently (an
unknown opcode ends the table, an index out of range reads 0xCDCDCDCD), this one stops with an error naming the table
and the offset. 11 host cases cover the opcodes on synthetic images and each bound.

**Four statuses the model answers for the hardware** (assumptions, each where a table waits on it):

| Register (PHY A) | Bit | Awaited | Bound in the table |
|---|---|---|---|
| `RDPCSTX0_RDPCSTX_PHY_CNTL0` | 28 `RDPCS_SRAM_INIT_DONE` | set (table 77 at 0xB864) | **none** |
| `RDPCSTX0_RDPCSTX_PHY_CNTL3` | 5, 13, 21, 29 `TX_ACK` of each lane | equal to the lane's `TX_REQ` (0xBAFB, 0xBE94) | none, but leaves if `DMCU_DPALT_DIS_BLOCK_REG` bit 0 is set |
| `RDPCSTX0_RDPCSTX_PHY_CNTL12` | 7 `RDPCS_PHY_DP_MPLLB_STATE` | set (0xBB9A) | 100,000 x 2 us |
| `DPCSTX0_DPCSTX_TX_CLOCK_CNTL`, `RDPCSTX0_RDPCSTX_CLOCK_CNTL` | 2 `SYMCLK_CLOCK_ON`; 10 `SYMCLK_DIV2_CLOCK_ON` | set after their enables (0xBBE4) | **none** |

`RDPCSTX_CLOCK_CNTL`'s `SRAMCLK_CLOCK_ON` (bit 14) is awaited **clear** after `SRAMCLK_EN` is set (0xB84A); the model
leaves it as written.

**Result:** all three end. Table 12 (setpixelclock 1.7): 530 instructions, 4 tables called (it calls the internal
table 77, the PHY's own sequence), 10 us of delays, 111 writes to 28 registers. Table 4 (digxencodercontrol 1.5): 31
instructions, 5 writes to 3 registers. Table 76 (dig1transmittercontrol 1.6): 286 instructions, 9 calls, 420 us,
23 writes to 13 registers.

| Table | What it writes |
|---|---|
| 12 | PHY A's `RDPCSTX0`: `PHY_CNTL0` to `PHY_CNTL14`, `PHY_FUSE0` to `3`, `CLOCK_CNTL`, `CNTL`, `SCRATCH`, `PLL_UPDATE_DATA`, and its control-register bus `RDPCS_TX_CR_ADDR`/`CR_DATA` (20 and 17 writes); `DPCSTX0_DPCSTX_TX_CLOCK_CNTL`; in DCCG `SYMCLKA_CLOCK_ENABLE` = 1, `PHYPLLA_PIXCLK_RESYNC_CNTL` = 0x101, `OTG1_PIXEL_RATE_CNTL` = 0 and `OTG1_PHYPLL_PIXEL_RATE_CNTL` = 0 |
| 4 | `DIG0_DIG_FE_CNTL` (three writes, last 0), `DIG0_DIG_CLOCK_PATTERN` = 0x1F, `DIG0_TMDS_CTL_BITS` = 0 |
| 76 | `DIG0_DIG_BE_CNTL` = 0x00020100 (mode 2, DVI; front end A), `DIG0_DIG_BE_EN_CNTL` = 1, `DIG0_DIG_LANE_ENABLE` = 0x10F, `UNIPHYA_LINK_CNTL`, `UNIPHYA_CHANNEL_XBAR_CNTL`, `RDPCSTX0`'s `PHY_CNTL3` (7), `PHY_CNTL6`, `PHY_FUSE0` to `3`, `RDPCSTX_CNTL`, `DPCSTX0_DPCSTX_TX_CNTL` |

All of them lie in the engine's window KRN grants (BAR 5 [0, 0x1B000), 213-KRN-0271); none needs the mailbox's page. The case asserts that every write lies on PHY A's path (DPCSTX0/RDPCSTX0, DIG0, UNIPHYA, PHYPLLA, OTG 1's pixel rate,
SYMCLKA) and none on the firmware's pipe 0, DIG E or PHY E. No table touches the SMU mailbox, DISPCLK or any OTG
timing. The register names are from `dcn_2_1_0_offset.h` and `dpcs_2_1_0_offset.h`.

What follows for the route (plans, not guarantees):
- **The tables reach PHY A's RDPCS and DPCS**, the PHY interface this survey left out of the proposed read as a likely
  hang. Under Linux the same path is driven for HDMI-A-1 (by the DMUB's own copy of these tables) without a hang, and
  the tables enable each clock before they use the block. Still, step 3's first run should log each table's name before
  it runs, as step 1 does for its batches.
- **Two waits have no bound of their own** (SRAM init done, the symbol clock on): on the machine a PHY that never answers
  would hold the CPU there. The interpreter's step limit bounds it (a run stops with `StepLimit` naming the offset).
- Table 4 leaves `DIG0_DIG_FE_CNTL` at 0, so the plan's own write of the front end's source afterwards is needed; table
  76 writes `DIG0_DIG_BE_CNTL` itself (the plan's earlier write of it is redundant, not wrong).

## Step 3 as code: first light in eight stages (213-DRV-0086)

`display/src/light.rs`, switched off (`STAGES_ALLOWED = 0` in `main.rs`): the route above as eight stages that a build
lets go one at a time: 1 the OTG's clocks, 2 the pixel clock (table 12), 3 the 1080p60 timing, 4 the OPP's colour
squares, 5 the OTG started and its frame count seen to move, 6 the DIG front end (table 4), 7 the back end in DVI
mode, 8 the PHY (table 76) and the `DIG_START` pulse.

- **The pipe** is chosen from step 1's values (all seven batches read on the same boot, so that step 3 touches no
  register step 1 has not read): the one timing generator running and its OPP,
  the one enabled back end among PHYs A, B, D, E and its front end, the HUBPs following that generator, HPD 1's sense;
  then the lowest free OTG with its own OPP, and the lowest free front end that is not the firmware's back end's.
- **The guard** lets a write through only to the chosen pipe's blocks (OTG, ODM, VTG, OPP_PIPE, DPG, the two DIG
  blocks, UNIPHY, the PHY's DPCS/RDPCS, its PLL resync, the OTG's pixel rate, SYMCLK in DCCG) and refuses first the
  firmware's (the same blocks of its pipe and PHY, and the HUBP, DPP and MPCC of each HUBP that follows its OTG). The
  write path is its own trait, taking only a `Writable` the guard made; the interpreter's writes go through it too.
- **The firmware's pipe** (30 registers on the expected state, all read by step 1) is read before step 3 and after each
  stage; any change but of known status bits fails the stage. The status bits are whole fields by the DCN 2.1 header's
  masks: `OTG_BUSY`, `OPTC_DOUBLE_BUFFER_PENDING`, the DIG front end's metadata-missed flag, and `DCHUBP_CNTL`'s
  outstanding-request, in-blank, XRQ (0xF0000), timeout (0xF00000) and underflow (0x70000000) statuses. On the Ryzen,
  HUBP0's XRQ field reads F (step 1's run 5).
- **The HUBP that follows step 3's OTG** (on the Ryzen HUBP1: powered, blanked, `HUBP_VTG_SEL` 1) is never written; it
  must be blanked for step 3 to run, and its `DCHUBP_CNTL` is read after stages 5 to 8 under the same masks.
- **Undo** (of a failed stage and every one before it, last first):
  - first the off call: stage 2 calls table 12 with pixel clock 0 (as Display Core powers a PLL down), stage 8 table
    76's DISABLE;
  - then every register the stage wrote is put back;
  - then every register only the off call wrote, so all are as before the stage;
  - PHY A's control register bus is left as it is.

  The guard knows the two registers table 76's DISABLE writes outside the DIG blocks: `DPn_DP_LINK_CNTL` and
  `DPn_DP_DPHY_TRAINING_PATTERN_SEL` of the output's own DIG, both zeroed. So stage 8's off call runs to its end (on
  the model: 160 steps, 6 calls, 18 writes).

What the model showed about the undo, with the Ryzen's tables:
- Each of tables 12, 4 and 76 failing at any of its reads (231, 42 and 86 places on the model) is undone whole: every
  register as before (the bus aside), the firmware's pipe unchanged, the off calls run to their end. Table 76's one
  wait is the lanes' `TX_ACK` (0x2943 + 0xD8p). If it never comes, stage 8 fails at its time limit; its DISABLE waits
  for the same and stops too, and the restore still returns every register.
- Table 12's power-down call writes **nothing** when the stage stopped early (at the SRAM wait: 41 instructions, 0
  writes), and **14 writes** after a table 12 that ran to its end. So the restore of the stage's own writes, not the
  call, is what returns the registers to their state before step 3.
- A completed table 12 makes 37 of its writes through PHY A's control register bus (`RDPCS_TX_CR_ADDR`/`_DATA`: 20
  address and 17 data writes). The two registers are bare 16-bit fields (`dpcs_2_1_0_sh_mask.h`), and a write to the
  data register is itself the write into the PHY. So a restore cannot undo them: putting the data register back would
  write a stale value into the PHY. The undo leaves the bus as it is and says so in the log. What table 12 set inside
  PHY A stays until something rewrites it.
