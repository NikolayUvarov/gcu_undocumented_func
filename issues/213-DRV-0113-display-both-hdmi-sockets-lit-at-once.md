# 213-DRV-0113 — `display`: both HDMI sockets lit at once (three screens)

**Type:** driver (`display`), design · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-27); stages 1 to 4 built on `claude/213-DRV-0113-display-both-hdmi-sockets`; three screens lit on the Ryzen by a bench build of them (release 202611271307); since the sixth round a release lights every HDMI socket with a monitor, shown by host cases and not yet run as a release (see "The release's behaviour") · **Blocked by:** 213-KRN-0285 (the third screen's slots and `display.wit` 1.5, made on `claude/213-KRN-0285-three-screens`, merged into this branch; 0113 lands after it or with it, round 181) · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer wants three screens at once on the Ryzen:
- DP-2, the firmware's;
- HDMI-A-1;
- HDMI-A-2, with a third monitor in it since 2026-11-26.

`display` lights one HDMI socket at a time
([213-DRV-0112](../issues-done/213-DRV-0112-display-a-monitor-on-the-ryzens-other-hdmi-socket.done)). With a monitor in each, the one not
taken is named `NOT DRIVEN ... DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW`.

The limit is `display`'s and its interface's, not the engine's:
- **The pipes.** DCN 2.1 has four timing generators, OPPs, HUBPs and DPPs and five stream encoders (`dcn21_resource.c`
  `res_cap`; `pipe_count` is four less any pipe fused off).
- **The firmware's state.** It leaves all four HUBPs and DPPs powered, HUBP1 to HUBP3 blanked on their own idle OTGs
  (`hwdocs/cezanne-display.md`, "Power").
- **Linux** drove two monitors on all four pipes.
- **What is taken today:** the firmware's picture takes one pipe and `display`'s second screen another, so two are free.

This file began as a design from the code and Linux's Display Core as they are (Linux v7.0). Stages 1 to 3 are now
built and shown on the host model ("Progress"); nothing of them has run on the Ryzen, and none of it is a guarantee
(MC-12.3).

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
- **OTG2's `OTG_VERTICAL_INTERRUPT2_POSITION`,** where the pipe's restart mark would go (batch 2 reads it in the survey's
  order only; step 1 beside display's own OTG now reads all four marks in either order, "Progress").

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
   - Criteria, on the model (host cases, not proofs: MC-12.2):
     - [x] both sockets lit, each on its own pipe (HDMI-A-1 on OTG1 in 16 stages, HDMI-A-2 on OTG2 to first light; and
       the other way round with `HDMI_FIRST HdmiA2`);
     - [x] every stage of the second refused at the first's blocks, which read as before after each stage;
     - [x] each unplug takes back only its own run, the other's picture and the firmware's unchanged;
     - [x] HDMI-A-1 alone behaves as before (its cases unchanged).
   - [x] The Ryzen's own tables with crtc 2 and PHY B's parameters, when `MIND_AMD_VBIOS` names the image (the model's
     counts in "Progress").
2. **Read on the machine first** (a bench build in the survey's order, HDMI-A-1 lit, nothing written for the second;
   setting (a) in "Progress"):
   - [x] step 1 with pipe 1 running, its batches 9 and 10 naming the firmware's pipe (boot of 202611271307, below);
   - [x] DPP2's DTO, MPCC2 and mux 2 recorded here (batches 13 and 14; below).
3. **First light on the third pipe** (setting (b) on the first boot, `SECOND_HDMI FirstLightUndone`; then (c); the
   survey's order, a person at the machine. `UNDO_AFTER_STAGES` stays off: it would undo HDMI-A-1 too, and then no second
   run is made. PHY B's table 12 took its short path on boots 110 and 111; its long path has never run on this
   hardware, 213-DRV-0097):
   - [ ] the colour squares on HDMI-A-2 while HDMI-A-1 shows the mirror and DP-2 the desktop (not looked at apart: the
     run below went through first light to the picture in one boot);
   - [x] the firmware's pipe and pipe 1 read as before after each stage (below: `THE OTHER TWO PIPES UNCHANGED` after
     each of the 16);
   - [x] the maintainer's look (MC-12.1), at the picture of stage 4 (below).
4. **The third screen offered** (213-KRN-0285; settings (d) and (e) in "Stage 4"):
   - [x] HDMI-A-2's surface in the second window (slot 7), offered as the third screen through slot 8, `display.wit`
     1.5 (on the model only);
   - [x] the mirror on both HDMI monitors in their own modes (the Ryzen, release 202611271307, below);
   - [x] the standing check clean on three pipes for an hour (boot 116, 3012 to 6734 s, below).
5. **Plugged and unplugged:**
   - [x] either HDMI monitor taken out and put back, the other's picture undisturbed (on the model, and on the Ryzen
     with the maintainer's look: release 202611271414, below);
   - [x] a monitor moved from one socket to the other (on the model, and on the Ryzen: the two monitors swapped,
     below);
   - [x] the watch clean on the machine at every plug change (release 202611271414, below); the standing check's hour
     is stage 4's open item.

**Before any hardware run, host cases can show:** the choice and the guards with two runs; each watch; the follower's
states; the standing check per pipe; the tables' writes for pipe 2 and PHY B on the image; and, on KRN's side, the
compositor with two screens on its own model. They cannot show the bandwidth, the clocks or the watermarks: only the
machine can (MC-12.2).

## Progress (2026-11-27, branch `claude/213-DRV-0113-display-both-hdmi-sockets`)

Commits on 213-DRV-0112's head (246850205): f6843550e (stage 1, `light.rs`), 531305776 (stage 2, `batches.rs`),
f78830912 (stage 3 and the switch, `start.rs`, `setting.rs`, `main.rs`), then the image case and this section. What
host cases show is on the model only.

**Stage 1, two runs each guarded against the other** (`light.rs`):
- `assess_beside`: the firmware's pipe as found at start, kept; display's lit pipe told by its mark; refused if the
  firmware's OTG stopped or its back end is off (`FirmwareMoved`), if the lit OTG lost its mark (`NotMarked`), another
  OTG holds the mark (`MarkedElsewhere`), a third OTG runs (`OtherRunning`) or another back end is on (`OtherBackEnd`).
  The second output takes the lowest free OTG (2 beside OTG1) and its own front end letter.
- `Guard::beside`: the lit pipe's OTG, ODM, VTG, OPP, DPG, HUBP, DPP, MPCC, mux, DIG front and back end, PHY, PLL resync,
  SYMCLK and pixel rate refused (`Refused::Lit`), checked before the run's own allow list; an overlap refuses the
  choice (`SharesLit`); a guard with no room left refuses the pipe whole (`GuardFull`).
- `Watch::pipe`: the lit pipe's 30 registers (its OTG's 21 of batches 2 and 7, its DIG front and back end, its PHY's
  PLL resync, its HUBP's surface and `DCHUBP_CNTL`) read before and after each stage of the other run; a change fails the
  stage (`LitChanged`) and undoes it. The first run reads the second's pipe the same way once the second's stages stay.
- Table 12's write of PHY A's `RDPCSTX_PHY_FUSE0` (1783C), which it makes for every PHY: left unmade where it holds the
  value, refused otherwise, whether PHY A is off the route or lit.
- `batches::read_into_beside`: step 1 with display's own OTGs counted out of the running, so that batches 9 and 10 name
  the firmware's pipe beside display's (`assess_for` and batch 2 refuse two running OTGs as before).

**Stage 2, the pipe read first** (`batches.rs` batches 13 and 14, `read_pipe`), each time a second run is about to be
made, never written, whole in either order, its values summed up in notes that both orders log:

| Batch | Registers of pipe 2 (bytes in BAR 5) | Source |
|---|---|---|
| 13 | the other OTGs' marks (`OTGn_OTG_VERTICAL_INTERRUPT2_POSITION`, OTG1's at 14298); OTG2's state as `optc1_read_otg_state` reads it (from `OTG2_OTG_H_TOTAL` 143A8); `OTG2_PIXEL_RATE_CNTL` 520, `OTG2_PHYPLL_PIXEL_RATE_CNTL` 52C; `OPP_PIPE2_OPP_PIPE_CONTROL` 13800; `DPG2_DPG_CONTROL` 13720 and `DIMENSIONS` 13728; `DOMAIN4/5_PG_STATUS` (HUBP2, DPP2) D524, D52C; `DPPCLK_DTO_CTRL` 5D8, `DPPCLK2_DTO_PARAM` 56C; `MPCC2`'s top, bottom, OPP and control (from 11D9C); `MPC_OUT2_MUX` 12134 | `dcn_2_1_0_offset.h` (`dcn.rs`), `optc1_read_otg_state`, `dcn20_hubp_pg_control` |
| 14 | HUBP2's state and the registers stages 11 to 15 write there (from F174), and DPP2's (from 1116C), each only where batch 13 finds its domain on | as batch 4 and stage 12 read them |

