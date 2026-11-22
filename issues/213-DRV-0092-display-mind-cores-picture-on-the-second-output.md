# 213-DRV-0092 — `display`: MIND Core's picture on the Ryzen's second output

**Type:** driver (`display`, step 4) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — (the compositor's second screen: KRN's display.wit 1.3, [213-KRN-0272](213-KRN-0272-a-second-screen-for-the-compositor.md), merged here for stage 16) · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

[213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) lit the Ryzen's HDMI-A-1 with the display engine's own
test pattern (first light, 2026-11-22: the colour squares at 1920x1080, seen by the maintainer). The maintainer's goal is
MIND Core's own screen on that monitor. That needs pipe 1 to fetch pixels from a surface in memory, in place of OPP1's
pattern generator, and the compositor to draw into that surface.

## Plan

Step 4 adds stages 9 to 16 after step 3's eight, in the same frame:
- `STAGES_ALLOWED` counts on to 16; the committed value stays 0;
- each stage is logged with every write before any is made, and undone with every stage before it if it fails;
- the firmware's pipe (DP-2) is never written and is read after every stage;
- nothing is read that step 1 has not read on the same boot.

**The surface.**
- **Where:** BAR 0's slot 5 (213-KRN-0271), [0x0100_0000, 0x0240_0000), behind the GOP's framebuffer. `display` maps
  it write-combined.
- **Its GPU address:** BAR 0 offset X is GPU address 0xF4_0000_0000 + X. HUBP0 scans 0xF4_0000_0000, which is BAR 0's
  offset 0 and the GOP's framebuffer, and `DCN_VM_FB_LOCATION_BASE` reads 0xF4_0000_0000. So the surface is at
  0xF4_0100_0000.
- **Its shape:** 1920x1080, `SURFACE_PIXEL_FORMAT` 8 (32-bit ARGB), linear. The pitch is 1920 pixels (7680 bytes).
- **The pitch convention.** The `PITCH` field holds the pitch less one, as Display Core writes it, so 1919. The
  firmware's HUBP0 holds 0xA00 against the GOP's stride of 2560. Both conventions are straight on this machine, so the
  hardware rounds the field and 1919 and 1920 give the same row (Progress, "The pitch"). The log gives the field
  written, HUBP0's, and the field read back.

| Stage | What it does | Undo | HDMI-A-1 | DP-2 |
|---|---|---|---|---|
| 9 reference | step 1's batch 9 has read the firmware's pipe 0: HUBP0's request, DLG and TTU registers, DPP0, the MPC tree, DCHUBBUB's watermarks, DCCG's DPP clock DTOs. Stage 9 logs pipe 1's values derived from them; no device access | none | squares | unchanged |
| 10 draw | `display` draws into the surface, with no register written (below) | none: nothing scans it yet | squares | unchanged |
| 11 clocks | DPPCLK1's DTO, DPP1's clock, HUBP1's DCFCLK | restore | squares | unchanged; **watched** |
| 12 DPP1 | ARGB 8888 in, colour stages in bypass, scaler 1:1 | restore | squares | unchanged |
| 13 HUBP1, blanked | surface, pitch, address, viewport, request, DLG and TTU | restore | squares | unchanged |
| 14 MPC | a free MPCC from DPP1 to OPP1 | restore | squares (the pattern overrides) | unchanged |
| 15 unblank, pattern off | `HUBP_BLANK_EN` 0, then `DPG_EN` 0 | pattern on, HUBP1 blanked, 14 to 11 restored | **the drawn picture** | unchanged; **watched** |
| 16 offer | `offer_screen` (display.wit 1.3): 1920x1080, pitch 7680 bytes, `bgr`, slot 5 lent; no register written | `screen_gone`, before stage 15's undo | **MIND Core's screen**: DP-2's mirrored at 1728x1080 from x 96, black either side | unchanged |

**Stage 10's picture is its own evidence**, so that a photo of the monitor says what it shows:
- in large block letters (a small font built in), the release's version and `MIND CORE / DISPLAY / HDMI-A-1 1920x1080`;
- a thick white border, both diagonals and a centre cross.

