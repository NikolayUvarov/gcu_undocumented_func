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
[213-KRN-0270](213-KRN-0270-the-radeons-registers-for-display.md) and
[213-KRN-0271](213-KRN-0271-displays-grant-for-first-light.md) (the `display` boot image; `init`'s grant of two windows
of BAR 5, the engine's registers [0, 0x1B000) in slot 2 and the SMU mailbox's page [0x58000, 0x59000) in slot 3, BAR 0
[0x1000000, 0x2400000) in slot 5 and a read-only copy of the ACPI VFCT table in slot 4; the command register left at the
firmware's 0006; ABI 37, handoff 38) with the survey branch merged in. `display/src/main.rs` replaces KRN's stub:

- It logs each slot and the VBIOS image's identity as the stub did (KRN's task closes on those lines), then runs step 1
  only on the Radeon `1002:1638` with an engine window of at least 0x1B000 bytes, and ends with `NO REGISTER READ OR
  WRITTEN` when it read none (`n REGISTERS READ, NONE WRITTEN` otherwise) and the command register after. Step 1 reads
  neither the mailbox nor BAR 0 nor the VBIOS copy beyond its header.
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

Tested on LEG9 (the build and the host cases), then on the Ryzen as run 0 (below). A release reaches the Ryzen through
the Effector server's `update_agent`: a slot carries its own loader (351-UPD-0010), so an ABI or handoff change needs no
stick. What to look for in the first run's log: KRN's
`[INIT] display: RADEON 1002:1638 (DEVICE n); COMMAND 0006 BEFORE THE GRANT, 0006 AFTER; BAR 5 0x0+0x1B000 AND
0x58000+0x1000` and its VBIOS line; then `[DISPLAY] PCI 1002:1638 ... 108 KIB OF ITS REGISTERS GRANTED`, `THE SMU
MAILBOX: 4096 BYTES GRANTED`, `BAR 0 FOR A SECOND SCREEN: 20480 KIB GRANTED`, `VBIOS: VFCT ... AN IMAGE FOR 1002:1638 ...
ROM SIGNATURE 55AA`, `STEP 1 MAY READ 0 OF 7 BATCHES`, the seven `NOT READ` batches with their registers, `STEP 1 DONE:
0 OF 7 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN`, `NO REGISTER READ OR WRITTEN` and `COMMAND 0006 AFTER STEP 1`;
Step 3 then logs `STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: STEP 1'S BATCH 1 WAS NOT READ (STEP 3 NEEDS ALL SEVEN)` and its eight stages `NOT RUN` with every write they would make (64 lines, the longest 161 bytes), ending `STEP 3 DONE: 0 OF 8 STAGES RUN, NOTHING WRITTEN`. DP-2 keeps its picture. Every register of the seven batches and every one tables 12, 4 and 76 wrote on the model lies
below 0x1B000 (the highest is `RDPCSTX0_RDPCSTX_PHY_FUSE3` at 0x17848, which tables 12 and 76 write), so the engine's
window needs no widening for them.

## Progress: the ATOM interpreter, host-only (2026-11-21)

`display/src/atom.rs` runs ATOM command tables, bounded (each table's index, offset and length checked before it
runs; every fetch and jump inside the table; data reads inside the image; work, parameter and scratch indices inside
their spaces; registers only through `Reg::new` in direct mode; no PCI, system, indirect, PLL or MC access; at most
1,000,000 instructions, 8 nested calls, 5 s of delays). `tests/display_atom_host.rs`: 10 cases on synthetic images
(the opcodes, each bound) and one on the Ryzen's VBIOS, which skips with a line unless `MIND_AMD_VBIOS` names it.

With the image, on LEG9, tables 12, 4 and 76 run with first light's parameters to their end against a register model
that answers four statuses the tables wait for (PHY A's SRAM init done, the lanes' acknowledges, MPLLB on, the symbol
clocks on). 111, 5 and 23 writes, all on PHY A's path and OTG 1's pixel clock, none on the firmware's pipe 0 or PHY E,
none to the SMU or DISPCLK (hwdocs/cezanne-display.md, "What first light's VBIOS tables write"). Not run on the
machine; the answers are assumptions about the hardware.

## Progress: step 3 written and switched off (2026-11-21)

