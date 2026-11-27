# 213-DRV-0113 — `display`: both HDMI sockets lit at once (three screens)

**Type:** driver (`display`), design · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-27), a design only, no code · **Blocked by:** for stage 4, `KRN`: `display.wit` 1.5, the compositor's second driver screen, and init's second window of BAR 0 (the asks below) · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer wants three screens at once on the Ryzen:
- DP-2, the firmware's;
- HDMI-A-1;
- HDMI-A-2, with a third monitor in it since 2026-11-26.

`display` lights one HDMI socket at a time
([213-DRV-0112](213-DRV-0112-display-a-monitor-on-the-ryzens-other-hdmi-socket.md)). With a monitor in each, the one not
taken is named `NOT DRIVEN ... DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW`.

The limit is `display`'s and its interface's, not the engine's:
- **The pipes.** DCN 2.1 has four timing generators, OPPs, HUBPs and DPPs and five stream encoders (`dcn21_resource.c`
  `res_cap`; `pipe_count` is four less any pipe fused off).
- **The firmware's state.** It leaves all four HUBPs and DPPs powered, HUBP1 to HUBP3 blanked on their own idle OTGs
  (`hwdocs/cezanne-display.md`, "Power").
- **Linux** drove two monitors on all four pipes.
- **What is taken today:** the firmware's picture takes one pipe and `display`'s second screen another, so two are free.

This file is a design from the code and Linux's Display Core as they are (Linux v7.0). Nothing of it is built, and none
of it is a guarantee (MC-12.3).

## The pipe a second HDMI output would take

| | The firmware's (DP-2) | HDMI-A-1 today | HDMI-A-2 beside it, proposed |
|---|---|---|---|
| OTG and ODM | 0 | 1 (`assess`: the lowest OTG not running whose OPP is free) | 2, by the same rule once the firmware's pipe is told apart from display's (below) |
| OPP and DPG | 0 | 1 | 2 |
| HUBP, DPP, MPCC, MPC_OUT mux | 0 (`EXPECTED`) | 1 (`pipe_plan`: the instance of its OTG) | 2 |
| DIG front end | E | A (its own letter) | B (its own letter, as Display Core pairs a link with its own DIG) |
| DIG back end and PHY | E | A | B |
| PHY PLL, SYMCLK, OTG pixel rate | E's | A's, OTG 1's | B's, OTG 2's |
| Surface | the GOP's, BAR 0 [0, 0xFA0000) | BAR 0 slot 5, [0x1000000, 0x2400000) | a second range of BAR 0, asked of KRN (below) |

**The firmware's state of pipe 2, as known.** From step 1 on the Ryzen (213-DRV-0086 runs 3 to 7, 213-DRV-0092 boot 70,
213-DRV-0112 boot 108):
- OTG2 is not running.
- HUBP2's and DPP2's domains are powered, and HUBP2 is blanked, following OTG2.
- MPCC2 is wired from DPP2 to OPP2 (boot 70: every MPCC n from DPP n to OPP n).
- DIG back end B is not enabled, PHY PLL B's pixel clock is off, DDC2 is idle, and both DDC pads are in I2C mode.

**What must be read first, nothing written,** in a bench build in the survey's order, with HDMI-A-1 lit:
- **Step 1 again with pipe 1 running.** Batch 2 then finds two OTGs running. Today that empties batches 9 and 10, which
  name the firmware's pipe only when one OTG runs; see "single today".
- **Batch 10's DPPCLK DTO for DPP2** (`DPPCLKn_DTO_PARAM`) as the firmware leaves it. Pipe 1 lit with DTO1 as found, and
  DTO2's value is in the boot logs' `B10 DCCG` lines but not yet looked at.
- **MPC_OUT mux 2 and MPCC2** (batch 10 reads all four), to confirm that `pipe_plan`'s checks pass for instance 2.
- **DPP2's own registers.** Step 1 reads the firmware's DPP only; the stages read each register they write before
  writing it, as for pipe 1.
- **OTG2's `OTG_VERTICAL_INTERRUPT2_POSITION`,** where the pipe's restart mark would go (batch 2 reads it).

