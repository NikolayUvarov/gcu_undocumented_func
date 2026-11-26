# 213-DRV-0112 — `display`: a monitor on the Ryzen's other HDMI socket (HDMI-A-2)

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-26) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.5

## Problem

The Ryzen has two HDMI sockets and a USB-C one that carries the first display. `display` drives only one of the HDMI
sockets: HDMI-A-1. A monitor plugged into the other gets no picture.

**The maintainer's report, 2026-11-26, boot 107:** «переключил несколько раз и попробовал перейти на hdmi1, он не
запустился», and then, asked which socket that was: «да, на Ryzen два HDMI порта и один type-c с дисплеем».

**What the log shows of it.** At 2050.628 s `STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR`, and then no line of `display`
for 19 s, until the cable was back in the first socket (2069.618 s, lit again at 2071.802 s). `display` follows HPD 1
alone, so it never saw the monitor on the other socket.

**Seen in the same seconds, not explained:** at 2054.3 s the hub on xHCI 1's port 1 (05E3:0608, with the mouse's
receiver and the serial converter behind it) went away and came back. Whether the hub is in that monitor is asked of
the maintainer.

## What is known

From the survey ([213-DRV-0085](../issues-done/213-DRV-0085-the-radeons-display-engine-surveyed.done),
`hwdocs/cezanne-display.md`, "The display paths"):

| | HDMI-A-1, driven today | HDMI-A-2, this task |
|---|---|---|
| VBIOS path | 0 | 1 |
| Connector | `330C`, HDMI type A, enum 3 | `340C`, HDMI type A, enum 4 |
| Encoder, PHY | `211E`, UNIPHY enum 1: PHY A | `221E`, UNIPHY enum 2: PHY B |
| DDC | line 0, `DC_GPIO_DDC1` | line 1, `DC_GPIO_DDC2` |
| HPD | pin 1: `DC_GPIO_HPD_A` bit 0, HPD 1 | pin 2: bit 8, HPD 2 |

- Linux names them in that order; which printed label each socket carries is not recorded. The maintainer calls the
  undriven one "hdmi1".
- Nothing of PHY B, of DIG back end B, of DDC2 or of HPD 2 has been read on this machine by MIND Core. What the
  firmware leaves there is not known.
- There is one spare pipe: the firmware's picture takes one, the second screen the other. So one HDMI socket at a time.

## Plan

1. **Read first.** Step 1's batches read, and never write, what steps 2 to 4 need of the second socket: HPD 2's sense
   and control, DDC2's lines and the I2C engine's DDC2 registers, PHY B and DIG back end B as the firmware left them.
   A run on the Ryzen with the monitor in the second socket records the values here before any write is designed on
   them.
2. **The output as a parameter.** `HDMI_A_1` is fixed in the choice of the pipe, in the EDID read (`ddc.rs`), in the
   follower of the plug (`start.rs`), in the guard of what may be written, and in the tables' parameters. Each takes
   the output, with host cases for both.
3. **Which socket.** `display` follows both senses and lights the socket that has a monitor. With a monitor in each,
   the first lit stays; the other is named in the log as not driven, with the reason (one spare pipe).
4. **The guard.** The registers of the second socket's PHY, back end, DDC and HPD join what may be written only for
   that output; the firmware's pipe and the other socket's blocks stay refused. A host case for each refusal.
5. **On the Ryzen, in stages as HDMI-A-1 was:** the read-only run; the EDID read; first light with the test pattern;
   MIND Core's picture; the plug taken out and put back; the monitor moved from one socket to the other.

## Acceptance criteria

- [ ] Step 1 reads the second socket's blocks, with host cases that it writes nothing; its values from the Ryzen
  recorded here.
- [ ] The EDID read over DDC2, with host cases on the model of the engine; on the Ryzen its checksums good and the
  engine left as found.
- [x] Host cases: the stages on a model with the monitor on the second socket; the guard's refusals for the first
  socket's blocks then; a monitor in each socket.
