# 213-DRV-0092 — `display`: MIND Core's picture on the Ryzen's second output

**Type:** driver (`display`, step 4) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — for stages 9 to 15; KRN (the compositor's second screen, a versioned interface) for the last criterion · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

[213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) lit the Ryzen's HDMI-A-1 with the display engine's own
test pattern (first light, 2026-11-22: the colour squares at 1920x1080, seen by the maintainer). The maintainer's goal is
MIND Core's own screen on that monitor. That needs pipe 1 to fetch pixels from a surface in memory, in place of OPP1's
pattern generator, and the compositor to draw into that surface.

## Plan

Step 4 adds stages 9 to 15 after step 3's eight, in the same frame:
- `STAGES_ALLOWED` counts on to 15; the committed value stays 0;
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
  firmware's HUBP0 holds 0xA00 against the GOP's stride of 2560, unexplained. Stage 15's picture settles it, and the
  log gives both the field written and HUBP0's.

| Stage | What it does | Undo | HDMI-A-1 | DP-2 |
|---|---|---|---|---|
| 9 reference | step 1's batch 9 has read the firmware's pipe 0: HUBP0's request, DLG and TTU registers, DPP0, the MPC tree, DCHUBBUB's watermarks, DCCG's DPP clock DTOs. Stage 9 logs pipe 1's values derived from them; no device access | none | squares | unchanged |
| 10 draw | `display` draws into the surface, with no register written (below) | none: nothing scans it yet | squares | unchanged |
| 11 clocks | DPPCLK1's DTO, DPP1's clock, HUBP1's DCFCLK | restore | squares | unchanged; **watched** |
| 12 DPP1 | ARGB 8888 in, colour stages in bypass, scaler 1:1 | restore | squares | unchanged |
| 13 HUBP1, blanked | surface, pitch, address, viewport, request, DLG and TTU | restore | squares | unchanged |
| 14 MPC | a free MPCC from DPP1 to OPP1 | restore | squares (the pattern overrides) | unchanged |
| 15 unblank, pattern off | `HUBP_BLANK_EN` 0, then `DPG_EN` 0 | pattern on, HUBP1 blanked, 14 to 11 restored | **the drawn picture** | unchanged; **watched** |

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

**The other side:** a second screen offered to the compositor. KRN owns the compositor; the owner session sends KRN
the request.
- **What `display` gives it:** that a second screen exists and when it comes or goes (HPD 1); its width, height,
  pitch in bytes and format (0x00RRGGBB, the GOP's); where its pixels are (a mapping `display` lends); and when a
  frame may be shown (OTG1's vertical blank).
- **The mirror first:** the compositor copies the same changed pixels into both screens. An extended screen comes
  later, and needs APP's `wm` too.

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
| 13 | HUBP1, still blanked: the surface at FB base + 0x1000000, pitch field 1919, viewport 1920x1080, the requestor, VM aperture, prefetch, QoS and cursor settings as the firmware HUBP's; DLG and TTU from stage 9; VREADY as the firmware's. The log gives the pitch field written and the firmware's |
| 14 | MPCC1 from DPP1 to OPP1 (bottom none, the firmware MPCC's blending); `MPC_OUT1_MUX` to MPCC1 |
| 15 | HUBP1 unblanked; three frames; no new underflow on either pipe; the pattern off; three frames; no new underflow |

- **The guard** lets pipe 1's HUBP, DPP, MPCC and output mux through only from stage 11. First light's stages and their
  tables cannot reach them, and pipe 0's blocks stay refused.
- **Not written, as decided:**
  - pipe 1's DPP clock DTO: left disabled, so pipe 1 runs at the full global DPP clock, as batch 10 shows it;
  - `hubp21_init`'s chicken bit for flips between DPTE and meta (no VM, no meta here);
  - DSCL's own blank timing, which Linux's bypass path leaves alone too.
- **Copied rather than computed:** the prefetch, VM-row, QoS and cursor values. They were set for the firmware's larger
  mode, which leaves pipe 1 more time, not less. A mistake shows as an underflow, which fails stage 15 and undoes it.
- **Host cases** (`display_light_host.rs`, 25; `display_picture_host.rs`, 4; batches, 11):
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

## Acceptance criteria

- [ ] Stages 9 to 15 exist, switched off by default, each logged before it writes and undone on failure; host cases on
  the state the Ryzen showed (213-DRV-0086's `WARM`) cover each stage, its undo and the widened watch.
- [ ] Stage 10's picture, read back on the model, has the border, the diagonals, the cross and the letters in place.
- [ ] On the Ryzen, HDMI-A-1 shows stage 10's picture through pipe 1 with straight edges, while DP-2 keeps MIND Core's
  screen through stages 11 to 15; the maintainer saw both. This holds for that machine and its two monitors (MC-12.1).
- [ ] With KRN's interface: the compositor shows MIND Core's screen on HDMI-A-1, a mirror of DP-2's.

## Related

[213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light, step 4's first plan),
[213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md),
[213-KRN-0271](213-KRN-0271-displays-grant-for-first-light.md) (slot 5).
