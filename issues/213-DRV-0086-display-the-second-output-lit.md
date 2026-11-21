# 213-DRV-0086 — `display`: the Ryzen's second output lit, first with the engine's own test pattern

**Type:** driver (a new `display` service) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-21) · **Blocked by:** [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md) (the route), KRN (the device's grant; later the compositor) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

The Ryzen's HDMI monitor stays dark under MIND Core: the firmware sets one output, and nothing drives the display engine ([213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md)). This task is the first light on the second output, by the route [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md) writes down.

## Plan

Each step ends with something a person or a log can check, and none of it is promised by a date (MC-12.3).

1. **A service that only reads.** `display` takes the device from KRN's grant, reads the engine's registers and logs what the firmware left: the pipe behind `DP-2`, and the idle state of the others. This proves the grant and the register map against the survey.
2. **The monitor read:** the HDMI connector's hot-plug state and its EDID over DDC.
3. **The engine's own test pattern on HDMI.** A second pipe's timing generator set to a mode of the EDID that the engine can clock (a common one first, 1920x1080 at 60 Hz, before 3440x1440), the HDMI encoder and its PHY brought up, and the output's built-in colour bars: a picture with no framebuffer.
   - The firmware's `DP-2` pipe is not touched. Its registers are read before and after, and any change is a failure of the step.
   - Every write is bounded and logged; a step that fails leaves the second pipe off.
4. **A framebuffer on the second pipe:** memory the engine can scan out (from KRN), the plane, and a picture drawn by `display` itself.
5. **The compositor** (KRN's): the second screen as a mirror first. Behind a versioned interface, asked of KRN.

## Acceptance criteria

- [ ] `display` runs on the Ryzen from the boot image and logs the engine's state as the survey has it.
- [ ] The HDMI monitor shows the engine's test pattern at a stated mode while the DP monitor keeps MIND Core's screen; the maintainer saw both. This holds for that machine and its two monitors (MC-12.1).
- [ ] Host cases cover the mode's timing arithmetic, the EDID parser and the order of the register writes against a model of the blocks.
- [ ] With KRN's part: MIND Core's screen on both monitors.

## Progress (2026-11-21)

**The host-side parts, in module files for KRN's `display` crate to take in** (KRN makes the crate, its grant and its
boot image, ABI 38; this branch adds no `Cargo.toml`, `main.rs` or linker script, so the two do not collide):

- `display/src/timing.rs`: a mode's totals, refresh rate and TMDS clock, and the timing generator's register values for
  it in the convention Linux programs and Display Core's dump shows; the inverse, for logging what the firmware left.
- `display/src/edid.rs`: EDID 1.3/1.4 and its CTA-861 extension, with no allocation: identity, name, range limits,
  detailed timings, VICs, the HDMI 1.4 and HDMI Forum blocks; the sink's TMDS limit; the largest timing within a clock.
- `display/src/dcn.rs`: DCN 2.1's registers as BAR 5 offsets (segments, blocks, instance strides), the fields first
  light writes, the three VBIOS command tables it calls with their parameter bytes, and `first_light`, the ordered plan
  of step 3 (the DPG's colour squares on a second pipe), which refuses the firmware's pipe and PHY.
- `tests/display_host.rs`, run by `scripts/host_tests.sh` with no line of its own (its loop over `tests/*_host.rs`): the timing arithmetic
  (CTA 1080p60; the Ryzen's two monitors, whose EDID timings give exactly the OTG values Display Core dumped under
  Linux), the parser on synthetic EDIDs (the monitors' own bytes are not kept) and on damaged ones, the register
  addresses against the header's instance 1, and the plan run against a model of the blocks: the firmware's registers
  never written and unchanged, the timing and wiring written, the order (clocks, pixel clock, timing, pattern, start,
  front end, back end, PHY), the tables' parameter bytes, and the refusals.

**KRN's rule for BAR 5** (2026-11-21): `display` never touches bytes 0x42000 to 0x43FFF, the MSI-X table and pending
bits. In `dcn.rs` a `Reg` can only be made by `Reg::new`, which refuses that range before anything else, then any
unaligned offset, then any outside the engine's bytes (0x48 to 0x1AFFF) and the SMU mailbox's three registers;
`R::at` goes through it and gives None past a block's instances; constant assertions keep every window clear of the
MSI-X bytes. Two cases: every offset of the range refused, every register of every instance inside a window, and every
step of first light inside the engine.

Tested on LEG9 only: the 15 cases pass, and the three modules build as `#![no_std]` with `-D warnings`. Nothing of it
has run on the Ryzen; the plan is the survey's route ([213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md)),
not a guarantee (MC-12.3). Still open before step 3 can run: the VBIOS command tables need an interpreter (or their
writes known) and the VBIOS image from the ACPI VFCT table, and the firmware's own state is to be read under MIND Core
(step 1, with KRN's grant).

## Progress: step 1, ready to run (2026-11-21)

On `claude/213-DRV-0086-display-reads-the-engine`, built on KRN's
[213-KRN-0270](213-KRN-0270-the-radeons-registers-for-display.md) (the `display` boot image and `init`'s grant of BAR 5
in `SLOT_DEV0`, the command register as the firmware left it; ABI 37, handoff 38) with the survey branch merged in.
`display/src/main.rs` replaces KRN's stub:

- It logs the grant as the stub did (`[DISPLAY] PCI 1002:1638 (DEVICE n), COMMAND xxxx, BAR 5 AT xxxxxxxx, k KIB
  GRANTED; ...`) and runs step 1 only on the Radeon `1002:1638` with a grant of at least 512 KiB.
- **`BATCHES_ALLOWED`, set to 0:** how many of the seven batches this build may read. At 0 it maps nothing and reads
  nothing; it logs each batch and every register it would read (156 lines of step 1, paced 20 ms: about 3 s). It is raised one run at a time,
  on the owner session's word.
- `display/src/batches.rs`: the seven batches of the survey's proposal (hwdocs/cezanne-display.md), registers Display
  Core itself reads: 1 HPD (10), 2 the OTGs' state (76), 3 the power domains (8), 4 the HUBPs batch 3 shows powered
  (5 each), 5 the DIG front and back ends of the board's PHYs (13), 6 the aperture (6), 7 the clocks (14). Each batch is
  named in the log, then 1 s passes, then each register is read and its value logged at once, so that a hang names its
  register; 3 s between batches; a summary decoded after each (the OTGs' timings, the domains, the surfaces, the DIG
  wiring, the aperture as GPU addresses). The bus it reads through has no write, and a `Reg` can name no MSI-X byte.
- `tests/display_batches_host.rs`, 9 cases: the batches as proposed and inside the engine; a build of 0 reads nothing
  and names every batch; a build of n reads exactly batches 1 to n; each batch named and paused before its first read;
  the state Linux's Display Core dumped on the Ryzen decodes as it printed it (OTG 0 3440x1440 in 3600x1500, OTG 1
  2560x1600 in 2720x1646, OTGs 2 and 3 off); batch 4 only for the HUBPs batch 3 shows powered; an unmapped register not
  read; every line within a log record; every register of the map at the header's offset.

Tested on LEG9 only (the build and the host cases). It has not run on the Ryzen: it needs the stick rewritten with an
ABI 37 release that holds KRN's image; the owner session says when. What to look for in the first run's log: KRN's
`[INIT] display: ... COMMAND xxxx BEFORE THE GRANT, xxxx AFTER`, then `[DISPLAY] PCI 1002:1638 ...`, the seven `NOT
READ` batches with their registers, `STEP 1 DONE: 0 OF 7 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN` and the command
register after; DP-2 keeps its picture.

## Related

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md), [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md), [213-KRN-0098](../issues-done/213-KRN-0098-black-screen-when-the-shell-takes-it.done).