- [ ] On the Ryzen: a monitor in the second socket lit in its own mode with the mirror on it, stage 15's watch clean,
  the firmware's pipe unchanged, and the maintainer's look (MC-12.1).
- [ ] On the Ryzen: the monitor moved from one socket to the other and back, lit each time.
- [ ] HDMI-A-1 alone behaves as before: its host cases pass unchanged and its lines on the Ryzen are the same.

## Progress (2026-11-26): built and host-tested on `claude/213-DRV-0112-display-hdmi-a-2`; nothing run on the Ryzen

Plan steps 1 to 4 are built, with a build switch for step 5's stages. Host cases pass; that is not a proof (MC-12.2),
and no value of the second socket has been read on the machine yet.

**What is done, and where.**
- **Step 1 reads the second socket** (`display/src/batches.rs`, `HDMI_A_2`, `read_second`): batch 12, after the eleven,
  at start and whenever a plug runs step 1 again. It is read whole in either order and never written (`Bus` has no
  write). Its values are summed up in notes, so the production order logs them too. HDMI-A-1's eleven batches and their
  lines are unchanged.
- **The output as a parameter:**
  - `light.rs`: `assess_for`, `prepare_on`, `HDMI_A_2`, `SOCKETS`, `name`, `other`, `expected`; the refusals
    `HdmiDriven(Output)` and `NoMonitor(Output)`; `DDC2_WRITES`, `ddc_writable_on`.
  - `ddc.rs`: `Line`, `DDC1`, `DDC2`, `read_on`, `On`.
  - `picture.rs`: `Lines::on`. `start.rs`: steps 2 and 3 take the output; `fallback`.
  - The tables' parameters come from the choice's output (`dcn::first_light`, `Light::off_call`), as before.
- **Which socket** (`start.rs`, `choose`, `Follower`; `hpd.rs`, `Settle`, `Pair`):
  - Steps 2 to 4 take the socket whose HPD senses a monitor. With a monitor in each, HDMI-A-1 is taken first and
    HDMI-A-2 is named as not driven, with the reason (one spare pipe).
  - After them both senses are followed. The lit socket's unplug takes it back, and the other socket lights if its
    monitor is in. A plug while none is lit lights its socket; a plug into the socket not lit is named.
  - HDMI-A-2's plug or unplug while it is not driven reads batch 12 again.
- **The guard** (`light.rs`, `socket_blocks`, `Guard`) refuses the other socket's blocks whatever the pipe: its DIG block,
  DP link registers, UNIPHY, DPCS/RDPCS, PLL resync, SYMCLK, HPD block and DDC line. Only the back end's three
  registers are refused when the chosen front end is on that DIG. The firmware's blocks are refused first.
- **Staged enabling** (`setting.rs`, `HdmiA2`, `Bringup::hdmi_a_2`, `Plan::takes`, `edid_on`, `stages_on`; `main.rs`,
  `HDMI_A_2`): committed `Read`, so a release only reads HDMI-A-2.
- **The front end** for HDMI-A-2 is DIG B, its own letter, as Display Core pairs a link with its own DIG. For HDMI-A-1
  the rule gives front end A, as before.

**Host cases added.**
- `display_batches_host`: batch 12's offsets and reads.
- `display_ddc_host`: DDC2's read on the engine model, each line hearing only its own monitor, the line guards.
- `display_hpd_host`: `Pair` with the monitor only in socket 2, in both, moved and back, a bounce, and `Follow`'s own
  traces.
- `display_picture_host`: the picture's name.
- `display_light_host`:
  - HDMI-A-2's pipe, refusals, table parameters and guard;
  - all 16 stages on HDMI-A-2 from step 1 on the model, taken back whole;
  - every level with the monitor in socket 2 alone; a monitor in each; moved and back;
  - the guard's refusals for each output;
  - the switch.
- One case runs the Ryzen's own tables on PHY B when `MIND_AMD_VBIOS` names its image, and skips itself without it. It
  skipped here.
- Two existing patterns changed shape, not meaning: `Refusal::HdmiDriven` and `Refusal::NoMonitor` now carry
  `HDMI_A_1`. The helper `bringups()` also walks the switch's four levels.