`display/src/light.rs` runs first light in eight stages; `STAGES_ALLOWED = 0` in `main.rs`, so this build makes no
writer at all and only logs each stage with every write it would make (hwdocs/cezanne-display.md, "Step 3 as code").
A stage runs only on what step 1 found on the same boot (`BATCHES_ALLOWED` at 7: step 3 touches no register step 1 has
not read), logs every write it will make before the first,
makes them through the guard (never the firmware's pipe, never off the route), runs its VBIOS tables with each table and
its parameters logged before it runs and each of its writes as it is made, within 1,000,000 instructions and 500 ms of
the table's own time (the log's time not counted), and reads the firmware's pipe after; a failure undoes it and every
stage before it and says what it undid. The mode is CTA 1080p60 (VIC 16, which HDMI-A-1's EDID lists), in DVI.

`tests/display_light_host.rs`, 18 cases on a register model: the pipe chosen from step 1's values, and refused when they
cannot tell it; the guard against every block of the firmware's pipe and off the route; the eight stages in the
route's order, never the firmware's pipe; a build stopping after its allowed stage; a stage that fails undone with every
one before it; a table writing the firmware's PHY refused and everything undone; a status that never comes ending by
the time limit and undone; more writes than the undo holds; the firmware's pipe changed under a stage; its status
fields moving by themselves failing no stage, while a setting beside them (`HUBP_VTG_SEL`) does; a build that undoes
what it ran (`UNDO_AFTER_STAGES`, off) leaving every register as before; the undo leaving PHY A's control register bus
as it is (no stale write into the PHY); the HUBP that follows step 3's OTG refused
unless blanked, read after stages 5 to 8, its statuses free to move, its leaving blank failing the stage; a build of 0
that only describes; the mode against the CTA table; the VFCT image; and the Ryzen's own tables (with `MIND_AMD_VBIOS`): all
eight stages with the four statuses answered, and the undo after stage 5 fails. Run on LEG9 only.

**What a person must see, stage by stage,** in the order they are let go (each a new boot; step 1's seven batches are
read on that boot first). Throughout, **DP-2 keeps MIND Core's screen with no flicker, blank or loss of signal**: if it
does any of these, that is the stage's failure whatever the log says, and the run stops there.

| Stage | On HDMI-A-1 | In the log, when it went through |
|---|---|---|
| 1 OTG clocks | dark, as before (no signal) | `S1 ... CLOCK_CONTROL AT ... SHOWED ITS STATUS IN n US` twice, `S1 OTG CLOCKS DONE; THE FIRMWARE'S PIPE UNCHANGED (n REGISTERS)` |
| 2 pixel clock | dark | `S2 TABLE 12 DONE: n STEPS, n CALLS, n US OF DELAYS, n WRITES`, `S2 PIXEL CLOCK DONE; ...` |
| 3 timing | dark | `S3 TIMING DONE; ...` |
| 4 test pattern | dark | `S4 TEST PATTERN DONE; ...` |
| 5 start | dark (the pipe runs, but nothing drives the PHY yet) | `S5 ... SHOWED ITS STATUS`, `S5 OTGn COUNTED n FRAMES IN n MS`, `S5 START DONE; ...` |
| 6 front end | dark | `S6 TABLE 4 DONE: ...`, `S6 FRONT END DONE; ...` |
| 7 back end | dark | `S7 BACK END DONE; ...` |
| 8 PHY | **the monitor wakes and shows the engine's colour squares**, a grid of coloured squares filling a 1920x1080 picture (its menu may say 1920x1080 at 60 Hz) | `S8 TABLE 76 DONE: ...`, `S8 PHY DONE; ...`, `STEP 3 STOPPED AFTER STAGE 8 OF 8 AS THIS BUILD ALLOWS` |

A stage that fails logs `Sn NAME FAILED: <why>; UNDOING STAGES n TO 1`, then each stage's `Un ... UNDONE: n WRITES
RESTORED`, then `STEP 3 FAILED AT STAGE n AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED`;
HDMI-A-1 stays dark. A stage that stops at the build's count leaves its writes in place; whether the next boot's
firmware clears them is not known. Step 3 starts from what step 1 reads either way: should a stopped run leave its OTG
running, the next boot refuses with `2 TIMING GENERATORS RUN, NOT ONE` (safe, but the next stage then needs a cold start).

**The carve-out and the frame pool** (for step 4's framebuffer, not step 3), from run 0's hardware report (hw0054):
the firmware's memory map ends with a **reserved** region 0x8_0E30_0000 to 0x8_2FFF_FFFF (541 MiB) below `TOP_MEM2`
0x8_3000_0000, the top of RAM, where a UMA carve-out sits: room for the 512 MiB carve-out from 0x8_1000_0000 with
29 MiB more below it. The frame pool, 31,982 MiB, is the conventional memory (31,987 MiB) less the kernel's own, so the
carve-out is not handed out. Confidence high that it is reserved. Its exact start, batch 6's `DCN_VM_FB_OFFSET`, reads
0x810 (in 16 MiB units): 0x8_1000_0000, inside that reserved region (run 7, below).

## Run 0 on the Ryzen (2026-11-21 18:26 UTC)

Release 202611211819, slot A confirmed (the owner session's bench build `claude/DRV-ryzen-bench-37` 609487f34: round
167's candidate, CAM's branch and this one at 6050f0169; `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0), put on the machine
through the Effector server. Every line expected came, none dropped (228 `[DISPLAY]` lines):
- `[INIT] display: RADEON 1002:1638 (DEVICE 24); COMMAND 0006 BEFORE THE GRANT, 0006 AFTER; BAR 5 0x0+0x1B000 AND
  0x58000+0x1000` and `VBIOS: THE VFCT TABLE, 55428 BYTES, READ-ONLY`;