The batches' counts on the model: 36 registers for batch 13, 62 for batch 14 with both domains on.

**Stage 3, first light on the third pipe** (`start.rs` `second_run`): after the first socket's stages, when SECOND_HDMI
asks and the other socket senses a monitor:
1. step 1 again beside display's own OTG, the four OTGs' marks read first; batch 12 again for HDMI-A-2;
2. batches 13 and 14 for the pipe `assess_beside` gives;
3. the mode from the other socket's EDID where the plan reads it (`STEP 3'S MODE`), else the socket's fallback;
4. `prepare_beside`, then the 8 first-light stages on pipe 2 with the firmware's pipe and the first's read after each;
   or only their description (Read, or where `HDMI_A_2` or `STAGES_ALLOWED` lets display go no further).

The first's guard refuses the second's blocks from then on. What follows (`Follower`):
- the second's unplug takes back its own stages, the first left lit;
- the first's unplug with the second lit takes back the first (screen_gone first), and the second is then the only one,
  followed as the first was;
- a monitor plugged in beside the lit socket makes the second run; after a failure or the build's undo it is not tried
  again this boot;
- the standing check stays the first's: the second stops at first light, before stage 15.

Its writes on the model: only DIG B's, UNIPHYB's, RDPCSTX1/DPCSTX1's, PHYPLLB's resync, SYMCLKB's and pipe 2's OTG,
ODM, VTG, OPP and DPG and OTG2's pixel rate; HDMI-A-1's pipe and the firmware's read as before after every stage.

**The Ryzen's own tables on CRTC 2 and PHY B beside a lit HDMI-A-1** (`the_ryzens_own_tables_on_crtc_2_and_phy_b_beside_a_lit_hdmi_a_1_...`,
with `MIND_AMD_VBIOS`; HDMI-A-1 lit first by its own tables at 3440x1440, then 1366x768 at 85.5 MHz in HDMI on pipe 2;
PHY A's FUSE0 set to the Ryzen's values after HDMI-A-1's stage 2):

| PHY B | Table 12 (PLL1, CRTC 2) | Table 4 | Table 76 | Writes in all | Outside the second's blocks |
|---|---|---|---|---|---|
| cold (SCRATCH 0) | 540 steps, 4 calls, 110 writes, 1 left | 33 steps, 2 calls, 5 writes | 289 steps, 9 calls, 23 writes | 191 | none |
| set up (SCRATCH 0x300) | 318 steps, 4 calls, 32 writes, 1 left | 33 steps, 2 calls, 5 writes | 241 steps, 9 calls, 18 writes | 108 | none |

- The one write left is table 12's of PHY A's FUSE0: with PHY A lit it holds B038003E or B038A034, both with MPLLB_V2I
  already 2, so the write would not change it and is not made (`S2 T12 LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C AS IT IS:
  ... (B038003E)`). A value that the write would change is refused as the lit pipe's (`Refused::Lit`), and stage 2 is
  undone (the synthetic case). On the model HDMI-A-1's own table 12 leaves B028003E there.
- Table 12's writes: PHY B's RDPCSTX1 block (17B00 to 17BA8), `PHYPLL1_PIXCLK_RESYNC_CNTL` 404, SYMCLKB 584,
  `OTG2_PIXEL_RATE_CNTL` 520 and `OTG2_PHYPLL_PIXEL_RATE_CNTL` 52C. Table 4's: DIG B's front end (158A0, 158AC, 15A68).
  Table 76's: DIG B's back end (159BC, 159C0, 15A84), UNIPHYB (174BC, 174C0) and RDPCSTX1.
- The bench's own counts on HDMI-A-2 on CRTC 1 (boots 110 and 111): table 12 318 steps, 32 writes, 1 left; table 4 33
  steps, 5 writes; table 76 289 steps, 23 writes. The model's are a model's.
- The second's unplug puts back every register it wrote but PHY B's control-register bus (by design); then HDMI-A-1's
  puts back everything else.

**The bring-up switch** (`main.rs`): `const SECOND_HDMI: setting::SecondHdmi = setting::SecondHdmi::Off;`, committed off
in this round. (Since the sixth round the release's value is `Full`: "The release's behaviour" below.)

| Value | What the second run does |
|---|---|
| `Off` (the release's until the sixth round) | nothing; the start's lines as before |
| `Read` | its pipe read first (step 1 beside, batches 12 to 14), its stages described, nothing written |
| `FirstLight` | first light's 8 stages on pipe 2, left in place |
| `FirstLightUndone` | the same, undone after them (its mark kept: not tried again this boot) |

How it combines with the others:
- `HDMI_A_2`: on HDMI-A-2 the second run goes no further than this lets display go. At `Read` or `Edid` its stages are
  described only; at `Read` its EDID is not read either (the mode is VIC 16's). At `FirstLight` or `Full`, 8 stages.
- `HDMI_FIRST`: which socket is first; the other gets the second run. HDMI-A-2 is first only where `HDMI_A_2` is
  `FirstLight` or `Full`.
- `SURVEY_ORDER`: both runs' order. The second's notes are logged in either; every register's value and every write
  only in the survey's.
- `UNDO_AFTER_STAGES`: undoes the first run, so no second run is made (`THE FIRST SOCKET'S FIRST LIGHT IS NOT IN
  PLACE`), and nothing is followed. Keep it off; `FirstLightUndone` undoes only the second.
- `STAGES_ALLOWED`: bounds both. The first must reach first light (8) for a second run; the second runs at most 8.
- The setting word: only `light` makes a second run (`read` and `off` plan none).

**Settings for the bench:**
- (a) the read-only run, HDMI-A-1 lit, a monitor in HDMI-A-2: `SECOND_HDMI = Read`, `HDMI_A_2 = Edid` (its EDID read
  over DDC2 as on boot 109, so that its stages are shown in its own mode; `Read` writes nothing of it at all and shows
  VIC 16's), `HDMI_FIRST = HdmiA1`, `SURVEY_ORDER = true` (every register's value), `UNDO_AFTER_STAGES = false`,
  `STAGES_ALLOWED = 16`.
- (b) first light on the third pipe, undone after its stages: `SECOND_HDMI = FirstLightUndone`, `HDMI_A_2 = FirstLight`,
  `HDMI_FIRST = HdmiA1`, `SURVEY_ORDER = true`, `UNDO_AFTER_STAGES = false`, `STAGES_ALLOWED = 16`.
- (c) the same, left in place: as (b) with `SECOND_HDMI = FirstLight`.

**The lines to look for** (each of the second run's is prefixed with its socket's name):
- `A BRING-UP BUILD, NOT A RELEASE: ... SECOND_HDMI; ONLY WHAT LIGHT DOES CHANGES` (the switch named, its word in the
  lines below: the line would pass 180 bytes with every switch set).
- `HDMI-A-2 TAKEN SECOND, AFTER HDMI-A-1'S STAGES: HPD 2 SENSES A MONITOR THERE TOO (SECOND_HDMI <WORD>: <WHAT>)`.
- `HDMI-A-2: SECOND HDMI RUN BESIDE HDMI-A-1 ON OTG1 (SECOND_HDMI <WORD>): <WHAT>`.
- `HDMI-A-2: STEP 1 BESIDE DISPLAY'S OWN OTG0100: COUNTED OUT OF THE RUNNING; MARKS OTG0 .... OTG1 5A5A OTG2 .... OTG3 ....`.
- `HDMI-A-2: B9 ...` (the firmware's pipe named beside display's), `HDMI-A-2: B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ`.
- `HDMI-A-2: B13 PIPE 2: ...` (five notes), `HDMI-A-2: B13 PIPE 2 DONE: 36 OF 36 REGISTERS READ, NOTHING WRITTEN`,
  `HDMI-A-2: B14 PIPE 2 PLANE: HUBP2 ...`, `... DPP2 ...`, `HDMI-A-2: B14 PIPE 2 PLANE DONE: ...`.