**The second socket's registers.** Byte offsets in BAR 5 are (segment base + dword) x 4, with segments 0xC0 (DCCG) and
0x34C0. Sources: Linux v7.0's `dcn_2_1_0_offset.h`, `dcn_2_1_0_sh_mask.h` and `dpcs_2_1_0_offset.h` (the survey's copies,
the same SHA-256). For DDC2's engine index: `dce_i2c_hw.c`, `dce_i2c_hw.h` and `dcn21_resource.c`; for the pads:
`ddc_regs.h`, `hw_factory_dcn21.c` and `hw_ddc.c` (`hwdocs/cezanne-display.md`, "HDMI-A-2").

| Register | Dword | BAR byte | Batch 12 reads | Written, and when |
|---|---|---|---|---|
| `HPD1_DC_HPD_INT_STATUS` (HPD 2's sense) | 2: 0x1F1C | 0x14F70 | yes; the follower polls it | never |
| `HPD1_DC_HPD_CONTROL` | 2: 0x1F1E | 0x14F78 | yes | never |
| `DIG1_DIG_FE_CNTL` (front end B) | 2: 0x2168 | 0x158A0 | yes (batch 5 too) | stages 6 and 8, when HDMI-A-2 is chosen with front end B |
| `DIG1_DIG_BE_CNTL` (back end B) | 2: 0x21AF | 0x159BC | yes (batch 5 too) | stage 7 and table 76, HDMI-A-2 only |
| `DIG1_DIG_BE_EN_CNTL` | 2: 0x21B0 | 0x159C0 | yes (batch 5 too) | table 76, HDMI-A-2 only |
| `PHYPLLB_PIXCLK_RESYNC_CNTL` | 1: 0x41 | 0x00404 | yes (batch 7 too) | stage 2 and table 12, HDMI-A-2 only |
| `DC_I2C_DDC2_HW_STATUS` | 2: 0x1E9D | 0x14D74 | yes | never |
| `DC_I2C_DDC2_SPEED`, `_SETUP` | 2: 0x1EA4, 0x1EA5 | 0x14D90, 0x14D94 | yes | step 2 on DDC2, from level Edid; put back as found |
| `DC_GPIO_DDC2_MASK` | 2: 0x28D4 | 0x17650 | yes | step 2 on DDC2, from level Edid; put back as found |
| `DC_GPIO_DDC2_A`, `_EN`, `_Y` | 2: 0x28D5 to 0x28D7 | 0x17654 to 0x1765C | yes | never |
| `DC_GPIO_AUX_CTRL_5` (pads' I2C mode, bits 12 and 13) | 2: 0x291D | 0x17774 | yes | never (open question 2) |
| `PHY_AUX_CNTL` (AUX1/AUX2 RXSEL, bits 10-11 and 12-13) | 2: 0x28FF | 0x176FC | yes | never (open question 2) |
| `DIG1_DIG_CLOCK_PATTERN`, `DIG1_HDMI_CONTROL`, `DIG1_TMDS_CTL_BITS`, AFMT | 2: 0x216B, 0x2171, 0x21DA | 0x158AC, 0x158C4, 0x15A68 | no | table 4 and stage 6, with front end B |
| `DIG1_DIG_LANE_ENABLE` | 2: 0x21E1 | 0x15A84 | no | table 76 (by the guard; the tables' run on the image is open) |
| `DP1_DP_LINK_CNTL`, `DP1_DP_DPHY_TRAINING_PATTERN_SEL` | 2: 0x2208, 0x2218 | 0x15B20, 0x15B60 | no | table 76's DISABLE (stage 8's undo) |
| `UNIPHYB_LINK_CNTL`, `UNIPHYB_CHANNEL_XBAR_CNTL` | 2: 0x286F, 0x2870 | 0x174BC, 0x174C0 | no | table 76 |
| `DPCSTX1`, `RDPCSTX1` (PHY B's DPCS/RDPCS; `RDPCS_TX_CR_ADDR`/`_DATA` 0x2A0C/0x2A0D, `PHY_CNTL0` 0x2A18) | 2: 0x2A00 to 0x2AD7 | 0x17B00 to 0x17E5C | no (the survey's rule) | tables 12 and 76; the undo leaves the bus as it is, as for PHY A |
| `SYMCLKB_CLOCK_ENABLE` | 1: 0xA1 | 0x00584 | no | table 12 |

The DDC2 fields lie where DDC1's do. `DC_I2C_DDC_SELECT` is 1 for DDC2: its engine index in Display Core.

**What the read-only run logs of the second socket** (any build of this branch; the switch at its committed `Read`).
`grep -E 'B12 |HDMI-A-2|HPD 2'` finds them all.
- At each start, and at each plug or unplug of socket 2 while it is not driven, both orders log:
  ```
  B12 HDMI-A-2 HPD 2 (HPD1_): SENSE s, ENABLED e; INT_STATUS xxxxxxxx, CONTROL xxxxxxxx
  B12 HDMI-A-2 DIG B: FE FROM OTG n, SYMCLK_FE_ON n (xxxxxxxx); BE ENABLED n, MODE n, FROM FE MASK n, SYMCLK_BE_ON n (xxxxxxxx, xxxxxxxx)
  B12 HDMI-A-2 PHYPLLB: PIXCLK_ENABLE n, DEEP COLOR n (xxxxxxxx)
  B12 HDMI-A-2 DDC2: HW STATUS n (xxxxxxxx); ENABLED n, TIME LIMIT n, PRESCALE n, THRESHOLD n (SPEED xxxxxxxx, SETUP xxxxxxxx)
  B12 HDMI-A-2 DDC2 PADS: AUX MODE n; CLOCK MASK n EN n A n Y n; DATA MASK n EN n A n Y n (MASK xxxxxxxx)
  B12 THE DDC PADS' I2C MODE: DDC1 n DDC2 n; AUX PAD RXSEL 1:n 2:n (DC_GPIO_AUX_CTRL_5 xxxxxxxx, PHY_AUX_CNTL xxxxxxxx)
  B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN
  ```
- The survey's order also logs `B12 HDMI-A-2: READING 15 REGISTERS NEXT, NEVER WRITTEN (213-DRV-0112)` first, and then
  each register as `B12 <NAME> AT <BYTE> = <VALUE>`.
- A register the grant does not map is named: `B12 <NAME> AT <BYTE>: NOT MAPPED, NOT READ`.
- The choice and the follower log, as the case is:
  ```
  HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE; THIS BUILD GOES NO FURTHER THAN ITS BLOCKS READ ONLY (HDMI_A_2 READ)
  HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND THERE IS ONE SPARE PIPE
  HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 READ): SENSE s; THE SECOND OUTPUT <Lit|Dark|Stopped> ON HDMI-A-1
  HPD 2 IS NOT ENABLED: ITS SENSE IS NOT FOLLOWED
  HDMI-A-2 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-1 IS LIT, AND THERE IS ONE SPARE PIPE
  HDMI-A-2: ITS MONITOR TAKEN OUT, NOT DRIVEN
  HDMI-A-1: ITS MONITOR TAKEN OUT, NOT DRIVEN        (only while HDMI-A-2 is lit)
  ```
- Under the `read` setting, HDMI-A-2's monitor alone is taken to be described, and nothing is written:
  `HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 READ: ITS BLOCKS READ ONLY`, then
  `S0 STEP 3'S PIPE: OTG1 OPP1 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; ...`. In the survey's order every `WOULD` line
  of the 16 stages on PHY B follows.

**How each level is selected.** Each level is a value of the bring-up switch in `display/src/main.rs`, set uncommitted on
the bench tree as the other switches are:

```rust
const HDMI_A_2: setting::HdmiA2 = setting::HdmiA2::Read;
```

- **`Read`**, the committed value (a release): HDMI-A-2's blocks are read only. It is never taken under `light`; no DDC2
  transaction and no write of its blocks.
- **`Edid`**: HDMI-A-2 is taken. Its EDID is read over DDC2, the engine given back as found, and its 16 stages are only
  described. Nothing of the display pipe is written.
- **`FirstLight`**: as `Edid`, then first light's 8 stages on PHY B, at most `STAGES_ALLOWED`.
- **`Full`**: all 16 stages and the second screen offered, at most `STAGES_ALLOWED`.

The other switches combine with it:
- `STAGES_ALLOWED` bounds both outputs.
- `UNDO_AFTER_STAGES` undoes the stages before display idles.
- `SURVEY_ORDER` logs every value, write and plan, with pauses.

A level other than `Read` makes a bring-up build, whose first line says so, for example
`A BRING-UP BUILD, NOT A RELEASE: HDMI_A_2 EDID; ONLY WHAT LIGHT DOES CHANGES`. The setting stays `light`.

**The bench procedure, stage by stage.** Each run is on the Ryzen with its two monitors; the logs are the owner
session's.
0. **Before any write, on LEG9.** Run the PHY B case on the Ryzen's image:
   `MIND_AMD_VBIOS=<image> <display_light-tests> the_ryzens_own_tables_on_phy_b`. It shows what tables 12, 4 and 76
   write with PHY B's parameters, and whether all of it lies on HDMI-A-2's route.
1. **The read-only run** (`Read`; `light`; the production order will do, `SURVEY_ORDER` gives each value too).
   - Boot with the monitor in socket 2 only.
     - Expected: the B12 notes, `HDMI-A-2 NOT DRIVEN: ... (HDMI_A_2 READ)`, `STEP 3 NOT RUN: HPD 1 SENSES NO MONITOR ON
       HDMI-A-1`, and `HPD 2 FOLLOWED TOO ...`.
     - Then move the cable to socket 1 (HDMI-A-1 lights as before) and back to socket 2.
     - Expected: `STEP 3 TAKEN BACK: HPD 1 ...`, then `HDMI-A-2 NOT DRIVEN: ...`, with fresh B12 notes at the plug.
   - Then boot with a monitor in each socket, if a second one is at hand.
   - What to look for:
     - `SENSE 1` on HPD 2 while the monitor is in the socket the maintainer calls "hdmi1": which label is HDMI-A-2;
     - `BE ENABLED 0` (the firmware does not drive PHY B);
     - `DDC2: HW STATUS 0` (the engine not held);
     - the DDC pads' I2C mode bits;
     - `PHYPLLB PIXCLK_ENABLE`;
     - that `B12 HDMI-A-2 DONE` comes at all (no read hung).
   - The values are recorded here.
2. **The EDID** (`Edid`), monitor in socket 2 only.
   - Expected:
     - `HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 EDID: ...`;
     - `D STEP 2 DONE: 2 BLOCKS READ (256 BYTES) IN ... US, ...; n WRITES PUT BACK, 6 OF 6 REGISTERS AS BATCHES 11 AND 12 READ THEM`;
     - `D EDID ..., FNV-1A ...`, `D PREFERRED: ...` and `STEP 3'S MODE: ...`;
     - `STEP 3 DONE: 0 OF 16 STAGES RUN, NOTHING WRITTEN`.
   - A failure names itself, for example `D STEP 2 NOT DONE: BLOCK 0: NO ACKNOWLEDGE ...`, and gives the engine back.
3. **First light** (`FirstLight`, with `SURVEY_ORDER` and, on the first boot, `UNDO_AFTER_STAGES`). Optionally step
   `STAGES_ALLOWED` from 1 to 8 as 213-DRV-0086 did for PHY A. Monitor in socket 2 only.
   - Expected:
     - `STEP 3: FIRST LIGHT ON HDMI-A-2 IN 8 STAGES, ...`;
     - `S0 STEP 3'S PIPE: OTG1 OPP1 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; ...`;
     - each `S<n> <NAME> DONE; THE FIRMWARE'S PIPE UNCHANGED (n REGISTERS)`;
     - `STEP 3 STOPPED AFTER STAGE 8 OF 16 AS THIS BUILD ALLOWS`.
   - The maintainer's look: the engine's colour squares on the monitor in socket 2, DP-2 unchanged.
4. **MIND Core's picture** (`Full`).
   - Expected:
     - `S10 ITS TEXT: MIND CORE / DISPLAY / HDMI-A-2 ...`;
     - `S15 UNBLANK DONE ..., WATCHED 60 FRAMES IN ... CLEAN ...`;
     - `S16 OFFER DONE ...`, `THE SECOND SCREEN OFFERED ...`;
     - every ten minutes `S15 STANDING CHECK: ... NOTHING NEW ON EITHER PIPE`.
   - The maintainer's look: the mirror on socket 2's monitor, in its own mode.
5. **The plug out and in** (`Full`).
   - Expected at the unplug: `STEP 3 TAKEN BACK: HPD 2 SENSES NO MONITOR; UNDOING STAGES 16 TO 1 (...)`, then
     `STEP 3 TAKEN BACK IN ... MS: THE SECOND PIPE IS OFF, ITS MARK CLEARED; THE FIRMWARE'S PIPE UNCHANGED`.
   - Expected at the plug: `A MONITOR ON HDMI-A-2: STEPS 1 TO 4 RAN AGAIN ON IT, THE SECOND OUTPUT Lit`.
6. **The monitor moved** (`Full`): from socket 1 to 2 and back.
   - Expected: the take-back lines, `A MONITOR ON HDMI-A-2: ...`, then
     `A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT Lit`.
   - With a monitor in each socket:
     - at start: `HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; ...`;
     - while HDMI-A-2 is lit: `HDMI-A-1 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-2 IS LIT, AND THERE IS ONE SPARE PIPE`;
     - HDMI-A-2 lit once HDMI-A-1's monitor is out.

**Open questions, and what was not written for lack of evidence.**
1. **Which printed label each socket carries.** The read-only run's HPD 2 sense tells.
2. **DDC pad 2's I2C mode.** Display Core sets `DC_GPIO_AUX_CTRL_5.DDC_PAD2_I2CMODE` and `PHY_AUX_CNTL.AUX2_PAD_RXSEL` for a
   hardware I2C transfer on DCN 2.x. Step 2 does not, on either line, and DDC1 read well without them. They are read, not
   written. If DDC2's pad reads otherwise than DDC1's and the EDID read fails, they are the next write to design.
3. **PHY B's DPCS/RDPCS are not read by step 1**, by the survey's rule since the hang. Table 12's path (short or long, by
   `RDPCSTX1_RDPCSTX_SCRATCH`) shows at stage 2, logged. Whether Linux ever powered PHY B is not known.
4. **What the Ryzen's tables write on PHY B.** Not run here (no image); the host case exists (procedure step 0).
5. **HDMI-A-2's monitor is not known.** Without its EDID the mode is CTA VIC 16 in DVI, the mode first light was shown in.
6. **The front end's fallback.** With DIG B busy it is the lowest free one, which may be A, HDMI-A-1's DIG. The guard then
   lets through that DIG's front end registers only.
7. **With a monitor in each socket at start, HDMI-A-1 is taken first.** A policy: the maintainer may want another.
8. **Batch 12's notes are seven lines** at every start, and at each plug of a socket 2 not driven. Once the values are
   recorded they could be cut to one.
9. **HPD 1's follower line keeps its old text.** `HPD 1 FOLLOWED ..., THE SECOND OUTPUT Lit` names the output's state, which
   may be HDMI-A-2's; HPD 2's line says which socket.
10. **Not addressed here: the hub on xHCI 1's port 1** that went away at 2054.3 s (the problem section above).

## Related

[213-DRV-0095](../issues-done/213-DRV-0095-display-the-second-outputs-mode-from-its-edid.done) (the mode from the EDID),
[213-DRV-0096](../issues-done/213-DRV-0096-display-the-second-output-plugged-and-unplugged.done) (the plug followed),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror),
[213-DRV-0078](../issues-done/213-DRV-0078-a-display-driver-for-the-ryzens-radeon.done) (the driver's main record).
