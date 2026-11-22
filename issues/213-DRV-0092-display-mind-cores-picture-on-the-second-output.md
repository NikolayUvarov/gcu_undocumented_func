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