- `HDMI-A-2: STEP 3'S MODE: 1366x768 AT 85500 KHZ IN Hdmi SIGNALLING, ...` (expected from boot 109's EDID).
- `HDMI-A-2: S0 STEP 3'S PIPE: OTG2 OPP2 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; MODE ...` and
  `HDMI-A-2: S0 HDMI-A-1'S PIPE, LIT BY DISPLAY: OTG1 OPP1 DIG A FRONT END, PHY A, HUBP1; NEVER WRITTEN, READ AFTER EACH STAGE (30 REGISTERS)`.
- Read: `HDMI-A-2: SECOND HDMI RUN SHOWN FOR OTG2, <N> REGISTERS READ, NOTHING WRITTEN (SECOND_HDMI READ, HDMI_A_2 EDID)`.
- First light: `HDMI-A-2: S<N> <STAGE> DONE; THE OTHER TWO PIPES UNCHANGED; ...` for 1 to 8 (since stage 4's commit; it
  named both pipes before, which took the line past 180 bytes at stage 15),
  `HDMI-A-2: S2 T12 LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C AS IT IS: ...`, then
  `HDMI-A-2: SECOND HDMI RUN ON OTG2 Lit: 8 STAGES IN PLACE, <R> REGISTERS READ, <W> WRITES, <MS> MS; HDMI-A-1 Lit ON OTG1`.
- Undone: `HDMI-A-2: STEP 3 UNDONE AFTER STAGE 8 OF 16 AS THIS BUILD ASKS; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED; HDMI-A-1'S PIPE UNCHANGED`,
  then `HDMI-A-2: SECOND HDMI RUN ON OTG2 Stopped: 0 STAGES IN PLACE, ...`.
- Not made: `HDMI-A-2: SECOND HDMI RUN NOT MADE: <WHY>`, `... NOT MADE (SECOND_HDMI <WORD>): THE FIRST SOCKET'S FIRST LIGHT IS NOT IN PLACE`,
  `SECOND HDMI RUN NOT MADE (SECOND_HDMI <WORD>): NO MONITOR SENSED IN THE OTHER HDMI SOCKET`.
- A failure: `HDMI-A-2: S<N> ... FAILED: ...`, `HDMI-A-2: STEP 3 FAILED AT STAGE <N> AND WAS UNDONE; ...; HDMI-A-1'S PIPE UNCHANGED|CHANGED`.
- What follows: `HDMI-A-2: STEP 3 TAKEN BACK IN <MS> MS: ...; THE FIRMWARE'S PIPE UNCHANGED; HDMI-A-1'S PIPE UNCHANGED`;
  `HDMI-A-2: A MONITOR PLUGGED IN THERE; HDMI-A-1 IS LIT: THE SECOND HDMI RUN NEXT (SECOND_HDMI <WORD>)`;
  `STEP 3 TAKEN BACK IN ... THE FIRMWARE'S PIPE UNCHANGED; HDMI-A-2'S PIPE UNCHANGED` then
  `HDMI-A-2 NOW THE ONLY HDMI SOCKET LIT, ON OTG2: FOLLOWED AS THE FIRST WAS; THE SECOND OUTPUT Lit`;
  `HDMI-A-2 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-1 IS LIT, AND THE SECOND HDMI RUN FAILED OR WAS UNDONE: NOT TRIED AGAIN THIS BOOT`.

**The bench, stage by stage** (HDMI-A-1's monitor and the third monitor in HDMI-A-2, DP-2 as ever):
1. (a): HDMI-A-1 lit as before; HDMI-A-2 stays dark; nothing of its pipe written. Record batches 13 and 14 here (DTO2,
   MPCC2, mux 2, HUBP2 and DPP2 as found) and check that step 1 beside names the firmware's pipe in batches 9 and 10.
2. (b), a person at the machine: the colour squares on HDMI-A-2 from stage 8 for the survey's pause, then dark again;
   HDMI-A-1's picture and DP-2's desktop unchanged throughout. Every stage's line names both pipes unchanged.
3. (c): the squares stay. Then HDMI-A-2's monitor out (its stages taken back, HDMI-A-1 untouched) and back in (the
   second run again); then HDMI-A-1's out (HDMI-A-2 the only one, still showing the squares) and back in (HDMI-A-1
   gets the second run, first light on OTG1, with no picture: the picture is stage 9 onwards, not run for a second).

**Not built in stages 1 to 3:** stage 4 and the second's standing check (both built since: "Stage 4"), and the scaler
(assessed below).

## Stage 4 (2026-11-27, fifth round): the second HDMI run lit whole, offered as the third screen

On this branch after merging `claude/213-KRN-0285-three-screens` (9e8fe6b58): merge bdf65c18e, then c19b6a10f. What the
host cases show is on the model only; nothing of it has run on the Ryzen (its boot drive refused a write twice,
`issues-human` §19).

**What 213-KRN-0285 gives `display`** (`mind::display`): slot 7 (`SLOT_THIRD_PIXELS`), BAR 0 [0x2400000, 0x3800000),
20 MiB, write-combining as slot 5; slot 8 (`SLOT_THIRD_SCREEN`), a second client of the compositor badged
`BADGE_THIRD_SCREEN` (4), granted only with slot 7. `offer-screen` and `screen-gone` through slot 8 are the third
screen's; through SLOT_DISPLAY (15) the second's, as before.