## What in `display` is single today, and must become per output

- **The choice** (`start.rs` `choose`, `light.rs` `assess_for`).
  - Today one output is taken.
  - `assess_for` refuses unless exactly one OTG runs (`RunningOtgs`) and exactly one board back end is enabled
    (`BackEnds`). With HDMI-A-1 lit, both are two, so a second output is refused.
  - Proposed: the firmware's pipe found once, at display's first start, and kept. Display's own pipes are told by their
    restart mark (`RESTART_MARK` in the OTG's `OTG_VERTICAL_INTERRUPT2_POSITION`). Free pipes and front ends are what
    neither uses.
- **Step 1's batches 9 and 10** (`batches.rs` `Known::running_otg`, `firmware_pipe`). They name the firmware's pipe only
  when one OTG runs; they must exclude OTGs that carry display's mark.
- **The run and its undo** (`start.rs` `Run`, `light.rs` `Light`, `Journal`).
  - One `Light`, one journal and one interpreter today. Proposed: one `Run` per lit output.
  - The interpreter's scratch persists across tables. The two runs never run tables at the same time (`display` is one
    thread), so one interpreter could serve both; whether to keep one each is a memory question (`Interpreter` and
    `Journal` are boxed).
- **The restart mark:** already in the output's own OTG; per output as it stands.
- **The guard** (`light.rs` `Guard`).
  - Today it denies the firmware's blocks and the other socket's, and allows the chosen pipe's.
  - With two pipes, each run's guard must also deny the other lit pipe's OTG, ODM, VTG, OPP and DPG, HUBP, DPP and MPCC,
    its MPC_OUT mux, its DIG blocks, PHY, PLL resync and SYMCLK. Each pipe is then refused to the other as the
    firmware's is.
- **The watch** (`light.rs` `Watch`).
  - Today the firmware's 30 registers are read before and after each stage.
  - While a second pipe's stages run, the lit pipe's registers (its OTG timing, OPTC source, HUBP surface and
    `DCHUBP_CNTL`) must be watched the same way, so that a stage that moves HDMI-A-1's picture fails and is undone.
- **The follower** (`start.rs` `Follower`, `hpd.rs` `Pair`).
  - Today one state and one lit socket. Proposed: a state per socket; each unplug takes back only its own run; a plug
    lights its socket if a pipe and a screen are free.
  - `Pair`'s `Busy` becomes a light-up when a pipe is free.
- **The standing check** (`light.rs` `standing_check`). Today it reads the firmware's HUBP and ODM and pipe 1's. Proposed:
  the firmware's, then each lit pipe's, each against its own stage 9 statuses; an underflow undoes the pipe it is on.
- **The surface and the offer** (`main.rs` `Writer`, `light.rs` `SURFACE_OFFSET`, `screen.rs` `Compositor`).
  - Today one canvas (slot 5), one surface offset, one `offer-screen`.
  - Proposed: a surface per output, each in its own granted range, each offered as its own screen (the interface asks
    below).
- **Stage 9's reference** measures the firmware's OTG and derives pipe n's DLG and TTU from the firmware's pipe. It is
  per output already; it measures the same pipe each time.

## What the pipes share, and its limits

- **DISPCLK and DPPCLK.**
  - `display` sets neither; it runs on what the firmware set (DENTIST divider id 0x14, 213-DRV-0086 batch 7).
  - Display Core sets both, and the DCFCLK hard minimum and deep-sleep minimum, through the SMU
    (`rn_clk_mgr_vbios_smu.c`: `VBIOSSMC_MSG_SetDispclkFreq`, `SetDppclkFreq`, `SetHardMinDcfclkByFreq`,
    `SetMinDeepSleepDcfclk`), from what its DML computes for all pipes (`rn_clk_mgr.c`).
  - DISPCLK must cover the highest pixel rate of one pipe, not their sum. HDMI-A-1 at 324 MHz lit with the firmware's,
    so a second 324 MHz pipe is not expected to need more. That is an inference, not a measure.
  - DPPCLK reaches each DPP through its DTO: DPP2's DTO as found is to be read first (above).