- `PCI 1002:1638 (DEVICE 24), COMMAND 0006, BAR 5 AT FCB00000: 108 KIB OF ITS REGISTERS GRANTED`, `THE SMU MAILBOX:
  4096 BYTES GRANTED`, `BAR 0 FOR A SECOND SCREEN: 20480 KIB GRANTED`, `VBIOS: VFCT 55428 BYTES, AN IMAGE FOR 1002:1638
  OF 55296 BYTES, ROM SIGNATURE 55AA` (the image's length is the 55,296 bytes Linux read);
- `STEP 1 MAY READ 0 OF 7 BATCHES`, the seven batches `NOT READ` with their 147 registers (156 lines),
  `STEP 1 DONE: 0 OF 7 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN`;
- `STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: STEP 1'S BATCH 1 WAS NOT READ (STEP 3 NEEDS ALL SEVEN)` and its 64 lines,
  `STEP 3 DONE: 0 OF 8 STAGES RUN, NOTHING WRITTEN`;
- `NO REGISTER READ OR WRITTEN`, `COMMAND 0006 AFTER STEP 1`.

So KRN's grant is as asked (the command register stays 0006 through the grant and after), the VFCT copy holds the same
image Linux read, and this build reads and writes nothing. Run 0 is evidence for that machine and that build (MC-12.1).

## Step 1 on the Ryzen (2026-11-21, reads only, the owner session's word)

The owner session allowed reads only: `BATCHES_ALLOWED` 1, 3, 5 and 7, one release per value, each after the previous
run's log came in full, the machine answered and its values were as expected; `STAGES_ALLOWED` stays 0. Each release
is the owner's bench tree `claude/DRV-ryzen-bench-37` with only that constant changed, put on the machine through the
Effector server (`update_agent`); each run's whole boot log is kept outside the repository (it carries the Wi-Fi
agent's lines too).

**Run 1** (`BATCHES_ALLOWED` 1): the first release, 202611211837, never reached the machine (the bundle's download
broke over the Ryzen's Wi-Fi, `failed | upload`; nothing changed there). Retried as release 202611211902 (the bench tree at
3ac64624b, which also carries 213-DRV-0090's Wi-Fi diagnostics), slot B confirmed 19:09 UTC. 229 `[DISPLAY]` lines, none
dropped:
- `B1 HPD: READING 10 REGISTERS NEXT`, then the ten values: `DC_HPD_INT_STATUS` 00000012 in HPD blocks 0 and 3,
  00000000 in blocks 1, 2 and 4; `DC_HPD_CONTROL` 10FA09C4 in all five;
- `B1 HPD SENSE 1:1 2:0 3:0 4:1 5:0; ENABLED 1:1 2:1 3:1 4:1 5:1`: a monitor on HPD 1 (HDMI-A-1, PHY A) and on HPD 4
  (DP-2, PHY E, the firmware's screen), none on HPD 2, 3 or 5, as the survey's connector table expects; every block's
  detection enabled (the firmware leaves it so);
- `STEP 1 DONE: 1 OF 7 BATCHES READ, 10 REGISTERS, NOTHING WRITTEN`, step 3 shown for the survey's expectation
  (`STEP 1'S BATCH 2 WAS NOT READ`), `10 REGISTERS READ, NONE WRITTEN`, `COMMAND 0006 AFTER STEP 1`.

The machine kept answering after the reads. These values are evidence for that machine, that boot and that build
(MC-12.1).

**Run 3** (`BATCHES_ALLOWED` 3): release 202611211911, slot A confirmed 19:17 UTC; 237 `[DISPLAY]` lines, none
dropped.
- B1 the same ten values as run 1.
- B2, 76 registers: `B2 OTG0: RUNNING 1, CLOCK_ON 1, FROM OPP 0; 2560x1600 TOTAL 2720x1646 H+ V-`, which is DP-2's
  preferred timing (the survey's EDID table) and the loader's GOP mode. `OTG0_OTG_CONTROL` reads 80011301 and
  `OTG0_OTG_CLOCK_CONTROL` 00010101. `B2 OTG1`, `OTG2` and `OTG3`: `RUNNING 0, CLOCK_ON 0, FROM OPP n; NO TIMING IN ITS
  REGISTERS`; each `OTG_CONTROL` reads 80000300 and every timing register reads 0. So the firmware runs one OTG, OTG0
  from OPP0, as the survey expected.
- B3, 8 registers: every `DOMAINn_PG_STATUS` reads 10000000, so `PGFSM_PWR_STATUS` (bits 31:30) is 0, on, in all
  eight domains: `B3 PIPE n: HUBPn ON, DPPn ON` for n 0 to 3.
- `STEP 1 DONE: 3 OF 7 BATCHES READ, 94 REGISTERS, NOTHING WRITTEN`, `94 REGISTERS READ, NONE WRITTEN`,
  `COMMAND 0006 AFTER STEP 1`.

Two findings; neither stops the next run. The fields are decoded with Linux's `dcn_2_1_0_sh_mask.h`:
1. **The firmware leaves all four pipes powered**, not only the one it drives. The header names bit 28 of `PG_STATUS`
   `DOMAINn_DESIRED_PWR_STATE`; the power state proper (`PGFSM_PWR_STATUS`, bits 31:30) is 0, on, in each. So batch 4
   reads the HUBP registers of all four pipes (20), each in a domain batch 3 shows on. Step 3 counts a HUBP as the
   firmware's when it is powered and its `HUBP_VTG_SEL` names OTG0. Run 5 shows that only HUBP0 does (below), so the
   firmware's HUBPs are as expected: `HUBPS 1000`.
2. **`OPTC_INPUT_GLOBAL_CONTROL` reads 80000000 in ODM1 to ODM3 and 00000000 in ODM0.** Bit 31 is
   `OPTC_DOUBLE_BUFFER_PENDING`: an update is waiting in each ODM whose OTG never runs. It is a status that moves by
   itself, so step 3's watch now leaves it out of ODM0's copy (below). Step 3 writes `OPTC_INPUT_CLOCK_CONTROL`, not
   this register.

**Run 5** (`BATCHES_ALLOWED` 5): release 202611211919 booted slot B on trial. `display` read batches 1 to 5, and all
its lines are in the cable's log taken at 554 s of that boot (250 `[DISPLAY]` lines). The trial was not confirmed:
Wi-Fi never joined, so the server never heard the machine, the trial ran out at 600 s, and the machine restarted into
Linux at 19:35 UTC. `rz-release.sh` printed `LINUX AGAIN: MIND CORE DID NOT STAY UP`. The owner session started MIND
Core again on slot A, which is run 3's release. The failed join is the Wi-Fi agent's (the boot's first scan heard no
country); nothing in it is `display`'s.
- B1 to B3 the same values as run 3.
- B4, 20 registers:
  - `B4 HUBP0: SURFACE 00F4_00000000, FORMAT 8, PITCH 2561, ROTATION 0, BLANK 0, TTU_DISABLE 0`, with `DCHUBP_CNTL`
    000F0002: `HUBP_VTG_SEL` 0 (OTG0), not blanked, the XRQ's no-outstanding-request field F. The firmware's surface is
    at GPU address 0xF4_0000_0000, the carve-out's base (the survey's 0xF4_0000_0000 + X for BAR 0 offset X, at X = 0:
    the GOP's framebuffer at BAR 0's base), format 8 (32-bit ARGB, as under Linux).
  - HUBP1 to HUBP3: `SURFACE 0000_00000000, FORMAT 8, PITCH 1, ROTATION 0, BLANK 1`, with `DCHUBP_CNTL` 000F001B,
    000F002B and 000F003B. Each is blanked and in blank, and `HUBP_VTG_SEL` names its own OTG (1, 2, 3), none of which
    runs. So the firmware's HUBP is HUBP0 alone.
- B5, 13 registers:
  - `B5 DIGE FE: FROM OTG 0, SYMCLK_FE_ON 1`; DIG A to D front ends 00000000, clock off.
  - `B5 DIGE BE: ENABLED 1, MODE 0, FROM FE MASK 16, SYMCLK_BE_ON 1`: `DIG_BE_CNTL` 30001000, so front end E, mode 0,
    `DIG_HPD_SELECT` 3 (HPD 4, DP-2's).
  - DIG A, B and D back ends: `ENABLED 0, MODE 1, FROM FE MASK 0, SYMCLK_BE_ON 1` (00010000, 00000100).
- `STEP 1 DONE: 5 OF 7 BATCHES READ, 127 REGISTERS, NOTHING WRITTEN`, `127 REGISTERS READ, NONE WRITTEN`,
  `COMMAND 0006 AFTER STEP 1`.

So, from batches 1 to 5, step 3's assessment finds what `EXPECTED` holds:
- the firmware's OTG0 from OPP0, DIG front end E into PHY E's enabled back end, HUBP0;
- HPD 1 sensed;
- step 3's pipe OTG1, OPP1, DIG front end A into PHY A.

It still refuses until all seven batches are read on one boot. Five findings:
1. **The two mode encodings.** The firmware's DP back end reads `DIG_MODE` 0, the ATOM tables' encoding of DP
   (`atom_encode_mode_def`). The idle back ends read 1. Linux's `dcn10_get_dig_mode` decodes 1 as DP and 0 as none,
   which this running DP output contradicts. The ATOM encoding is the one that matches here. Step 3 writes 2 (DVI),
   which both encodings give the same meaning.
2. **The pitch convention.** HUBP0's `PITCH` field holds 0xA00 (2560). The decoder adds one, after Display Core's
   convention that the field holds the pitch less one, so it prints 2561 against the GOP's stride of 2560. Either the
   GOP writes the stride as it is, or that convention does not hold for this surface. This is open, and step 4 (a
   framebuffer) must settle it. Step 3 writes no HUBP.
3. **HUBP1 is attached to step 3's OTG1.** It is powered and blanked, and `HUBP_VTG_SEL` names OTG1, with surface 0
   and pitch field 0. When stage 5 starts OTG1, that blanked HUBP is attached to it. Step 3 neither writes it nor
   unblanks it, and the test pattern (stage 4) replaces the pixels in OPP1.
4. **Every back end's `SYMCLK_BE_ON` is 1**, the disabled ones too, while only front end E's clock is on.
5. **The watch's status masks.** HUBP0's XRQ field reads F, four bits wide, where step 3's watch masked only bit 16.
   A change in bits 17 to 19, or of `OPTC_DOUBLE_BUFFER_PENDING`, would have been taken for a change of the
   firmware's pipe, and the stage undone. The watch now masks whole status fields by the header's masks, and a host
   case shows that these fields moving fail no stage while `HUBP_VTG_SEL` changing does. This errs to the safe side
   either way: only stage 3 onward would have been undone, and nothing was ever written.

**What batches 6 and 7 are expected to show** (run 7):
- B6, the aperture:
  - `DCN_VM_FB_LOCATION_BASE` 0xF4_0000_0000, which Linux read from the memory hub and HUBP0's surface address agrees
    with;
  - `_TOP` 0xF4_1F00_0000 (its last 16 MiB of 512 MiB);
  - `DCN_VM_FB_OFFSET` 0x8_1000_0000, the carve-out's system address: 512 MiB below `TOP_MEM2` 0x8_3000_0000, inside
    the firmware's reserved region 0x8_0E30_0000 to 0x8_2FFF_FFFF;
  - the AGP window is not used by the GOP's surface and is not predicted (likely closed, bottom above top).

  Confidence: high for the base, medium for the offset.
- B7, the clocks:
  - OTG0's pixel rate from the DP DTO (`DP_DTO_ENABLE` 1), as a DP output takes it;
  - OTGs 1 to 3 with no DTO;
  - PHYPLL E's resync with `PHYPLL_PIXCLK_ENABLE` whatever DP needs and deep colour 0;
  - PHYPLLs A, B and D off;
  - `DENTIST_DISPCLK_CNTL` with equal write and read dividers (a settled DISPCLK) for at least 2560x1600's 268.5 MHz
    pixel clock.

  Confidence: medium. Linux's state is no guide to the firmware's clocks, and the survey left the source open.
- Either batch's values only describe what step 3 inherits. Step 3 stops on its own refusals if they differ, for
  example `2 TIMING GENERATORS RUN, NOT ONE`.

**Run 7** (`BATCHES_ALLOWED` 7): release 202611211944, slot B confirmed 19:48 UTC, with the country chosen beforehand
(the owner session's). The boot log taken right after the confirmation ended at 32.9 s, not yet written further, so
it was taken again whole: 260 `[DISPLAY]` lines, none dropped.
- B1 to B5: the same values as run 5.
- B6, 6 registers: `B6 APERTURE: FB F400000000..F41F000000 (TOP'S LAST 16 MIB), FB OFFSET 810000000, AGP 0..0 BASE
  0`, from `DCN_VM_FB_LOCATION_BASE` 0000F400, `_TOP` 0000F41F, `DCN_VM_FB_OFFSET` 00000810 and the three AGP
  registers 0.
  - The carve-out's GPU address is 0xF4_0000_0000 for 512 MiB, at system address **0x8_1000_0000**, as expected.
  - The AGP registers are all 0, not the closed window guessed. The header gives only their widths (24 bits, 16 MiB
    units). Whether 0..0 is an empty window or a 16 MiB one at GPU address 0 is not settled here. The firmware's
    surface does not use it.
- B7, 14 registers:
  - `B7 OTG0 PIXEL RATE: SOURCE 0, DP_DTO 1, PHYPLL SOURCE 0, PLL SOURCE 0`. OTGs 1 to 3: `DP_DTO 0`, each
    `OTGn_PIXEL_RATE_CNTL` 0000C000, which is `OTGn_DIO_FIFO_ERROR` 3, a status, in the three idle OTGs.
  - `B7 PHYPLLA` to `PHYPLLE`: `PIXCLK_ENABLE 0, DEEP COLOR 0`. Every resync register reads 0, PHY E's too: the DP
    output takes its clock from the DTO, not a PHY PLL resync.
  - `B7 DENTIST: DISPCLK WDIVIDER 20, RDIVIDER 20`. `DENTIST_DISPCLK_CNTL` reads 141F1414, so the DISPCLK and DPPCLK
    write dividers are both 0x14 and both changes done (`CHG_DONE`, `DPPCLK_CHG_DONE`). The frequency needs the
    DENTIST VCO, which is not read.
- `STEP 1 DONE: 7 OF 7 BATCHES READ, 147 REGISTERS, NOTHING WRITTEN`, then step 3 assessed from step 1 itself:
  - `S0 THE FIRMWARE'S PIPE (AS STEP 1 FOUND): OTG0 OPP0 DIG E FRONT END, PHY E, HUBPS 1000; NEVER WRITTEN, READ
    AFTER EACH STAGE (30 REGISTERS)`;
  - `S0 STEP 3'S PIPE: OTG1 OPP1 DIG A FRONT END, PHY A (HDMI-A-1), HPD 1; MODE 1920x1080 AT 148.5 MHZ IN DVI
    SIGNALLING (CTA VIC 16)`;
  - `STEP 3 DONE: 0 OF 8 STAGES RUN, NOTHING WRITTEN`, `147 REGISTERS READ, NONE WRITTEN`, `COMMAND 0006 AFTER
    STEP 1`.

**Step 1 as a whole.** Four runs on the Ryzen (1, 3, 5 and 7) read the engine's state without a hang. The register
values agree from run to run. Nothing was written, and the command register read 0006 after each.
- **What the survey expected and the reads confirm:**
  - one OTG runs, OTG0 from OPP0, at DP-2's preferred 2560x1600;
  - DIG front end E feeds PHY E's back end, the only one enabled, in DP mode on HPD 4;
  - HUBP0 scans the carve-out's base, 0xF4_0000_0000;
  - the carve-out is 512 MiB at 0x8_1000_0000;
  - HDMI-A-1's monitor is sensed on HPD 1;
  - OTG1, OPP1, DIG front end A and PHY A are free.

  Step 3's assessment from the reads equals `EXPECTED`, the pipe its host cases were written for.
- **What the survey did not expect:**
  - all four pipes are powered, with HUBP1 to HUBP3 blanked and each attached to its own idle OTG;
  - the `DIG_MODE` encoding is ATOM's (DP = 0), not that of Linux's `dcn10_get_dig_mode`;
  - HUBP0's pitch field reads 0xA00 against the GOP's stride of 2560 (the convention is open);
  - the AGP registers read 0;
  - the idle OTGs show `DIO_FIFO_ERROR` 3;
  - every back end's symbol clock is on.
- **One fix to step 3 came of it:** the watch masks whole status fields (run 5, finding 5).
- These values are evidence for this machine, this firmware and these boots only (MC-12.1, 12.9). The values on a
  cold boot, after a firmware update or with other monitors are not known.

**What it means for step 3's plan.** No stage's writes change.
- **None of the eight stages writes a power domain, a HUBP, a DPP or an MPCC.** OTG1, OPP1 and the DIG and PHY
  blocks are not in the HUBP and DPP domains that batch 3 reads. So the pipes left powered take nothing away from the
  plan and add no stage.
- What they change is around the plan:
  1. **HUBP1 is attached to OTG1.** When stage 5 starts OTG1, HUBP1 follows its timing, powered but blanked
     (`HUBP_BLANK_EN` 1, in blank), with surface address 0 and pitch field 0. Step 3 never unblanks it, and stage 4's
     test pattern in OPP1 replaces whatever the MPC passes. Display Core blanks and power-gates such a pipe at its
     start; step 3 does not, and it must not unblank HUBP1 at address 0. A HUBP1 that left blank on its own would
     not be seen today, because HUBP1 is outside the watch. Reading HUBP1's `DCHUBP_CNTL` after stages 5 to 8 would
     show it; that is not made.
  2. **The watch now masks the status fields**, so a good stage is not undone because HUBP0's request statuses
     moved.
  3. The firmware's `OTG0_PIXEL_RATE_CNTL` holds two status fields (`DIO_FIFO_ERROR`, `DIO_ERROR_COUNT`), which read
     0. They stay watched: an error on DP-2's output during a stage is the change that stage must not cause.
- **By stage:**
  - Stage 1 (OTG1's and ODM1's clocks) stands as written: OTG1 reads clock off and no timing.
  - Stage 2 (table 12, PHY A's PLL; PHYPLL A's resync to 8-bit colour) stands. OTG1 has no DTO, and the resync reads
    0 already. OTG1's DIO FIFO error bits are kept by any read-modify-write.
  - Stage 3 (the timing) stands. Every OTG1 timing register reads 0 and `OTG_MASTER_EN` is 0.
  - Stage 4 (OPP1's clock, the pattern) stands. OPP1's registers are not among step 1's, so the WROTE lines are the
    first to show their values.
  - Stage 5 (OTG1 started on OPP1) stands, with point 1 above. ODM1's source select already names OPP1 (0000F100).
  - Stages 6 to 8 (DIG A's front end, back end and PHY A, by tables 4 and 76) stand. DIG A's front end reads 0 with
    its clock off, and its back end 00010000.

**Recommendation for stage 1.** Run it, alone, in one release: `BATCHES_ALLOWED` 7, `STAGES_ALLOWED` 1.
- Preferably with `UNDO_AFTER_STAGES` true, so that the same boot also undoes it.
- **What it writes:** two read-modify-writes (`ODM1_OPTC_INPUT_CLOCK_CONTROL` bits 0 to 1, `OTG1_OTG_CLOCK_CONTROL`
  bits 0 to 1) and two bounded waits (1 ms each), on an OTG that runs nothing and drives no output. It runs no VBIOS
  table and touches no PHY or DIG. Its 30 watched registers are read before and after.
- **What a person sees:** nothing on either monitor.
- **The log lines that show it went through:**
  - `S1 OTG CLOCKS: STAGE 1 OF 8 NEXT; ITS WRITES:` and its `S1 WILL` lines;
  - `S1 WROTE ODM1_OPTC_INPUT_CLOCK_CONTROL ... (WAS ...)` and `S1 ... SHOWED ITS STATUS IN n US`, then the same for
    `OTG1_OTG_CLOCK_CONTROL`;
  - `S1 OTG CLOCKS DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`;
  - with the undo: `STEP 3 RAN 1 OF 8 STAGES (2 WRITES); UNDOING STAGES 1 TO 1`, two `U1 RESTORED` lines,
    `U1 OTG CLOCKS UNDONE: 2 WRITES RESTORED` and `STEP 3 UNDONE AFTER STAGE 1 OF 8 ...; THE FIRMWARE'S PIPE
    UNCHANGED`;
  - without the undo: `STEP 3 STOPPED AFTER STAGE 1 OF 8 AS THIS BUILD ALLOWS; 2 WRITES MADE ARE LEFT IN PLACE`.
- **Why the undo:**
  - it shows the undo path on the hardware with the smallest stage, before any later stage depends on it;
  - it leaves nothing across a boot, which is still unknown ground;
  - its restore order (OTG clock first, then the input clock) is the order in which Display Core turns them off, in
    `optc1_enable_optc_clock` as recalled (not among the files at hand).
- **Without the undo,** OTG1's and ODM1's clocks stay on until the next boot. That boot's batch 2 shows whether the
  firmware cleared OTG1's (`OTG_CLOCK_CONTROL`; ODM1's input clock is not among step 1's reads). It does not make
  OTG1 count as running: the assessment reads `OTG_CURRENT_MASTER_EN_STATE`.
- **If a wait times out,** the stage is undone and the log says `STEP 3 FAILED AT STAGE 1 AND WAS UNDONE`. If the
  machine hangs, the hang procedure applies, as for step 1's reads.
- Stage 2 should come in a release of its own, because it runs the first VBIOS table on the hardware.
- The decision is the owner session's. This build keeps `STAGES_ALLOWED` 0 and `UNDO_AFTER_STAGES` false.

**The attached HUBP, checked since.** `assess` now lists the powered HUBPs that are not the firmware's and whose
`HUBP_VTG_SEL` names step 3's OTG: on the Ryzen, HUBP1.
- It refuses `HUBPn FOLLOWS STEP 3'S TIMING GENERATOR AND IS NOT BLANKED` when one is not blanked.
- Their `DCHUBP_CNTL` is read at the start and after stages 5 to 8, through the same watch and status masks as the
  firmware's pipe. A change beyond the statuses fails the stage (`THE ATTACHED HUBP'S ... CHANGED`), which is undone
  with every stage before it.
- The log names it (`S0 HUBP1 FOLLOWS OTG1, POWERED AND BLANKED: NEVER WRITTEN, ITS DCHUBP_CNTL READ AFTER STAGES 5 TO
  8`) and says `Sn THE ATTACHED HUBPS UNCHANGED, BLANKED (1 REGISTERS)` after each of those stages.
- Nothing writes it. It is in place before stage 5 is asked for. It has not run on the Ryzen: stage 1's release
  predates it.

## Step 3's stage 1 on the Ryzen (2026-11-21 20:05 UTC, the owner session's word for stage 1 only)

Release 202611211959 (the owner's bench tree at 5c59b11a1, with this branch at 260aee691; `BATCHES_ALLOWED` 7,
`STAGES_ALLOWED` 1, `UNDO_AFTER_STAGES` true), slot A confirmed 20:05:15. The whole boot log has 270 `[DISPLAY]` lines,
none dropped:
- Step 1 as in run 7 (`STEP 1 DONE: 7 OF 7 BATCHES READ, 147 REGISTERS, NOTHING WRITTEN`), then step 3 assessed from it:
  - `S0 THE FIRMWARE'S PIPE (AS STEP 1 FOUND): OTG0 OPP0 DIG E FRONT END, PHY E, HUBPS 1000; NEVER WRITTEN, READ AFTER
    EACH STAGE (30 REGISTERS)`;
  - `S0 STEP 3'S PIPE: OTG1 OPP1 DIG A FRONT END, PHY A (HDMI-A-1), HPD 1; ...`;
  - `S0 THIS BUILD UNDOES THE STAGES IT RAN, LAST FIRST, BEFORE IT STOPS`.
- `S1 OTG CLOCKS: STAGE 1 OF 8 NEXT; ITS WRITES:` with its four `S1 WILL` lines and `S1 UNDO: ITS WRITES RESTORED, LAST
  FIRST`. Then, 1 s later:
  - `S1 WROTE ODM1_OPTC_INPUT_CLOCK_CONTROL AT 13E7C = 00000003 (WAS 00000000)`;
  - `S1 ODM1_OPTC_INPUT_CLOCK_CONTROL AT 13E7C SHOWED ITS STATUS IN 1 US`;
  - `S1 WROTE OTG1_OTG_CLOCK_CONTROL AT 14318 = 00000003 (WAS 00000000)`;
  - `S1 OTG1_OTG_CLOCK_CONTROL AT 14318 SHOWED ITS STATUS IN 5 US`;
  - `S1 OTG CLOCKS DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`.
- After the 3 s pause:
  - `STEP 3 RAN 1 OF 8 STAGES (2 WRITES); UNDOING STAGES 1 TO 1 AS THIS BUILD ASKS`;
  - `U1 RESTORED OTG1_OTG_CLOCK_CONTROL AT 14318 = 00000000`, then `U1 RESTORED ODM1_OPTC_INPUT_CLOCK_CONTROL AT 13E7C =
    00000000`;
  - `U1 OTG CLOCKS UNDONE: 2 WRITES RESTORED`;
  - `STEP 3 UNDONE AFTER STAGE 1 OF 8 AS THIS BUILD ASKS; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED`.
- Stages 2 to 8 `NOT RUN (THIS BUILD MAY RUN 1)`. Then `147 REGISTERS READ BY STEP 1, 2 WRITES BY STEP 3` (the count
  leaves out the two restores) and `COMMAND 0006 AFTER STEP 1`.
- The machine kept answering: its logs were taken through the server after the run.

So OTG1's and ODM1's clocks came on within microseconds of being asked for, and went back off with the undo.
- **DP-2, from the registers only.** Nobody watched the monitors. The firmware's 30 registers read the same before
  the stage, after it and after the undo, the status fields aside: OTG0's timing, control and clocks, ODM0, OTG0's
  pixel rate, DIG E's front and back end, PHYPLL E, and HUBP0's surface, format, pitch and control. So by its
  registers DP-2's pipe did not change. That it kept its picture is inferred from them, not seen.
- HDMI-A-1 was not driven: no PHY, DIG or VBIOS table was touched.
- This is evidence for this machine, this boot and this build (MC-12.1). Every boot of this release runs stage 1 and its
  undo again.

**Not met tonight: a person watching DP-2.** The rule above says that DP-2 keeping its screen is watched at every
stage, and that a flicker, blank or loss of signal is that stage's failure. Nobody was at the monitors on the night
of 2026-11-21. So:
- the dark stages (1 to 7) were let go only with `UNDO_AFTER_STAGES`, one more stage per release;
- "DP-2 kept its picture" is said from the firmware's registers only, each time in those words;
- stage 8, where HDMI-A-1's monitor wakes, was not let go.

The watched run (all stages, no undo, a person at both monitors) is the maintainer's.

**PHY A's control register bus and the undo (found before stage 2 ran).** Table 12 makes 17 data writes into PHY A
through `RDPCS_TX_CR_DATA` (and 20 to `RDPCS_TX_CR_ADDR`). A write to the data register is the write into the PHY.
The undo used to put both registers back, last first. That would have made 17 more writes of stale data into PHY A.
- Since 6ab7fd7a1 the undo leaves the two registers as they are and logs `U2 PHY A'S CONTROL REGISTER BUS LEFT AS IT
  IS: 20 ADDRESS AND 17 DATA WRITES; WHAT THEY SET INSIDE THE PHY STAYS`.
- So stage 2's undo cannot return PHY A's insides to their state before step 3. They keep the PLL setup table 12 gave
  them, while PHY A's outer registers go back and its power-down runs.
- Stage 8 (table 76) also writes through that bus.

**Does Linux run table 12 on this engine? Not on this machine: the DMCUB does.** The owner session let stage 2 go on
one condition: that Linux itself runs this table for an HDMI pixel clock on DCN 2.1, so that what it leaves inside PHY
A is a state this PHY is routinely in. The Linux v7.0 files at hand show the path, then a turn away from it:
- `dcn21_resource.c` makes each clock source with `dcn21_clock_source_create` → `dcn20_clk_src_construct`.
- `dcn20_hwseq.c` `dcn20_enable_stream_timing` calls `program_pix_clk` = `dce_clock_source.c` `dcn20_program_pix_clk`,
  then `dce112_program_pix_clk`, then the BIOS parser's `set_pixel_clock` = `command_table2.c` `set_pixel_clock_v7`
  (SetPixelClock revision 7, the Ryzen's 1.7).
- But `dcn21_resource.c` sets `debug.dmub_command_table = true`. With a DMUB service present, `set_pixel_clock_v7`
  calls `set_pixel_clock_dmcub`, which sends `DMUB_CMD__VBIOS` / `DMUB_CMD__VBIOS_SET_PIXEL_CLOCK` with the same
  `set_pixel_clock_parameter_v1_7`, and returns before `EXEC_BIOS_CMD_TABLE`. The encoder control (v1.5), transmitter
  control (v1.6, v1.7) and display power gating tables take the same turn.
- On this Ryzen, Linux loaded and started the DMCUB: `Loading DMUB firmware via PSP: version=0x0101002B` and `DMUB
  hardware initialized` (dmesg at 6.50 s and 6.61 s).

So under Linux here, the host never ran table 12's bytecode. The DMCUB's firmware programmed the pixel clock, and what
it writes into PHY A is not visible from these files. The condition does not hold. Stage 2 was not run, and the night
release (`STAGES_ALLOWED` 0) followed.
- What this means for the route: step 3 runs the VBIOS's own tables 12, 4 and 76 on the host. On DCN 2.1 that is the
  path Linux takes only when no DMUB service is present.
- Whether the bytecode is sound to run from the host on this engine is not shown by Linux's practice here. Neither is
  what the DMCUB does in its place.
- The GOP is another runner of these tables. It lights DP-2 before any OS, with its own code; whether that code is the
  same bytecode is not known.
- **Linux's own fallback.** `set_pixel_clock_v7` and `transmitter_control_v1_6` go to the DMCUB only `if (dmub_srv &&
  debug.dmub_command_table)`. Otherwise they run `EXEC_BIOS_CMD_TABLE(setpixelclock, ...)` and `(dig1transmittercontrol,
  ...)` on the host with the same parameter structs. This is so in the v7.0 file at hand, and the owner session found
  the same in v5.6 and v5.10. So host execution of tables 12 and 76 is a supported path of Linux on this engine when the
  DMCUB's firmware is not there.
  - Its limits: this board has not taken that path under Linux.
  - What the DMCUB writes into the PHY in its place is not visible.

**Step 1's batch 8: does the DMCUB run under MIND Core?** This read-only question now decides the design.
- If the firmware left the DMCUB running and it owns the PHY and the pixel clocks, host tables may fight it. The right
  path would then be to ask it, as Linux does.
- If it is held in reset, the host tables are the only way, and the GOP must itself have used them or its own code.

Batch 8 (0ba209bf1) reads 31 registers, all in segment 2 (base 0x34C0), so bytes 0xD668 and 0x19CE0 to 0x19D9C, inside
the granted [0, 0x1B000) (`dcn_2_1_0_offset.h`; fields from `dcn_2_1_0_sh_mask.h`):

| Register | Dword | Byte | What it tells |
|---|---|---|---|
| `CC_DC_PIPE_DIS` | 0xCA | 0x0D668 | `DC_DMCUB_ENABLE` (bit 16): the board has a DMCUB (`dmub_dcn20_is_supported`) |
| `DMCUB_CNTL` | 0x32A0 | 0x19D80 | `DMCUB_ENABLE` (16), `DMCUB_SOFT_RESET` (17), `DMCUB_PWAIT_MODE_STATUS` (20): enabled, held in reset, waiting (`dmub_dcn20_is_hw_init`, `_reset`) |
| `DMCUB_SEC_CNTL` | 0x3278 | 0x19CE0 | `DMCUB_SEC_RESET` (16) and `DMCUB_SEC_RESET_STATUS` (21) |
| `DMCUB_SCRATCH0`..`15` | 0x328D..0x329C | 0x19D34..0x19D70 | 0: the firmware's boot status; 7: its answer to a GPINT; 10: read by `dmub_dcn21_is_phy_init` |
| `DMCUB_INBOX0_WPTR`, `_RPTR` | 0x327C, 0x327D | 0x19CF0, 0x19CF4 | the inbox 0 pointers |
| `DMCUB_INBOX1_BASE_ADDRESS`, `_SIZE`, `_WPTR`, `_RPTR` | 0x327E..0x3281 | 0x19CF8..0x19D04 | the command ring Linux writes, and how far the DMCUB has read |
| `DMCUB_OUTBOX1_WPTR`, `_RPTR` | 0x3288, 0x3289 | 0x19D20, 0x19D24 | its answers |
| `DMCUB_GPINT_DATAIN1`, `_DATAOUT` | 0x32A2, 0x32A3 | 0x19D88, 0x19D8C | the GPINT mailbox |
| `DMCUB_TIMER_CURRENT` | 0x32A7 | 0x19D9C | read first and last in the batch: counting means its clock runs |

- The roles of the scratch registers are as recalled from Linux's `dmub_dcn20.c` and `dmub_dcn21.c`, which are not
  among the files at hand. The fields are the header's.
- The batch decodes into three lines:
  - `B8 DMCUB: ON THE BOARD n, ENABLED n, SOFT RESET n, PWAIT n; SECURE RESET n, ITS STATUS n; TIMER COUNTS|STILL (v)`;
  - `B8 DMCUB: SCRATCH0 (BOOT STATUS) ..., SCRATCH7 (GPINT ANSWER) ..., SCRATCH10 ..., GPINT IN ... OUT ...`;
  - `B8 DMCUB: INBOX1 AT ... OF n BYTES, READ n WRITE n; INBOX0 ...; OUTBOX1 ...`.
- Step 3 now needs all eight batches. It refuses with `THE DMCUB IS ENABLED AND NOT IN RESET` while `DMCUB_CNTL`
  shows it enabled and out of reset.
- Host cases: the registers and offsets against the header; a running DMCUB (enabled, counting, a ring set up) and
  one held in reset; the refusal, and its absence when held in reset.

## The watched run, for the maintainer (planned for 2026-11-22)

**First, batch 8.** If it shows the DMCUB enabled and out of reset, step 3 refuses by itself (`THE DMCUB IS ENABLED
AND NOT IN RESET`). Then nothing below runs, and the route must ask the DMCUB as Linux does. If it is held in reset,
the plan below holds.

**The release.** One release with `BATCHES_ALLOWED` 8, `STAGES_ALLOWED` 8 and `UNDO_AFTER_STAGES` false, through
`S/rz-release.sh`.
- It takes 6 to 8 minutes from the start to the confirmed trial: about 1 minute to build, bundle and upload, then 4 to
  6 minutes for the download over the Ryzen's Wi-Fi, the restart through Linux and MIND Core's start.
- Times below are seconds on MIND Core's boot clock, the log's `[ n.nnn ]`. That clock starts about half a minute
  after Linux's `boot_switch`.
- Step 1 ends at about 45 s. Each stage is then logged, paused 1 s, run, and paused 3 s, so the stages fall roughly
  at these times.

| Stage | About | DP-2 (MIND Core's screen) | HDMI-A-1 |
|---|---|---|---|
| 1 OTG clocks | 46 s | unchanged: no flicker, blank or loss of signal | dark (no signal / asleep) |
| 2 pixel clock (table 12) | 50-53 s | unchanged | dark |
| 3 timing | 57 s | unchanged | dark |
| 4 test pattern | 61 s | unchanged | dark |
| 5 start (OTG1 runs) | 65 s | unchanged | dark: nothing drives PHY A yet |
| 6 front end (table 4) | 69 s | unchanged | dark |
| 7 back end | 73 s | unchanged | dark |
| 8 PHY (table 76) | 77-80 s | unchanged | **wakes: the engine's colour squares, 1920x1080, its menu showing 1920x1080 at 60 Hz** |

- Any flicker, blank or loss of signal on DP-2 is the stage's failure, whatever the log says. Note the time, so the
  log's stage can be matched to it. The log lines that show each stage went through are in the table under "What a
  person must see".
- Without the undo, everything stays as written until the next boot. Any release afterwards (the night one,
  `STAGES_ALLOWED` 0) starts from a restart.

**The one thing that cannot be undone, and the maintainer decides whether stage 2 runs.**
- **What.** Table 12 (stage 2) writes into PHY A's internal registers through its control register bus
  (`RDPCS_TX_CR_ADDR`/`_DATA`). On the model with the Ryzen's VBIOS that is 20 address and 17 data writes out of its
  111. No restore can return those internal registers. The undo leaves the bus alone (6ab7fd7a1), so that it does not
  write stale data into the PHY.
- **Table 76 (stage 8)** makes no write through the bus on the model: 23 writes, none to the bus. The hardware may take
  another path through the table; the log shows every write it makes.
- **Why we think it is harmless:**
  - it is the vendor's own routine for the purpose (the VBIOS's SetPixelClock);
  - its parameters are for a standard CTA mode (1920x1080 at 148.5 MHz, DVI signalling, PHY A's PLL);
  - PHY A stays off until stage 8;
  - running these tables on the host is Linux's own fallback on this engine when no DMCUB is present
    (`set_pixel_clock_v7`, `transmitter_control_v1_6`).
- **What is not known:**
  - whether the firmware resets PHY A's internals at the next boot;
  - what the DMCUB would have written instead, since under Linux it, not the host, ran these tables on this board;
  - whether the bytecode run from the host does on this hardware what the model shows.
- **A gap in stage 8's undo,** which only matters if stage 8 fails. On the model, table 76's DISABLE writes
  `DP0_DP_LINK_CNTL` (DIG A's DP block, dword 0x2108), which the guard refuses as off the route. So the off call stops
  there, and the undo restores stage 8's writes without the VBIOS's own disable having run to its end. Allowing that
  register for the off call only is a change to make before stage 8 is asked for with the undo.

## Related

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md), [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md), [213-KRN-0098](../issues-done/213-KRN-0098-black-screen-when-the-shell-takes-it.done).