**What changed in `display`:**
- **Each run holds a screen** (`light::Screen`): the second (slot 5's pixels, offered through slot 15) or the third (slot
  7's, through slot 8). The start's first run holds the second; the second run takes the one the first does not hold,
  and keeps it while it lives. Its HUBP scans its own window (FB + 0x1000000 or + 0x2400000), stage 10 draws there in its
  own mode (1366x768 at a pitch of 1408 pixels, 5632 bytes), stage 16 offers it through its own client, and an undo's
  screen_gone goes through that client first.
- **Stages 9 to 16 for the second run** as for the first: stage 9's reference is the firmware's pipe (step 1 beside
  display's own OTG names it), its DLG and TTU derived for its own mode; its DPP2, HUBP2 and MPCC2 into OPP2 (mux 2);
  stage 15's unblank, its check and its watch read the firmware's pipe and the other display pipe too, and a new
  underflow or request timeout on either fails the stage at once.
- **No table runs in stages 9 to 16:** on the Ryzen's image, the second run's sixteen stages call tables 12, 4 and 76
  in stages 2, 6 and 8 only, as the first's do; every write of the second is in its own blocks, refused to the first.
- **A missing slot 7 or 8:** said, and the second run stops after first light with its pattern, as in round 4. A third
  screen the compositor refuses: stage 16 not ready, stages 1 to 15 stay, the picture up and not offered.
- **The standing checks:** each picture's every second, the first's then the second's; each reads the firmware's pipe,
  its own and the other display pipe. An underflow or request timeout undoes the run on whose pipe it is (on the
  firmware's: the run whose check found it), named in its own line; the other run stops reading that pipe and goes on.
  During a run's stage 15 watch, a new underflow on the other display pipe fails that run at once (as the firmware's
  does); the status stays latched, so the other run's standing check then undoes its own run too: both end undone.
- **The follower:** either unplug takes back only its own run, its screen let go through its own client first. A plug
  back in relights it beside the other: the lowest free OTG and the screen left free, so the same pipe and client as
  before. The first's unplug while the second is lit leaves the second on its pipe (2) and its screen (the third, slot
  8): nothing of it moves. After each change `HDMI SOCKETS NOW: ...` says both sockets.
- **The switch:** `SECOND_HDMI` gains `Full` (all sixteen stages and the offer), committed at `Off` in this round (the
  release's value since the sixth round); `FirstLight` and `FirstLightUndone` keep their meaning. Slots 7 and 8 are said,
  and slot 7 mapped, only in a build with it on.

**Settings for the bench:**
- (d) three screens at once, the mirror on both HDMI monitors: `SECOND_HDMI = Full`, `HDMI_A_2 = Full`, `HDMI_FIRST =
  HdmiA1`, `SURVEY_ORDER = true` (every value and write logged, for the first boot), `UNDO_AFTER_STAGES = false`,
  `STAGES_ALLOWED = 16`; init's `output second light` as before, and an init carrying 213-KRN-0285 (slots 7 and 8).
- (e) the same in the production order: as (d) with `SURVEY_ORDER = false`.

**The lines to look for** (the second run's prefixed with its socket's name):
- At start: `BAR 0 FOR A THIRD SCREEN (SLOT 7): 20480 KIB GRANTED` and `A SECOND CLIENT OF THE COMPOSITOR (SLOT 8, THE
  THIRD SCREEN'S): GRANTED` (said only with SECOND_HDMI on).
- `HDMI-A-2 TAKEN SECOND, AFTER HDMI-A-1'S STAGES: ... (SECOND_HDMI FULL: ALL 16 STAGES, ITS PICTURE OFFERED AS ITS OWN SCREEN)`.
- `HDMI-A-2: S9 REFERENCE DONE; THE OTHER TWO PIPES UNCHANGED; ...`, `HDMI-A-2: S10 DRAWN: 1366x768 AT SLOT 7'S START, PITCH 1408 PIXELS (5632 BYTES); ...`.
- `HDMI-A-2: S15 NO NEW UNDERFLOW: HUBP0 AND ODM0 (THE FIRMWARE'S), HUBP2 AND ODM2, HUBP1 AND ODM1 (HDMI-A-1'S)`,
  `HDMI-A-2: S15 NO UNDERFLOW OR REQUEST TIMEOUT ON ANY OF THE THREE PIPES IN 60 FRAMES`.
- `HDMI-A-2: S16 OFFERED THE THIRD SCREEN: 1366x768, PITCH 5632 BYTES, BGR, ITS PIXELS SLOT 7'S SURFACE; ...`, then the
  compositor's own `THIRD SCREEN ...` line with its three times (`THE ONE BEFORE LET GO IN … US`, `MAPPED IN … US`,
  `DRAWN IN … US`).
- `HDMI-A-2: SECOND HDMI RUN ON OTG2 Lit: 16 STAGES IN PLACE, ...; HDMI-A-1 Lit ON OTG1; OFFERED AS THE THIRD SCREEN THROUGH SLOT 8`.
- `HDMI SOCKETS NOW: HDMI-A-1 LIT ON OTG1 (16 STAGES, SECOND SCREEN); HDMI-A-2 LIT ON OTG2 (16 STAGES, THIRD SCREEN)`
  after each plug change; `... NO MONITOR`, `... A MONITOR, NOT LIT`, `... STOPPED AFTER A FAILURE` for the others.
- An unplug of HDMI-A-2: `HDMI-A-2: U16 SCREEN_GONE THROUGH SLOT 8: THE COMPOSITOR HAS LET THE THIRD SCREEN'S SURFACE GO`,
  then `HDMI-A-2: STEP 3 TAKEN BACK IN ... HDMI-A-1'S PIPE UNCHANGED`.
- Every ten minutes: `S15 STANDING CHECK: ... NOTHING NEW ON ANY OF THE THREE PIPES` and the same prefixed `HDMI-A-2: `.
- Not granted: `HDMI-A-2: THE THIRD SCREEN'S SLOTS: SLOT 7 (ITS PIXELS) ..., SLOT 8 (ITS CLIENT) ...: THE SECOND HDMI RUN STOPS AFTER FIRST LIGHT, ITS PATTERN LEFT`.
- Refused: `HDMI-A-2: S16 OFFER NOT READY: THE COMPOSITOR DID NOT TAKE THE THIRD SCREEN: <WHY>; ...`.
- An underflow: `HDMI-A-2: S15 UNBLANK FAILED: AN UNDERFLOW AT <OFFSET> (...)` during its watch, or `S15 STANDING CHECK:
  AN UNDERFLOW AT <OFFSET> ...` and `THE STANDING CHECK: AN UNDERFLOW AT <OFFSET> (...) ON PIPE <N>'S HUBP|ODM (<WHOSE>): <SOCKET>'S RUN ON OTG<N> UNDONE`.
  The offsets: DCHUBP_CNTL 0EACC (HUBP0, the firmware's), 0EE3C (HUBP1), 0F1AC (HUBP2); OPTC_INPUT_GLOBAL_CONTROL
  13E28, 13E68, 13EA8 (ODM0 to ODM2).

**The bench, for (d) then (e)** (the 3440x1440 monitor in HDMI-A-1, the 1366x768 one in HDMI-A-2, DP-2 as ever):
1. (d), a person at the machine: DP-2's desktop throughout; HDMI-A-1's colour squares then the mirror, as on boots 95 to
   112; then HDMI-A-2's squares from its stage 8, its own picture (MIND CORE / DISPLAY / HDMI-A-2 1366X768) at stage 15,
   the mirror once the compositor takes the third screen. HDMI-A-1's picture must not move while HDMI-A-2's stages run.
2. Ten minutes and more: both standing lines every ten minutes, nothing undone; the compositor's three times at each
   offer.
3. Plug changes, one at a time, a few seconds apart: HDMI-A-2 out and in; HDMI-A-1 out (HDMI-A-2 must stay lit on
   OTG2, the third screen) and in; then the 1366x768 monitor moved to HDMI-A-1 with HDMI-A-2 empty. Each ends with an
   `HDMI SOCKETS NOW:` line that matches the monitors.
4. (e): the same start in the production order, the times of `THE SECOND SCREEN OFFERED ...` and of the second run's
   line for comparison with (d).

**Host cases** (`tests/display_light_host.rs`, `tests/display_screen_host.rs`), on the model:
- two offers through two clients, 3440x1440 through slot 15 and 1366x768 through slot 8, each from its own slot, the
  first's writes exactly as alone, every one of the second's in its own blocks;
- slot 7 or 8 (or both) not granted, and a third screen refused;
- each unplug order, a plug back on the same pipe and client, both out with every register as before;
- a monitor moved from one socket to the other, the state said after each change;
- an underflow on pipe 1 during the second's watch, on pipe 2 during the first's (relit) watch, and on either in the
  standing checks, each named with its pipe;
- the Ryzen's tables for the second's sixteen stages beside HDMI-A-1's sixteen, cold and set up (`MIND_AMD_VBIOS`);
- `screen.rs`'s third client through the compositor's badge rule (`screen_of`), slot 8 missing, a badge naming no
  screen.

**The bandwidth, by arithmetic (text only; only the machine can answer it, MC-12.2):**

| Pipe | Mode | Active fetch, average | At the line rate (H and V blank left out) | One DET (164 KiB) lasts |
|---|---|---|---|---|
| DP-2 (OTG0) | 2560x1600 at 60 Hz | 0.98 GB/s | 1.08 GB/s | 156 us |
| HDMI-A-1 (OTG1) | 3440x1440 at 60 Hz | 1.19 GB/s | 1.30 GB/s | 129 us |
| HDMI-A-2 (OTG2) | 1366x768 at 59.79 Hz | 0.25 GB/s | 0.34 GB/s | 491 us |
| All three | | 2.42 GB/s | 2.71 GB/s | |

- **Against Display Core's own bounds for DCN 2.1** (`dcn2_1_soc`, `dcn2_1_ip` in `dcn20_fpu.c`, Linux v7.0; defaults
  that Linux replaces with the SMU's clock table at run time): at the lowest state (DCFCLK 400 MHz, fabric 400 MHz, DRAM
  1600 MT/s) the return bus carries 64 B a DCFCLK (25.6 GB/s), the fabric 32 B a cycle (12.8 GB/s, 60% of it the normal
  average, 7.7 GB/s), the DRAM 4 channels of 4 B (25.6 GB/s). The latencies a pipe's DET must cover: urgent 4 us, self
  refresh exit 12.5 us (enter and exit 17 us), the DRAM clock change 23.84 us: the 3440x1440 pipe needs some 30 KiB for
  the longest of them, its DET holds 164 KiB. By this arithmetic three pipes fit with room.
- **What is known to carry:** two pipes, DP-2's and HDMI-A-1's 3440x1440 linear surface (about 2.2 GB/s), the standing
  check clean on boots 95 to 100 and 112. The third adds 0.25 GB/s on average.
- **What is not known:** the DCFCLK and fabric the SMU runs at (`display` sets neither), and whether the watermark sets
  A to D the firmware set for its one pipe (batch 10's `DCHUBBUB_ARB_WATERMARK` lines, in the boot logs) hold when three
  pipes ask at once: the urgent and the DRAM clock-change watermarks, and when the SMU lets the DRAM change its clock.
  The arithmetic above is Display Core's model, not a measure.
- **What the log shows first if they do not carry:** an underflow on the HUBP that ran dry, `HUBP_UNDERFLOW_STATUS`
  (DCHUBP_CNTL bits 30:28) at 0EACC, 0EE3C or 0F1AC, or a request timeout (`HUBP_TIMEOUT_STATUS`, bits 23:20); then its
  ODM's `OPTC_UNDERFLOW_OCCURRED_STATUS` (OPTC_INPUT_GLOBAL_CONTROL bit 10) at 13E28, 13E68 or 13EA8. During the second
  run's stage 15 it is `HDMI-A-2: S15 UNBLANK FAILED: AN UNDERFLOW AT ...` (its frames line marks `U`, `T` or `O`);
  later it is the standing check's line and the `THE STANDING CHECK: ... ON PIPE <N>'S ...` line, the pipe undone. An
  underflow on HUBP0 is DP-2's: the firmware's picture, which display does not drive, glitches first on the monitor.

## The release's behaviour (2026-11-27, sixth round)

The maintainer's requirement after release 202611271307: all three screens work, both HDMI sockets and USB-C, not as a
bench setting. Commit cb6a255b4 makes it the release's behaviour; nothing of it has run on the Ryzen as a release yet.

**What a release does** (`setting::Bringup::RELEASE`, `main.rs`'s consts), in the production order:
- `HDMI_A_2 = Full`: HDMI-A-2 driven in full when it is the socket taken (its monitor alone).
- `SECOND_HDMI = Full`: with a monitor in each socket, HDMI-A-1 first (`HDMI_FIRST = HdmiA1`, unchanged), then HDMI-A-2
  in full beside it on OTG2, offered as the third screen through slots 7 and 8.
- A monitor plugged into the other socket later gets the same second run; each unplug takes back only its own; the first's
  unplug leaves the second on its pipe and screen (stage 4's follower).
- `is_release()` is true at these values only; a build that sets any switch away from them says `A BRING-UP BUILD, NOT A
  RELEASE: …` first, naming it (`HDMI_A_2 <LEVEL>` for Read, Edid or FirstLight; `SECOND_HDMI` for Off, Read, FirstLight
  or FirstLightUndone). The restricted levels stay for bring-up and mean what they meant.
- **The third screen's slots missing** (an init without 213-KRN-0285, or a grant that failed): the second run is not
  made at all; nothing of HDMI-A-2's pipe, DIG or PHY is written (its blocks read by step 1 and its EDID read as at any
  start), and the log says
  `HDMI-A-2: SECOND HDMI RUN NOT MADE: THE THIRD SCREEN'S SLOTS: SLOT 7 (ITS PIXELS) …, SLOT 8 (ITS CLIENT) …; HDMI-A-2 STAYS DARK`.
  Chosen over first light's pattern: a pattern no screen follows gives the person at the machine nothing, can look like a
  fault, and costs PHY B's and pipe 2's writes for no use. A bring-up level of first light still shows the pattern
  without the slots.

**The lines a release logs with both monitors in** (the production order: each stage's note, its detail held unless a
part fails; display's own lines start `[DISPLAY] `):
- `THE SECOND OUTPUT: LIGHT, THE WORD AFTER DEVICE <N>: STEPS 1 TO 4 ON EACH HDMI SOCKET WITH A MONITOR IN ITS EDID'S MODE, THEN BOTH HPDS FOLLOWED AND THE STANDING CHECKS`
  (no `A BRING-UP BUILD` line before it).
- `BAR 0 FOR A THIRD SCREEN (SLOT 7): 20480 KIB GRANTED`, `A SECOND CLIENT OF THE COMPOSITOR (SLOT 8, THE THIRD SCREEN'S): GRANTED`.
- `STEP 1 MAY READ 11 OF 11 BATCHES, WRITES NOTHING; THE PRODUCTION ORDER`, then
  `HDMI-A-2 TAKEN SECOND, AFTER HDMI-A-1'S STAGES: HPD 2 SENSES A MONITOR THERE TOO (SECOND_HDMI FULL: ALL 16 STAGES, ITS PICTURE OFFERED AS ITS OWN SCREEN)`.
- `HDMI-A-2 TAKEN SECOND: ITS EDID READ OVER DDC2 FIRST, THE I2C ENGINE GIVEN BACK AS FOUND; ITS STAGES AFTER THE FIRST SOCKET'S`
  and its `HDMI-A-2: D …` notes; `STEP 3'S MODE: 3440x1440 AT 324000 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING`.
- HDMI-A-1's run as before: `S1` to `S16 … DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS); …`, `STEP 3'S TIMES (MS): …`,
  `STEP 3 STOPPED AFTER STAGE 16 OF 16 AS THIS BUILD ALLOWS; …`, `THE SECOND SCREEN OFFERED <T> MS AFTER DISPLAY'S START, IN THE PRODUCTION ORDER: …`,
  and the compositor's `SECOND SCREEN 3440X1440 …` line.
- `HDMI-A-2: SECOND HDMI RUN BESIDE HDMI-A-1 ON OTG1 (SECOND_HDMI FULL): ALL 16 STAGES, ITS PICTURE OFFERED AS ITS OWN SCREEN`.
- `HDMI-A-2: STEP 1 BESIDE DISPLAY'S OWN OTG0100: COUNTED OUT OF THE RUNNING; MARKS …`, `HDMI-A-2: STEP 1 DONE: 11 OF 11 BATCHES READ, …`,
  the `HDMI-A-2: B12 …` notes, five `HDMI-A-2: B13 PIPE 2: …` notes, `HDMI-A-2: B13 PIPE 2 DONE: 36 OF 36 REGISTERS READ, NOTHING WRITTEN`,
  `HDMI-A-2: B14 PIPE 2 PLANE: …` (two), `HDMI-A-2: B14 PIPE 2 PLANE DONE: 62 OF 62 REGISTERS READ, NOTHING WRITTEN`.
- `HDMI-A-2: STEP 3'S MODE: 1366x768 AT 85500 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING`.
- `HDMI-A-2: S0 STEP 3'S PIPE: OTG2 OPP2 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; MODE 1366x768 AT 85.5 MHZ IN HDMI SIGNALLING (CTA VIC 0)`,
  `HDMI-A-2: S0 HDMI-A-1'S PIPE, LIT BY DISPLAY: OTG1 OPP1 DIG A FRONT END, PHY A, HUBP1; NEVER WRITTEN, READ AFTER EACH STAGE (30 REGISTERS)`.
- `HDMI-A-2: S<N> <STAGE> DONE; THE OTHER TWO PIPES UNCHANGED; …` for 1 to 16: S2 names table 12's path (`THE PHY SET UP
  ALREADY: THE SHORT PATH`, or `THE LONG PATH: <N> WRITES THROUGH THE PHY'S BUS`), S15 ends `WATCHED 60 FRAMES IN … MS CLEAN, …`,
  S16 ends `OFFERED 1366x768 PITCH 5632`.
- `HDMI-A-2: STEP 3'S TIMES (MS): IN ALL <T>; …` and `HDMI-A-2: STEP 3 STOPPED AFTER STAGE 16 OF 16 AS THIS BUILD ALLOWS; <W> WRITES MADE ARE LEFT IN PLACE`.
- The compositor's `THIRD SCREEN 1366X768, PITCH 5632, Bgr, …; THE ONE BEFORE LET GO IN … US, MAPPED IN … US, DRAWN IN … US`.
- `HDMI-A-2: SECOND HDMI RUN ON OTG2 Lit: 16 STAGES IN PLACE, <R> REGISTERS READ, <W> WRITES, <MS> MS; HDMI-A-1 Lit ON OTG1; OFFERED AS THE THIRD SCREEN THROUGH SLOT 8`.
- Then `HPD 1 FOLLOWED EVERY 50 MS, …`, `HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 FULL): SENSE 1; THE SECOND OUTPUT Lit ON HDMI-A-1`,
  `THE STANDING CHECK: …`; every ten minutes `S15 STANDING CHECK: … NOTHING NEW ON ANY OF THE THREE PIPES` and the same
  prefixed `HDMI-A-2: `; after each plug change `HDMI SOCKETS NOW: …`.
- One monitor only: `SECOND HDMI RUN NOT MADE (SECOND_HDMI FULL): NO MONITOR SENSED IN THE OTHER HDMI SOCKET`; HDMI-A-2's
  alone: `HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 FULL: ITS EDID AND ALL 16 STAGES`, lit on OTG1 and
  offered as the second screen.

**How long the second run's light-up takes in the production order:** on the model 1.6 s from its first line to its
last, about the first run's own: stage 9's count of 30 frames of the firmware's OTG (0.5 s) and stage 15's watch of 60
frames (1.0 s) are most of it, then stage 5's and 13's waits for frames (some 60 ms); step 1 beside and batches 12 to 14
take milliseconds. The 140 s of release 202611271307 were the survey's order (1 s before and 3 s after each stage, every
line paced). On the Ryzen the first run's own `STEP 3'S TIMES` line is the reference to compare with; the second's
`HDMI-A-2: STEP 3'S TIMES` and the `… MS;` of its result line give it.

**The bench, for a release built from this branch** (the 3440x1440 monitor in HDMI-A-1, the 1366x768 one in HDMI-A-2):
1. Boot with both in: the lines above, in that order; all three screens with the mirror; the second run's result line
   some seconds after the first's offer.
2. Ten minutes: both standing lines, nothing undone.
3. Plug changes, one at a time: HDMI-A-2 out and in; HDMI-A-1 out (HDMI-A-2 stays, the third screen) and in; the 1366x768
   monitor moved to HDMI-A-1 with HDMI-A-2 empty; each ends with an `HDMI SOCKETS NOW:` line that matches the monitors.
4. A boot with HDMI-A-2's monitor alone, and one with HDMI-A-1's alone.

**Table 12's long path in a release** (213-DRV-0097). With HDMI-A-2 lit by default, a monitor in it after a start not lit
through Linux meets PHY B cold (its `RDPCSTX1_RDPCSTX_SCRATCH` not 0x300), and table 12 takes its long path, which has
never run on this hardware. No way around it is added.
- **On the model with the Ryzen's image** (`the_release_on_the_ryzens_tables_…`, both PHYs cold, their statuses answered
  as the tables wait for them): the release lights both sockets whole; HDMI-A-2's table 12 on PLL1 for CRTC 2 runs 540
  steps, 110 writes (one left: PHY A's FUSE0), 37 through PHY B's control-register bus (17 into the PHY), and the stage
  says `THE LONG PATH: 37 WRITES THROUGH THE PHY'S BUS`. The model's answers are what the tables wait for; whether PHY B
  gives them is what the bench will show.
- **If a table run fails** (on the model: PHY B's statuses never coming): table 12 waits inside table 77 and its run stops
  at its limit, 500 ms (`HDMI-A-2: S2 PIXEL CLOCK FAILED: THE TABLE STOPPED: TimeLimit { table: 77, … }; UNDOING STAGES 2 TO 1`).
  Its off call (table 12 with pixel clock 0: 41 steps, no write on the model) runs to its end, every write of stages 2
  and 1 is put back but PHY B's control-register bus (what its writes set inside the PHY stays), and the run ends
  `HDMI-A-2: STEP 3 FAILED AT STAGE 2 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED; HDMI-A-1'S PIPE UNCHANGED`,
  then `HDMI-A-2: SECOND HDMI RUN ON OTG2 Stopped: 0 STAGES IN PLACE, …`. HDMI-A-1's picture and DP-2's are not
  touched. OTG2 keeps its mark: HDMI-A-2 is not tried again this boot (a replug says `… AND THE SECOND HDMI RUN FAILED OR
  WAS UNDONE: NOT TRIED AGAIN THIS BOOT`, and `HDMI SOCKETS NOW:` says `HDMI-A-2 STOPPED AFTER A FAILURE`).
- **If HDMI-A-2 is the only socket lit** and its first run fails so, it is undone the same way and, as for any first run
  that fails (213-DRV-0096), nothing more is tried this boot, HDMI-A-1's socket included.
- A write of the long path off HDMI-A-2's route is refused by the guard and the stage undone, as on any route.

**Host cases** (on the model; not proofs, MC-12.2):
- [x] the release in the production order with both sockets lit, the third screen offered, its lines as above;
- [x] one monitor in either socket, lit as the first run, the other socket said to have none;
- [x] a monitor plugged into the other socket after the start, lit beside, HDMI-A-1 untouched;
- [x] an unplug of each, a plug back, the moved monitor and the underflows (stage 4's cases, whose plan is the release's);
- [x] the third screen's slots missing: HDMI-A-2 dark, said, nothing of its pipe written;
- [x] on the Ryzen's tables: both PHYs cold, both lit; PHY B's statuses never coming, stage 2 undone, HDMI-A-1 untouched.
- On the machine, not yet: a release (not a bench build) lighting three screens; the plug tests; an hour of both standing
  checks; a start not through Linux (PHY B cold).

## The scaler, assessed (not built)

The question: pipe 2 scanning the same surface as pipe 1 (slot 5, 3440x1440), through DPP2's scaler, down to the third
monitor's 1366x768 at 59.790 Hz: the mirror on all three without the compositor's second driver screen or another
window of BAR 0.

**Within DCN 2.1's limits, as Display Core states them:**
- Ratios 3440/1366 = 2.5183 across and 1440/768 = 1.875 down (`calculate_scaling_ratios`, truncated to 19 bits).
- The plane's limit: `max_downscale_factor` 250 for ARGB8888 (`dcn21_resource.c` `plane_cap`), which `amdgpu_dm_plane.c`
  reads as dst/src at least 250/1000: 4:1. (`dc->caps.max_downscale_ratio` 200 is set, and used by none of the files
  read.) DML's:
  `max_hscl_ratio` and `max_vscl_ratio` 4, `max_hscl_taps` and `max_vscl_taps` 8 (`dcn2_1_ip`, `dcn20_fpu.c`).
- The source width: `max_downscale_src_width` 4096 (`debug_defaults_drv`).
- DPPCLK: DML's rule (`display_mode_vba_21.c`) gives pclk x max(vtaps/6 x min(1, hratio), hratio x vratio / PSCL, 1).
  PSCL = min(4, 2 x hratio / ceil(htaps/6)) = 4 with 4 taps, so DPPCLK >= 85.5 x 1.18 = 101 MHz. Pipe 1 at 324 MHz
  1:1 needs more already. With 8 taps the rule asks 2 x pclk, 171 MHz.

**What Display Core programs for it** (`dpp1_dscl_set_scaler_manual_scale`, `dcn10_dpp_dscl.c`, used by DCN 2.x; DPP2's
DSCL registers from 1116C, `DSCL2_SCL_MODE` at 11208):

| Register | Value for 3440x1440 to 1366x768, RGB |
|---|---|
| `DSCL_AUTOCAL` | 0 (manual) |
| `DSCL_CONTROL` | `SCL_BOUNDARY_MODE` 0 |
| `RECOUT_START`, `RECOUT_SIZE` | 0,0; 1366x768 |
| `MPC_SIZE` | 1366x768 |
| `SCL_MODE` | `DSCL_MODE` 1 (`SCALING_444_RGB_ENABLE`), then `SCL_COEF_RAM_SELECT` flipped after the coefficients |
| `LB_DATA_FORMAT` | `INTERLEAVE_EN` 0, `ALPHA_EN` per-pixel alpha (DCN 2 processes float data) |
| `LB_MEMORY_CTRL` | `MEMORY_CONFIG` 0 (`use_max_lb`), `LB_MAX_PARTITIONS` 63 |
| `SCL_BLACK_OFFSET` | RGB's black for both fields |
| `SCL_HORZ_FILTER_SCALE_RATIO` (and `_C`) | u3.19 of 2.5183 << 5 = 0x0284AF60 |
| `SCL_VERT_FILTER_SCALE_RATIO` (and `_C`) | 1.875 << 24 = 0x01E00000 |
| `SCL_HORZ_FILTER_INIT` | (ratio + taps + 1) / 2 = 3.7591: INT 3, FRAC 0xC257A0 |
| `SCL_VERT_FILTER_INIT` (and `_BOT`) | 3.4375: INT 3, FRAC 0x700000 (`_BOT` adds the ratio) |
| `SCL_TAP_CONTROL` | 4 taps each way (Display Core's default for RGB: fields 3 and 3; chroma 1 and 1) |
| `DSCL_2TAP_CONTROL` | hard-coded 2-tap filters off (more than 2 taps) |
| `SCL_COEF_RAM_TAP_SELECT` and `_DATA` | 64-phase filters, 33 phases x 2 tap pairs = 66 writes each for luma across and down; S1.12 coefficients, 14 bits (0x1000 is 1.0) |

- **The viewport** (HUBP2's `DCSURF_PRI_VIEWPORT_*`): 3440x1440 at 0,0. `calculate_init_and_vp` gives 3441 and 1441,
  each cut to the surface.
- **The line buffer:** one line takes (1366 + 5) / 6 = 228 entries, so configuration 0's 2744 hold 12 lines
  (`dscl2_calc_lb_num_partitions`). 4 vertical taps at a ratio below 2 need 4: it fits.
- **The coefficients:** Display Core picks its 4-tap table for ratios of 5/3 and more (`get_filter_4tap_64p`). The
  tables are AMD's data; `display` would compute its own (a windowed sinc, each phase summing to 1.0), which a host case
  can check.

**What is beyond what `display` does today:**
- **The DLG and TTU of a scaled plane.** With vratio above 1, DML's line and request delivery times use
  `delivery_width / hscale_pixel_rate` rather than `recout_width / pclk` (`get_refcyc_per_delivery`,
  `display_rq_dlg_calc_21.c`). `hscale_pixel_rate` is 4 x DPPCLK here (`min_hratio_fact` 4 for hratio 2.52 and 6 taps
  or fewer), and the firmware's DPPCLK frequency is not read. The prefetch's `vratio_prefetch`, and the scaler's delay
  (`dppclk_delay_scl` 50 cycles, not `_lb_only` 16) in VSTARTUP and the after-scaler offsets, change too. Stage 9's rule
  (pipe n's DLG and TTU from the firmware's 1:1 pipe) does not give these; DML's formulas would have to be built anew,
  and checked against what Linux's `amdgpu_dm_dtn_log` shows for the same two pipes.
- **The bandwidth.** Pipe 2 fetches the whole 3440x1440 surface at 59.79 Hz: 1.18 GB/s more, as much as a full-size third
  screen (a 1366x768 surface of its own would be 0.26 GB/s). The firmware's watermarks are the same risk as for stage 4.
- **The picture.** 4 taps across at 2.52:1 cover fewer source pixels than a low-pass filter for that ratio wants (about
  twice the ratio, 5 or 6): thin lines and text alias. 8 taps fit the line buffer (12 lines) and DPPCLK; Display Core
  uses 4 unless asked.

**What a host case could show:** the register values above from the two modes; the coefficients' sums per phase; the
line buffer's count; DPP2's and HUBP2's writes refused nowhere else; the DLG formulas against hand-worked numbers. Not
whether it underflows, nor how it looks.

**Recommendation:** not as the interim now. The DSCL part is small and well defined, but the scaled plane's DLG and TTU
are new derivations that `display` cannot check against the hardware's own values, and it costs the bandwidth of a full
third screen while still waiting on the same watermark question. Stage 4 (each output its own surface in its own mode,
1:1, with stage 9's rule as it stands) needs KRN's asks but no new derivation. If KRN's asks wait long, the scaler is the
way to a picture on HDMI-A-2 without them: then 2.52 by 1.875 at 4 taps (or 8), DLG built from DML's formulas, a bench
build that undoes after the watch, and Linux's dtn log for the same two modes as the reference.

## Three screens at once on the Ryzen (2026-11-27, release 202611271307)

The bench image `claude/DRV-ryzen-bench-48-three-screens-retries` 0256effd5: `main` f32ba90c93 (which carries
213-KRN-0285's slots and 700-KRN-0284), this branch, 000-DRV-0115, and one bench-only commit of switches, setting (d):
`SECOND_HDMI Full`, `HDMI_A_2 Full`, `HDMI_FIRST HdmiA1`, the survey's order. Through QEMU first: "x86: keys, shell,
tools" and "x86: services, storage, audio" on LEG9, and the update from the release before on x32-128. On acc-1 the same
two groups failed early once, at a step that is not display's (`svc stop tts`); see the bench log. The monitors: DP-2
2560x1600 (the firmware's), HDMI-A-1 3440x1440, HDMI-A-2 1366x768. It holds for that machine, those monitors and that
release (MC-12.1).

**The first HDMI run, as before:** HDMI-A-1 lit in 16 stages; `[COMPOSITOR] SECOND SCREEN 3440X1440, PITCH 13824 …
THE FIRST MIRRORED AT 2304X1440 FROM 568,0; THE ONE BEFORE LET GO IN 0 US, MAPPED IN 42 US, DRAWN IN 8568 US`.

**The second HDMI run, beside it** (142.3 s to 283.0 s of uptime; slow because of the survey's order):
- `BAR 0 FOR A THIRD SCREEN (SLOT 7): 20480 KIB GRANTED`, `A SECOND CLIENT OF THE COMPOSITOR (SLOT 8, THE THIRD SCREEN'S): GRANTED`.
- `HDMI-A-2 TAKEN SECOND, AFTER HDMI-A-1'S STAGES: HPD 2 SENSES A MONITOR THERE TOO (SECOND_HDMI FULL …)`.
- `HDMI-A-2: STEP 1 BESIDE DISPLAY'S OWN OTG0100: COUNTED OUT OF THE RUNNING; MARKS OTG0 0000 OTG1 5A5A OTG2 0000 OTG3 0000`,
  then `S0 THE FIRMWARE'S PIPE (AS STEP 1 FOUND): OTG0 OPP0 DIG E FRONT END, PHY E` and `S0 STEP 3'S PIPE: OTG2 OPP2 DIG B
  FRONT END, PHY B (HDMI-A-2), HPD 2`.
- **Pipe 2 as the firmware left it** (batches 13 and 14, 36 and 62 registers, nothing written): OTG2 not running, its
  clock off; OPP2's clock off, DPG2 off; HUBP2's and DPP2's power domains on; `DPPCLK DTO CTRL 00000000, DTO2 0/0`;
  `MPCC2 TOP 2 BOT 2 OPP 2 MODE 1 ALPHA 255 (CONTROL FFFF0461); MPC_OUT MUX 2: 2`; `HUBP2 DCHUBP_CNTL 000F002B (BLANK 1,
  VTG_SEL 2, TTU_DISABLE 0)`; `DPP2 CONTROL F0000000 (CLOCK 0)`.
- **Stage 2:** `S2 T12 LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C AS IT IS … (B028003E)`, the value HDMI-A-1's own stage 2
  had left; `S2 TABLE 12 DONE: 318 STEPS, 4 CALLS, 10 US OF DELAYS, 32 WRITES, 1 LEFT AS IT IS`; `THE PHY WAS SET UP
  ALREADY (SCRATCH 0x300): THE SHORT PATH`.
- `S1` to `S16` each `DONE; THE OTHER TWO PIPES UNCHANGED`. Table 4: 33 steps, 5 writes; table 76: 289 steps, 23
  writes; `S9 … OTG0 AT 268498 KHZ`; `S10 DRAW DONE; … 1366x768 DRAWN AND READ BACK`; `S13 … THE PITCH READ BACK AS
  WRITTEN`.
- `S15 NO NEW UNDERFLOW: HUBP0 AND ODM0 (THE FIRMWARE'S), HUBP2 AND ODM2, HUBP1 AND ODM1 (HDMI-A-1'S)`;
  `S15 WATCHED 60 FRAMES OF OTG2 IN 990 MS … 0 PASSED BETWEEN TWO READS`;
  `S15 NO UNDERFLOW OR REQUEST TIMEOUT ON ANY OF THE THREE PIPES IN 60 FRAMES`.
- `[COMPOSITOR] THIRD SCREEN 1366X768, PITCH 5632, Bgr, FROM PID 39: THE FIRST MIRRORED AT 1228X768 FROM 69,0; THE ONE
  BEFORE LET GO IN 0 US, MAPPED IN 35 US, DRAWN IN 2171 US`.
- `HDMI-A-2: SECOND HDMI RUN ON OTG2 Lit: 16 STAGES IN PLACE, 422 REGISTERS READ, 178 WRITES, 140767 MS; HDMI-A-1 Lit ON
  OTG1; OFFERED AS THE THIRD SCREEN THROUGH SLOT 8`.

**The maintainer's look:** «все три светятся и отображают загрузочные сообщения» (all three lit, showing the boot
messages), and, asked whether the third has bars at its sides: «Да, полосы есть».

**What it shows and what not:**
- Three pipes fetched at once, each with no underflow or request timeout, on the firmware's clocks and watermarks.
- DPP2 ran on DTO2 as the firmware left it (0/0): stage 11 writes no DTO, and the picture came. Why that suffices is
  not known.
- Not yet: an hour of the standing check on three pipes, plug changes on the machine, the production order (setting
  (e)), table 12's long path, and a start not through Linux.

## The release's behaviour on the Ryzen, and plug changes (2026-11-27, release 202611271414)

The bench image `claude/DRV-ryzen-bench-49-release-three-screens` 7898901a1: this branch at c2f00edea, 000-DRV-0115 and
`main` c0a3a7e8e, with **no bench-only commit**: what a release of this branch does. Through QEMU first: acc-1 "x86:
keys, shell, tools" (597 s) and "x86: services, storage, audio" (1146 s), and the update from the release before on
x32-128. Boot 116. It holds for that machine, those monitors and that release (MC-12.1).

**At the start, in the production order:**
- `THE SECOND OUTPUT: LIGHT, THE WORD AFTER DEVICE 24: STEPS 1 TO 4 ON EACH HDMI SOCKET WITH A MONITOR IN ITS EDID'S MODE, THEN BOTH HPDS FOLLOWED AND THE STANDING CHECKS`, no bring-up line.
- HDMI-A-1: `THE SECOND SCREEN OFFERED 2453.3 MS AFTER DISPLAY'S START`; the compositor `… MAPPED IN 48 US, DRAWN IN 5199 US`.
- HDMI-A-2: `STEP 3'S TIMES (MS): IN ALL 2041.8`, `S15 … WATCHED 60 FRAMES IN 1002 MS CLEAN`, `[COMPOSITOR] THIRD SCREEN 1366X768 … MAPPED IN 60 US, DRAWN IN 1543 US`, `SECOND HDMI RUN ON OTG2 Lit: … 2471 MS`.
- So all three screens were lit 8.6 s after `display` started.
- **The maintainer:** «Светятся» (they are lit).

**Plug changes by the maintainer** (uptime in seconds; each `TAKEN BACK IN` 25 to 26 ms, each light-up 2.3 to 2.5 s):
- **The big monitor's cable out and in** (HDMI-A-1): 3101.353 `STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR`, `SECOND SCREEN GONE (PID 39): LET GO IN 615 US`, `… HDMI-A-2'S PIPE UNCHANGED`, `HDMI-A-2 NOW THE ONLY HDMI SOCKET LIT, ON OTG2`; 3103.592 `HDMI-A-1: A MONITOR PLUGGED IN THERE; HDMI-A-2 IS LIT: THE SECOND HDMI RUN NEXT`, lit again on OTG1 as the second screen. **The maintainer: «Не гас»** (the other did not go dark).
- **The third monitor's cable out and in** (HDMI-A-2): 3111.604 taken back, `THIRD SCREEN GONE (PID 39): LET GO IN 641 US`, `… HDMI-A-1'S PIPE UNCHANGED`; 3115.177 plugged in, lit again on OTG2 as the third screen. **«Не гас»**.
- **The two monitors swapped between the sockets:** both taken back (3120.6, 3122.8), then `HDMI SOCKETS NOW: HDMI-A-1 NO MONITOR; HDMI-A-2 LIT ON OTG1 (16 STAGES, SECOND SCREEN)` with the 3440x1440 monitor at 324 MHz on PHY B, and `HDMI-A-1: … SECOND HDMI RUN ON OTG2 Lit … OFFERED AS THE THIRD SCREEN THROUGH SLOT 8` with the 1366x768 one on PHY A. **«Поменял, показывает»** (swapped, it shows).
- No underflow, no request timeout, no stage failed, and the firmware's pipe unchanged throughout.

**What it shows:** the release lights both sockets, each unplug takes back only its own run, each plug lights its socket
again beside the other, and either socket drives either monitor (PHY B at 324 MHz too).

**The monitors swapped back** (4320.6 to 4330.8 s): both taken back, then HDMI-A-1 lit on OTG1 as the second screen and
HDMI-A-2 on OTG2 as the third, `HDMI SOCKETS NOW: HDMI-A-1 LIT ON OTG1 (16 STAGES, SECOND SCREEN); HDMI-A-2 LIT ON OTG2
(16 STAGES, THIRD SCREEN)`; the maintainer: «мониторы поставил обратно».

**An hour of the standing checks** (read every ten minutes over the server, 3012 to 6734 s of uptime): every ten-minute
line of both runs `NOTHING NEW ON ANY OF THE THREE PIPES`, 579 to 590 checks each, the longest gap 1041 to 1045 ms
between two while lit. Two lines say `DARK 1 TIMES, 6 S IN ALL` and `7 S`: the plug changes above. No underflow line.

**Not shown yet:**
- table 12's long path: every PHY was warm here, since every start came through Linux, which lights both sockets;
- a start not through Linux.

## Both sockets empty under Linux, then plugged: still the short path (2026-11-27, boot 117)

To reach table 12's long path, the maintainer took both HDMI cables out and the Ryzen was restarted through Ubuntu.
Linux then reported `card1-HDMI-A-1 disconnected disabled` and `card1-HDMI-A-2 disconnected disabled`, and MIND Core
started with both sockets empty (`STEP 3 NOT RUN: HPD 1 SENSES NO MONITOR ON HDMI-A-1`, `SECOND HDMI RUN NOT MADE …: NO
MONITOR SENSED IN THE OTHER HDMI SOCKET`). He then plugged the monitors in one at a time, on a sound from the speaker:
- **The big monitor into HDMI-A-1:** `S2 PIXEL CLOCK DONE; … TABLE 12 170 US (321 STEPS, 33 WRITES), THE PHY SET UP
  ALREADY: THE SHORT PATH`; lit in 2027.9 ms, as the second screen. «есть изображение».
- **The third monitor into HDMI-A-2:** `HDMI-A-2: S2 PIXEL CLOCK DONE; … TABLE 12 165 US (331 STEPS, 32 WRITES), THE PHY
  SET UP ALREADY: THE SHORT PATH`; lit in 2457 ms beside it, as the third screen.

**What it says:** both PHYs were set up although Linux had driven neither. Their scratch mark (0x300) is set before
Linux, most likely by the VBIOS when the machine starts. So on this Ryzen a start through Linux always meets the short
path, whatever is plugged in. Table 12's long path stays not shown, as in 213-DRV-0097: it would need a start where
nothing set the PHYs up, which this bench does not give.

## Not known

- Whether the firmware's DISPCLK, DPPCLK (DTO2), DCFCLK and watermarks carry three pipes without underflow. First
  light on pipe 2 fetches nothing (the pattern is OPP2's), so stage 3 does not answer it; stage 4's (d) will (the
  arithmetic in "Stage 4" is a model).
- Why DPP2 runs on the firmware's DTO2 as found (0/0 on release 202611271307, the picture came): stage 11 writes no DTO.
- The compositor's three times at a plug change with two driver screens (at the start: mapped in 35 us, drawn in 2171 us
  for the third, release 202611271307).
- Whether the windows stay clear of the GOP's screen: init does not check it (KRN's note; [0, 0xFA0000) today).
- What the Ryzen's tables write with crtc 2 on PHY B on the hardware. On the model: only the second's blocks (above);
  the model answers the tables' status reads as `answered_on` does, and its counts differ from the bench's.
- Whether a cold PHY B's long path (table 12, 540 steps on the model with CRTC 2) behaves on the hardware (213-DRV-0097):
  a release now meets it at a start not lit through Linux ("The release's behaviour").
- PHY A's FUSE0 when the second run starts: B028003E on release 202611271307, left as it is. If a value ever needs
  changing, stage 2 is refused and undone (the lit pipe's register).
- The firmware's DPPCLK frequency (for the scaler's DLG).
- Whether the compositor's frame time stays within its budget with two copies a change (KRN's to measure).
- Whether an underflow on the lit pipe during the other's watch should undo both: today it does (the newcomer at once,
  the lit one at its next standing check), the cautious side; the bench may show it too strict.
- Whether starts not through Linux leave the same state. Every boot of this machine so far was a start out of Linux.

## Related

[213-DRV-0112](../issues-done/213-DRV-0112-display-a-monitor-on-the-ryzens-other-hdmi-socket.done) (HDMI-A-2 alone, one socket at a time),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror),
[213-DRV-0097](213-DRV-0097-display-table-12s-cold-path-on-the-ryzen.md) (table 12's cold path),
[213-DRV-0099](213-DRV-0099-display-both-screens-by-default.md) (the setting),
`hwdocs/cezanne-display.md` (the survey).