- **The reference clock measurement** (stage 9): the firmware's frame counter, the same for every pipe. No limit.
- **The I2C engine.** One engine, its lines selected by `DC_I2C_DDC_SELECT`. EDID reads are sequential in `display`, and
  each gives the engine back as found (213-DRV-0112's third round reads the socket not taken without touching a lit one).
  No limit beyond that.
- **Watermarks and bandwidth.**
  - `display` copies the firmware's HUBP request, prefetch, QoS and TTU settings for its pipe, and leaves DCHUBBUB's
    watermark sets A to D (`DCHUBBUB_ARB_WATERMARK`, read by batch 10) as the firmware set them for its one 2560x1600
    pipe.
  - Display Core computes the watermarks, the DRAM clock-change and self-refresh margins, and the clocks above, for the
    whole configuration (DML). It also counts the active displays for its clock manager (`rn_get_active_display_cnt_wa`).
  - The load at 4 bytes a pixel, active pixels only, by arithmetic:

    | Pipe | Mode | GB/s |
    |---|---|---|
    | DP-2 | 2560x1600 at 60 Hz | 0.98 |
    | each HDMI | 3440x1440 at 60 Hz | 1.19 |
    | three pipes | | about 3.4 |

    Linux carried two monitors, about 2.2 GB/s, with compressed surfaces (DCC), and its own watermarks and clocks.
  - **Whether the firmware's watermarks and clocks carry three linear pipes is not known.** The standing check would see
    an underflow and undo the pipe, and that is the evidence to gather. Display Core's own numbers for this configuration
    could be had from Linux's `amdgpu_dm_dtn_log` with three monitors: a read the owner session's rules allow (files the
    kernel hands out).

## Memory

- **What a second HDMI surface needs.** The second screen's surface is BAR 0's slot 5, 20 MiB at [0x1000000, 0x2400000)
  (init's `DISPLAY_SECOND_SCREEN`, 213-KRN-0271). A 3440x1440 surface at a pitch of 3456 pixels is 19,906,560 bytes, so
  slot 5 holds one.
- **A third screen** needs another range of the same size, for example BAR 0 [0x2400000, 0x3800000):
  - BAR 0 is 256 MiB of the 512 MiB carve-out, and the VBIOS reserves none of it (`vram_usagebyfirmware` empty);
  - its GPU address would be `DCN_VM_FB_LOCATION_BASE` + 0x2400000, by the survey's reasoning for slot 5 (confidence
    medium, as there).
- **Whose it is:** init's grant, KRN's (below).

## The interface, and the asks to KRN

**What there is today:**
- `idl/display.wit` 1.4 (213-KRN-0272, 213-KRN-0273):
  - `offer-screen(width, height, pitch, format, pixels: borrow<mmio>)` offers one second screen. Offering again replaces
    it.
  - `screen-gone()` takes it away.
  - `mirror-fit(fit)` and `mirror()` act on the one second screen.
- The compositor (`compositor/src/main.rs`):
  - it holds one `Option<Second>` and mirrors the first screen into it;
  - it refuses a client not badged `BADGE_SCREEN`;
  - `display`'s client is in its `SLOT_DISPLAY`.
- init grants `display` slot 5 (`SLOT_MEM`), one 20 MiB window of BAR 0.

**What `display` would need for two driver screens, as asks to KRN:**
1. **`display.wit` 1.5**, a new version with an explicit transition (MC-12.4; before the first release 1.4 is replaced,
   not kept beside it):
   - `offer-screen` and `screen-gone` gain a screen index from the driver, `which: u8` (0 or 1), with pixels lent per
     index;
   - an offer with an index replaces only that index's screen; `screen-gone(which)` takes away only that one;
   - `mirror()` names the index it answers for, or returns both;
   - the errors as today, and `bad-screen` for an index beyond those the compositor holds.
2. **The compositor** holds up to two driver screens at once and mirrors the first screen into each, scaled to fit each
   (the fit chosen once, as in 1.4, or per index if the shell is to choose apart).
   - The cost of the copy doubles while both are offered: two 3440x1440 targets for each change of the first screen.
   - The compositor's budget and its frame times are KRN's to judge.
3. **init:** a second window of BAR 0 for the third screen's pixels, beside `DISPLAY_SECOND_SCREEN`.
   - The proposal is BAR 0 [0x2400000, 0x3800000), 20 MiB, in a slot of `display`'s that KRN names.
   - Or slot 5 grown to [0x1000000, 0x3800000), if `display` may lend a sub-range of its own MMIO capability, which it
     cannot do today as far as `display` knows.
4. **The grant's text in `docs/profile/bootstrap.md`** for `display`: the second window added (KRN's).