With the pitch off by one pixel, every row moves by a pixel against the last. The border's sides and the cross's
upright then lean by 45 degrees over the screen's height, and the letters smear into slants: unmistakable from across
the room.

**The watch, widened.** Stage 15 also reads the underflow statuses of HUBP0 and HUBP1 and of OTG0 and OTG1. Any
underflow fails the stage. From stage 13, HUBP1 is on the route, while DPP0, MPCC0 and HUBP0 stay refused.

**Runs on the Ryzen:** stage by stage, each on the owner session's word, with the maintainer watching DP-2 at stages 11
and 15. Stages 9 and 10 may run once their host cases pass.

**Stage 2's path in the log.** Stage 2 says which path table 12 took. Either `THE PHY WAS SET UP ALREADY (SCRATCH
0x300): THE SHORT PATH`, or `... THE LONG PATH: n WRITES THROUGH ITS BUS`. So a long path never passes unnoticed in a
log (213-DRV-0086, "Table 12's cold path").

**The other side:** a second screen offered to the compositor. KRN owns the compositor; the owner session sent KRN
the request.
- **What `display` gives it:** that a second screen exists and when it comes or goes (HPD 1); its width, height,
  pitch in bytes and format (0x00RRGGBB, the GOP's); where its pixels are (a mapping `display` lends); and when a
  frame may be shown (OTG1's vertical blank).
- **The mirror first:** the compositor copies the same changed pixels into both screens. An extended screen comes
  later, and needs APP's `wm` too.
- **KRN's answer** ([213-KRN-0272](213-KRN-0272-a-second-screen-for-the-compositor.md), display.wit 1.3):
  - `offer-screen(width, height, pitch, format, pixels: borrow<mmio>)` and `screen-gone()`, from a client badged
    `BADGE_SCREEN`, which init gives `display` in `SLOT_DISPLAY`;
  - the compositor mirrors the first screen into the second, scaled to fit and centred, and only what changed;
  - 1.3 has no vertical blank: the compositor draws when its own frame comes.

## Progress (2026-11-22)

**Stages 9 and 10** (dbc8f6e3e), with step 1's batches 9 and 10 (reads only, 43 and 68 registers, all inside the
window). They read the firmware's pipe as batches 2 and 4 name it:
- its HUBP's request, DLG, TTU, viewport, VM aperture, flip and VMID registers, and its OTG's start-up parameters;
- its DPP;
- what every pipe shares: the MPC tree, DCHUBBUB's four watermark sets, DCCG's DPP clock DTOs.

Step 3 now needs all ten batches.
- **Stage 9** counts 30 of the firmware OTG's frames for its pixel clock, and with `REF_FREQ_TO_PIX_FREQ` the reference
  clock.
  - It logs the DLG formulas against what the firmware set, then pipe 1's values for 1080p60.
  - The formulas are Display Core's own (`display_rq_dlg_calc_21.c`, Linux v7.0, fetched with the HUBP, DPP, MPC and DCCG
    sources into the scratchpad, not the repository): REF/PIX, PER_HTOTAL, H_BLANK_END, V_BLANK_END, MIN_DST_Y_NEXT_START,
    line and request delivery, 64 pixels a request for a linear 32-bit surface.
  - It also notes the underflow statuses of both pipes.
- **Stage 10** draws the picture into slot 5 and reads nine samples back. The picture is a 12-pixel border, both
  diagonals, a centre cross, and `MIND CORE / DISPLAY / HDMI-A-1 1920X1080 / BUILD <commit>` in block letters (`picture.rs`).
  - The release's version cannot be drawn: it is given out at release time, after the build. The build's commit
    (`MIND_BUILD_COMMIT`, 02_build.sh) is drawn instead.
- **Stage 2** says which path table 12 took: `THE PHY WAS SET UP ALREADY (SCRATCH 0x300): THE SHORT PATH, NO WRITE
  THROUGH ITS BUS`, or `... THE LONG PATH: n WRITES THROUGH ITS BUS (m INTO THE PHY)`.

**Stages 11 to 15** (1e2855c60, switched off). They are built when stage 9 has run, in Display Core's order, and each is
logged in full before its first write:

| Stage | What it writes |
|---|---|
| 11 | DPP1's and HUBP1's clock enables |
| 12 | DPP1's format, colour stages and scaler as the firmware DPP's; recout and MPC size 1920x1080 |
| 13 | HUBP1, still blanked: the surface at FB base + 0x1000000, pitch field 1919, viewport 1920x1080, the requestor, VM aperture, prefetch, QoS and cursor settings as the firmware HUBP's; DLG and TTU from stage 9; VREADY as the firmware's. The log gives the pitch field written and the firmware's, and the pitch register read back just after its write; since boot 74 also, first, OTG1's global sync for a plane (VUPDATE offset and width, VREADY offset), then two frames; the QoS as Display Core's (level 8, high watermark four lines); `HUBPREQ_DEBUG_DB` = 0x100 (the DLG's test mode off) and VREADY after the sync by `hubp2_vready_at_or_After_vsync`'s rule |
| 14 | MPCC1 from DPP1 to OPP1 (bottom none, the firmware MPCC's blending); `MPC_OUT1_MUX` to MPCC1 |
| 15 | HUBP1 unblanked; three frames; no new underflow or request timeout on either pipe; the pattern off; then the watch: 300 frames of OTG1 read one by one without sleeping, both pipes' HUBP and ODM statuses at each. The firmware's showing anything new puts stage 15 back at once; pipe 1's are kept by frame, cleared and watched on, and after the watch fail the stage, put back at once (the pattern on, then HUBP1's CNTL as stage 13 left it), then 14 to 1 |

- **The guard** lets pipe 1's HUBP, DPP, MPCC and output mux through only from stage 11. First light's stages and their
  tables cannot reach them, and pipe 0's blocks stay refused.
- **Not written, as decided:**
  - pipe 1's DPP clock DTO: left disabled, so pipe 1 runs at the full global DPP clock, as batch 10 shows it;
  - `hubp21_init`'s chicken bit for flips between DPTE and meta (no VM, no meta here);
  - DSCL's own blank timing, which Linux's bypass path leaves alone too.
- **Copied rather than computed:** the prefetch, VM-row, QoS and cursor values. They were set for the firmware's larger
  mode, which leaves pipe 1 more time, not less. A mistake shows as an underflow, which fails stage 15 and undoes it.
- **Host cases** (`display_light_host.rs`, 29; `display_picture_host.rs`, 4; batches, 11):
  - all fifteen stages on the model, with the registers written and copied, and the guard by stage;
  - the undo of all fifteen;
  - an underflow at stage 15, a busy MPCC, stage 9 without a reference, stage 10 without a surface;
  - the picture's glyphs and layout, and the 45-degree lean an off-by-one pitch gives.

**The first release of stages 9 and 10 faulted (2026-11-22, boot 69, release 202611220523, dbc8f6e3e, switches
10/10/false).**
- **What happened:** `display` read all ten batches (282 registers, nothing written), then took a page fault (vector 14)
  before step 3's first line. init restarted it, and it faulted the same way each time, about every 52 s. No stage ran
  and nothing was written.
- **The cause:** display's ELF asked for no stack (no `PT_GNU_STACK`), so it ran on the 64 KiB a task gets by default,
  with an unmapped guard page below (262-KRN-0164). Steps 3 and 4 hold about 76 KB by value:
  - the VBIOS interpreter (20,840 bytes) and the journal (16,392), as `Box::new` builds them on the stack;
  - `Light` (5,736) and its plans (3,848 each, three live);
  - step 1's values (2,576) and stage 10's row (7,680).

  That is past the guard page.
- **Fixed in 13c77c7b9:**
  - `linker.ld` asks for 256 KiB.
  - `light::STEP3_STACK_BYTES` (76,240) is bounded by half of it, at compile time and in a host case that reads
    `linker.ld`. The case also shows that the 64 KiB would have failed the bound.
- **Step 3 once a boot.** Since init restarts a faulting program, step 3 now leaves a mark before stage 1:
  `OTG1_OTG_VERTICAL_INTERRUPT2_POSITION = 0x5A5A`, a register whose interrupt is off.
  - A start that finds the mark runs no stage and writes nothing.
  - The undo never takes the mark back.
  - A machine boot clears OTG1's registers: step 1 has read them 0 after every boot, though Linux drove OTG1 before.

**Boot 70 (2026-11-22, release 202611220537, 13c77c7b9, switches 10/10/false): stage 9 refused, everything undone.**
- **What happened:** stages 1 to 8 lit HDMI-A-1 again. Stage 9 then refused, and stages 8 to 1 were undone: 98 restores
  (3 of them of registers only the off calls wrote), every off call run to its end.
- **The two errors:**
  - The reference clock came out 43,837 kHz. It had been taken from the firmware's `REF_FREQ_TO_PIX_FREQ`, which is the
    GOP's own value. Its `PER_HTOTAL` gives 48 MHz by Display Core's formula, as `REFCLK_CNTL` 0 (the crystal) and the
    global timer's divider 1 say. The GOP's line and request delivery are 0, and its blank ends are not Display Core's.
  - The MPC check wanted MPCC1 free (`OPP_ID` 15). The firmware wires every MPCC n from DPP n to OPP n in passthrough
    (`MPCC0_MPCC_CONTROL` 0xFFFF0461), so MPCC1's `OPP_ID` is 1, its own OPP.
- **Fixed in d562026b2:**
  - The reference clock comes from `PER_HTOTAL`, rounded to the MHz within 0.5%, and both estimates are logged.
  - Pipe 1's DLG and TTU are computed from the clocks directly, with 30 requests a line (1920 / 64), not the GOP's.
  - The MPC check reads the tree as the firmware wired it. Its refusal says which of three reasons, with the values.
  - Stage 9, 10 or 16 not ready stops step 4 and leaves the stages before it in place.

**Boot 71 (2026-11-22, release 202611220554, d562026b2, switches 10/10/false): stages 9 and 10 went through.**
- **Stage 9:** OTG0 counted 30 frames in 500,246 us (59.970 Hz, pixel clock 268,495 kHz). The reference clock was
  47,998 kHz by `PER_HTOTAL` (43,835 by `REF_FREQ_TO_PIX_FREQ`), taken as 48,000. Pipe 1 for 1920x1080 at 148.5 MHz:
  REF/PIX 0.3232, `PER_HTOTAL` 182,044, `H_BLANK_END` 62, `V_BLANK_END` 41, line 620, request 21,183, next start 4,484.
  These are the values the host cases expect.
- **Stage 10:** drawn at slot 5's start with a pitch of 1920 pixels, nine samples read back as drawn.
- **Step 3 stopped after stage 10** with its 95 writes left in place. The squares stayed on HDMI-A-1 and DP-2 was
  unchanged.

**The pitch.** Display Core writes the pitch less one (`hubp2_program_size`), and step 4 writes 1919 for 1920. The
firmware's HUBP0 holds 2560 for its 2560 pixels.
- **What this machine shows:** both conventions give a straight picture here:
  - the GOP's 2560 on DP-2 now;
  - Linux's 3455 for 3456 and 2559 for 2560 (Linux's state of 2026-11-21, in `display_batches_host.rs`).
- **So the hardware rounds the field.** It rounds 2559 and 2560 to the same row, and 3455 and 3456 too, so its unit is at
  most 128 pixels.
- **What follows for step 4:** 1920 is a multiple of 128. So 1919 and 1920 give the same 1920-pixel row, and stage 15's
  picture cannot tell the two conventions apart. Step 4 keeps Display Core's 1919.
- **The read-back:** stage 13 reads the pitch register back right after writing it, and logs it with its field. A
  register that keeps other bits than those written shows there.
- **Step 1's batch 4 summary:** it printed the field plus one (`PITCH 2561` for the GOP's 2560), so it now prints the
  field as read (`PITCH FIELD 2560`).

**Stages 11 to 15 on the Ryzen.** The owner session's plan is three releases: 11 to 14 first, then 15, then 16 with KRN's
interface.
- **Timing:** at boot 71's pace (about 21 ms a log line, 1 s before each stage's writes, 3 s after each stage), the
  stages write at about:
  - 102 s (11);
  - 106.5 s (12);
  - 112 s (13);
  - 116.7 s (14);
  - 121 s (15).
- **What the maintainer should see:**
  - **Stages 11 to 14:** they turn on pipe 1's clocks, then set its DPP, its HUBP (blanked) and its MPCC to OPP1. The
    squares stay on HDMI-A-1, because OPP1's pattern generator replaces whatever the MPC sends. DP-2 should not change:
    no write reaches pipe 0's blocks.
  - **Stage 15:** HUBP1 is unblanked behind the pattern; three frames later, with no new underflow, the pattern goes
    off. The squares then give way to the picture, on a dark blue ground:
    - a white border all round;
    - both diagonals, corner to corner;
    - a cross through the centre;
    - a dark panel with the four lines in yellow.
  - **An underflow:** HUBP1 is blanked again behind the pattern at once. Everything is then undone, and HDMI-A-1 goes
    dark.

**Stage 16** (KRN's 213-KRN-0272 merged, ab838d73b). `screen.rs` is `display`'s client of the compositor:
- **The offer:** it offers the surface with what stage 16 passes (1920x1080, a row 7680 bytes apart), in `bgr` (the
  picture's 0x00RRGGBB words), with slot 5 lent as an MMIO capability (copied: `display` keeps its range).
- **Without a client in `SLOT_DISPLAY`:** `UNAVAILABLE`, and no call.
- **The answers** are named as step 4 names them: `NO SCREEN`, `NO MEMORY`, `DENIED`, `BAD SCREEN`, or `CALL` for a call
  that failed or an answer outside the interface.
- **Not ready:** a refused offer stops step 4 with stages 1 to 15 in place, and with them the picture on HDMI-A-1.
- **The undo:** `screen_gone` comes first, then stage 15 is taken back. A refused `screen_gone` is logged, and the undo
  goes on.
- **Host cases:**
  - `display_light_host.rs` (31 with stage 15's watch below):
    - stage 16 offers once, after stage 15's last write, and writes no register;
    - the undo order, with `screen_gone` refused too;
    - each refusal as not ready, with the undo the build asks for;
    - the lines of a build that stops at 15, and of a build of 0.
  - `display_screen_host.rs`, 3: the offer and `screen_gone` run through libmind's generated client and a model of the
    IPC into the generated `decode`, with the compositor's answers by its `second_screen`'s rules and back:
    - 1920x1080, pitch 7680, `bgr` and an MMIO capability, as the compositor decodes them;
    - denied without `BADGE_SCREEN`;
    - `bad-screen` for a range that does not hold the shape, or for a pitch given in pixels;
    - a memory capability in place of MMIO refused by the schema check;
    - an undefined answer and a failed call taken as `CALL`.
- **On the Ryzen:** nothing of stage 16 has run there. HDMI-A-1 should then show DP-2's screen, mirrored at 1728x1080
  from x 96, black either side.

**Boot 73 (2026-11-22, release 202611220637, switches 10/14/false): stages 11 to 14 went through.**
- **The writes:** each stage was logged and done, with the firmware's pipe unchanged:
  - `S11 PIPE CLOCKS DONE` at 102.1 s;
  - `S12 DPP DONE` at 106.9 s;
  - `S13 HUBP DONE` at 112.6 s, after `S13 READ BACK HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 0000077F, AS WRITTEN: FIELD 1919`;
  - `S14 MPC DONE` at 116.9 s.
- **The stop:** step 3 stopped after stage 14 with its 156 writes left in place. The squares stayed on HDMI-A-1.

**Boot 74 (2026-11-22, release 202611220644, switches 10/15/false): stage 15 met an underflow.**
- **What the log shows:**

  ```
  121.093 S15 WROTE HUBP1_DCHUBP_CNTL AT 0EE3C = 000F001A (WAS 000F001B)
  121.148 S15 OTG1 COUNTED 3 FRAMES IN 34 MS
  121.181 S15 NO NEW UNDERFLOW: HUBP0 AND ODM0 (THE FIRMWARE'S), HUBP1 AND ODM1
  121.201 S15 WROTE DPG1_DPG_CONTROL AT 135B8 = 00661000 (WAS 00661001)
  121.899 S15 OTG1 COUNTED 41 FRAMES IN 676 MS
  121.982 S15 UNBLANK FAILED: AN UNDERFLOW AT 0EE3C (102F0012); ITS WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; UNDOING STAGES 14 TO 1
  ```
- **What came after:** the two writes were put back before the line (121.919 and 121.940), then stages 14 to 1 were undone. HDMI-A-1 went dark, and DP-2's pipe stayed unchanged.
- **HUBP1's CNTL, 000F001A to 102F0012, field by field** (`dcn_2_1_0_sh_mask.h`):

  | Field | 000F001A | 102F0012 |
  |---|---|---|
  | `HUBP_BLANK_EN` (0) | 0 | 0 |
  | `HUBP_NO_OUTSTANDING_REQ` (1) | 1 | 1 |
  | `HUBP_IN_BLANK` (3) | 1 | **0** |
  | `HUBP_VTG_SEL` (7:4) | 1 | 1 |
  | `HUBP_VREADY_AT_OR_AFTER_VSYNC` (8) | 0 | 0 |
  | `HUBP_TTU_DISABLE` (12) | 0 | 0 |
  | `HUBP_XRQ_NO_OUTSTANDING_REQ` (19:16) | F | F |
  | `HUBP_TIMEOUT_STATUS` (23:20) | 0 | **2** |
  | `HUBP_UNDERFLOW_STATUS` (30:28) | 0 | **1** |

  The fields mean the following:
  - `IN_BLANK` fell: the pipe left blank, as unblanking it should.
  - `UNDERFLOW_STATUS` rose: a sticky record of an underflow.
  - `TIMEOUT_STATUS` rose to 2, so a request timed out. Display Core never reads it.
  - At the read, no request was outstanding, as the two "no outstanding" fields show.
  - The firmware's HUBP0 (000F000A) had neither status.
- **The reading: a real underflow, not a side effect of the unblank.**
  - Linux clears a HUBP's underflow only in `dcn10_did_underflow_occur`, a check helper, and an OPTC's only in DCN 1.0's
    `false_optc_underflow_wa`. Neither runs when a plane is enabled (`dcn10_hwseq.c`, `dcn20_hwseq.c`, Linux v7.0).
  - The three frames behind the pattern showed nothing. The status rose once the pattern stopped hiding the pipe, and it
    came with a timeout.
- **The 41 frames:** they were 676 ms at the firmware's 60 Hz (41 frames take about 683 ms). The frame wait sleeps 1 ms
  between reads (`delay_us` → `mind::time::sleep`). Its first sleep after the pattern went off came back after 676 ms,
  and no other task logged meanwhile. So the status was read once, after those frames, and the frame it rose in is not
  known.

**What stage 13 did not program as Display Core does for a pipe going live** (`dcn20_program_pipe`,
`dcn20_program_tg`, `dcn20_enable_plane`, `dcn20_update_dchubp_dpp`, `hubp21_setup`, `hubp2_setup_interdependent`,
`hubp2_vready_at_or_After_vsync`, `display_rq_dlg_calc_21.c`; Linux v7.0):

| What | Display Core | Stage 13 (boot 74) | Since this commit |
|---|---|---|---|
| OTG1's global sync | `program_global_sync` with DML's values when the plane is added, then `set_vtg_params` | first light's, for no plane: VSTARTUP 36, VUPDATE offset 0 width 1, VREADY 1 | VUPDATE offset 550 (a quarter line, DML's `VUpdateOffsetPix`), width 177 and VREADY 166 (the firmware's 320 and 300 at 268.5 MHz, the same times at 148.5); VSTARTUP kept |
| `HUBP_VREADY_AT_OR_AFTER_VSYNC` | 1 when VSTARTUP − (VREADY + VUPDATE width + offset) / htotal ≤ the blank's end: 1 here | 0, the firmware's | 1, by that rule |
| `HUBPREQ_DEBUG_DB` | 0x100 ("disable_dlg_test_mode") before every setup | not written | 0x100; batch 9 reads the firmware's |
| `QoS_LEVEL_FIXED` (surface, cursor) | 8, ramp on | 0: only `REFCYC_PER_REQ_DELIVERY` was written over HUBP1's 0 | 8; the firmware's pipe has 8 |
| `QoS_LEVEL_HIGH_WM` | 4 × htotal × ref/pix: 2844 at 48 MHz | 1682, pipe 0's | 2844; low watermark 0 |
| `MIN_TTU_VBLANK`, prefetch, VBLANK parameters, `DST_AFTER_SCALER` | from DML for the mode and VSTARTUP | the firmware's pipe's | unchanged: DML is not ported; the firmware's VSTARTUP is 13, pipe 1's 36 lines leave more time |
| Line and request delivery | Display Core's (620, 21183) | Display Core's | unchanged; the firmware's own pipe has 0 for both (no deadline) |
| Watermarks A to D, arbiter, `DRAM_STATE_CNTL` | recomputed for both pipes | the firmware's, one set for all: urgent 510, SR enter 1210, SR exit 1060, DRAM 2210; self-refresh forced off (2) | unchanged |
| DPP clock DTO for DPP1 | set to the pipe's DPPCLK | disabled: the full global DPPCLK | unchanged (more clock, not less) |
| `DCN_VM_MX_L1_TLB_CNTL` | L1 TLB on, `SYSTEM_ACCESS_MODE` 3 | 0, the firmware's | unchanged: the firmware's pipe scans with 0 |
| SMU: hard minimum DCFCLK, deep-sleep DCFCLK, display count | sent for two displays | as the firmware left them, for one | unchanged: step 4 does not talk to the SMU |

- **The watch** (stage 15 since this commit): after the pattern goes off, 300 frames of OTG1 (5 s), read one by one
  without sleeping.
  - Each new frame reads HUBP0, ODM0, HUBP1 and ODM1.
  - The firmware's pipe showing a new underflow or timeout puts stage 15 back at once.
  - Pipe 1's are kept by frame and cleared: `HUBP_UNDERFLOW_CLEAR`, `HUBP_TIMEOUT_STATUS_CLEAR` and
    `OPTC_UNDERFLOW_CLEAR`, each written 1 then 0, over the register's own bits without its read-only statuses.
  - After the watch, any of pipe 1's fails the stage, which is put back at once and then undone whole.
  - The log names:
    - the frames that passed between two reads;
    - the longest gap between reads, and the frame it came before;
    - the first frame of each status, and how many frames had it;
    - all 300 frames, 60 a line: `.` none, `U` underflow, `T` timeout, `B` both, `O` ODM, `X` ODM and HUBP, `-` not
      read.
- **Host cases** (`display_light_host.rs`, 29): boot 70's values with the new stage 13 writes, and the watch:
  - clean, 300 frames;
  - a one-off underflow some 17 frames in, kept at its frame, the stage failing after the full watch;
  - an underflow that comes back each frame;
  - the firmware's HUBP underflowing during the watch, put back at once;
  - a request timeout;
  - a 676 ms stall of display, shown as 40 frames not read.
- **Not shown:**
  - which of the differences caused boot 74's underflow: only the next run can tell;
  - DML's prefetch values for pipe 1;
  - the SMU's view of two displays.

## Acceptance criteria

- [ ] Stages 9 to 15 exist, switched off by default, each logged before it writes and undone on failure; host cases on
  the state the Ryzen showed (213-DRV-0086's `WARM`) cover each stage, its undo and the widened watch.
- [ ] Stage 10's picture, read back on the model, has the border, the diagonals, the cross and the letters in place.
- [ ] On the Ryzen, HDMI-A-1 shows stage 10's picture through pipe 1 with straight edges, while DP-2 keeps MIND Core's
  screen through stages 11 to 15; the maintainer saw both. This holds for that machine and its two monitors (MC-12.1).
- [ ] With KRN's interface (stage 16): the compositor shows MIND Core's screen on HDMI-A-1, a mirror of DP-2's.

## Related

[213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light, step 4's first plan),
[213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md),
[213-KRN-0271](213-KRN-0271-displays-grant-for-first-light.md) (slot 5).