Not asked: init's word (`output second light`) can govern both sockets as it is; a word per socket can come later.

## A staged plan, with acceptance criteria

1. **Host only: two runs, each guarded against the other.** In `light.rs` and `start.rs`:
   - the firmware's pipe found once and kept; display's pipes told by their mark;
   - a `Run` per output, with each guard denying the other lit pipe;
   - each watch reading the other lit pipe; the follower and the standing check per socket.
   - Criteria, on the model:
     - [ ] both sockets lit, each on its own pipe;
     - [ ] every stage of the second refused at the first's blocks, which read as before after each stage;
     - [ ] each unplug takes back only its own run, the other's picture and the firmware's unchanged;
     - [ ] HDMI-A-1 alone behaves as before (its cases unchanged).
   - The Ryzen's own tables with crtc 2 and PHY B's parameters, when `MIND_AMD_VBIOS` names the image.
2. **Read on the machine first** (a bench build in the survey's order, HDMI-A-1 lit, nothing written for the second):
   - [ ] step 1 with pipe 1 running, its batches 9 and 10 naming the firmware's pipe;
   - [ ] DPP2's DTO, MPCC2 and mux 2 recorded here.
3. **First light on the third pipe** (`FirstLight` for HDMI-A-2, `UNDO_AFTER_STAGES` on the first boot, the survey's
   order, a person at the machine; PHY B's table 12 takes its long path, never run on this hardware, 213-DRV-0097):
   - [ ] the colour squares on HDMI-A-2 while HDMI-A-1 shows the mirror and DP-2 the desktop;
   - [ ] the firmware's pipe and pipe 1 read as before after each stage;
   - [ ] the maintainer's look (MC-12.1).
4. **The third screen offered** (after KRN's asks):
   - [ ] HDMI-A-2's surface in the second window, offered as screen 1 through `display.wit` 1.5;
   - [ ] the mirror on both HDMI monitors in their own modes;
   - [ ] the standing check clean on three pipes for an hour.
5. **Plugged and unplugged:**
   - [ ] either HDMI monitor taken out and put back, the other's picture undisturbed;
   - [ ] a monitor moved from one socket to the other;
   - [ ] the watch and the standing check clean.

**Before any hardware run, host cases can show:** the choice and the guards with two runs; each watch; the follower's
states; the standing check per pipe; the tables' writes for pipe 2 and PHY B on the image; and, on KRN's side, the
compositor with two screens on its own model. They cannot show the bandwidth, the clocks or the watermarks: only the
machine can (MC-12.2).

## Not known

- Whether the firmware's DISPCLK, DPPCLK (DTO2), DCFCLK and watermarks carry three pipes without underflow.
- DPP2's and pipe 2's request state as found. Only the firmware's pipe is read today.
- What the Ryzen's tables write with crtc 2 on PHY B. Host cases with the image can tell, before any write.
- Whether a cold PHY B's long path (table 12, 530 steps on the model) behaves on the hardware (213-DRV-0097).
- Whether the compositor's frame time stays within its budget with two copies a change (KRN's to measure).
- Whether starts not through Linux leave the same state. Every boot of this machine so far was a start out of Linux.

## Related

[213-DRV-0112](213-DRV-0112-display-a-monitor-on-the-ryzens-other-hdmi-socket.md) (HDMI-A-2 alone, one socket at a time),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror),
[213-DRV-0097](213-DRV-0097-display-table-12s-cold-path-on-the-ryzen.md) (table 12's cold path),
[213-DRV-0099](213-DRV-0099-display-both-screens-by-default.md) (the setting),
`hwdocs/cezanne-display.md` (the survey).
