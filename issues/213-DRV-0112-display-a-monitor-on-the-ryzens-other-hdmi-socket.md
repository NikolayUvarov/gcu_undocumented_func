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

- Linux names them in that order. **The printed labels are the other way round** (the maintainer, 2026-11-27): the
  socket he calls HDMI2 is HDMI-A-1, the one lit first, with the 3440x1440 monitor; the socket he calls HDMI1 is
  HDMI-A-2, with the third monitor.
- Nothing of PHY B, of DIG back end B, of DDC2 or of HPD 2 has been read on this machine by MIND Core. What the
  firmware leaves there is not known.
- **Corrected 2026-11-27:** this line said there is "one spare pipe", and that is wrong. The engine has four pipes:
  - DCN 2.1 has four timing generators, OPPs, HUBPs and DPPs and five stream encoders (`dcn21_resource.c`, `res_cap`;
    `pipe_count` is four less any pipe fused off);
  - the firmware leaves all four HUBPs and DPPs powered, HUBP1 to HUBP3 blanked on their own idle OTGs (the survey's
    "Power");
  - Linux drove two monitors on all four.

  The firmware's picture takes one pipe and the second screen another. That `display` lights one HDMI socket at a time is
  its own limit, for now: one second pipe, and one second screen offered (`display.wit` 1.3). Both at once is
  [213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md).

## Plan

1. **Read first.** Step 1's batches read, and never write, what steps 2 to 4 need of the second socket: HPD 2's sense
   and control, DDC2's lines and the I2C engine's DDC2 registers, PHY B and DIG back end B as the firmware left them.
   A run on the Ryzen with the monitor in the second socket records the values here before any write is designed on
   them.
2. **The output as a parameter.** `HDMI_A_1` is fixed in the choice of the pipe, in the EDID read (`ddc.rs`), in the
   follower of the plug (`start.rs`), in the guard of what may be written, and in the tables' parameters. Each takes
   the output, with host cases for both.
3. **Which socket.** `display` follows both senses and lights the socket that has a monitor. With a monitor in each,
   the first lit stays; the other is named in the log as not driven, with the reason (one socket lit at a time, for now).
4. **The guard.** The registers of the second socket's PHY, back end, DDC and HPD join what may be written only for
   that output; the firmware's pipe and the other socket's blocks stay refused. A host case for each refusal.
5. **On the Ryzen, in stages as HDMI-A-1 was:** the read-only run; the EDID read; first light with the test pattern;
   MIND Core's picture; the plug taken out and put back; the monitor moved from one socket to the other.

## Acceptance criteria

- [x] Step 1 reads the second socket's blocks, with host cases that it writes nothing; its values from the Ryzen
  recorded here (boot 108, the socket empty and with the monitor in it).
- [x] The EDID read over DDC2, with host cases on the model of the engine; on the Ryzen its checksums good and the
  engine left as found (boot 109, below).
- [x] Host cases: the stages on a model with the monitor on the second socket; the guard's refusals for the first
  socket's blocks then; a monitor in each socket.
- [x] On the Ryzen: a monitor in the second socket lit in its own mode with the mirror on it, stage 15's watch clean,
  the firmware's pipe unchanged, and the maintainer's look (MC-12.1). Boot 112, below, on a bench image that takes
  HDMI-A-2 first.
- [ ] On the Ryzen: the monitor moved from one socket to the other and back, lit each time.
- [x] HDMI-A-1 alone behaves as before: its host cases pass unchanged and its lines on the Ryzen are the same (boots
  108, 109 and 113: the same mode, stage 15's watch clean, the offer 2011 to 2013 ms after step 3 began).

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
    HDMI-A-2 is named as not driven, with the reason (`display` lights one HDMI socket at a time, for now).
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
  HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW
  HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 READ): SENSE s; THE SECOND OUTPUT <Lit|Dark|Stopped> ON HDMI-A-1
  HPD 2 IS NOT ENABLED: ITS SENSE IS NOT FOLLOWED
  HDMI-A-2 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-1 IS LIT, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW
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
   write with PHY B's parameters, and whether all of it lies on HDMI-A-2's route. It passes since the second round.
   Also look up PHY A's FUSE0 in old survey-order logs (`S2 T12 WROTE AT 1783C`, the second round's open question).
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
     - while HDMI-A-2 is lit: `HDMI-A-1 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-2 IS LIT, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW`;
     - HDMI-A-2 lit once HDMI-A-1's monitor is out.

**Open questions, and what was not written for lack of evidence.**
1. **Which printed label each socket carries.** The read-only run's HPD 2 sense tells.
2. **DDC pad 2's I2C mode.** Display Core sets `DC_GPIO_AUX_CTRL_5.DDC_PAD2_I2CMODE` and `PHY_AUX_CNTL.AUX2_PAD_RXSEL` for a
   hardware I2C transfer on DCN 2.x. Step 2 does not, on either line, and DDC1 read well without them. They are read, not
   written. If DDC2's pad reads otherwise than DDC1's and the EDID read fails, they are the next write to design.
3. **PHY B's DPCS/RDPCS are not read by step 1**, by the survey's rule since the hang. Table 12's path (short or long, by
   `RDPCSTX1_RDPCSTX_SCRATCH`) shows at stage 2, logged. Whether Linux ever powered PHY B is not known.
4. **What the Ryzen's tables write on PHY B.** Run on LEG9 on a model since: all on HDMI-A-2's route, but for table 12's
   first write, of PHY A's `RDPCSTX_PHY_FUSE0`, left unmade where it changes nothing (the second round, below).
5. **HDMI-A-2's monitor is not known.** Without its EDID the mode is CTA VIC 16 in DVI, the mode first light was shown in.
6. **The front end's fallback.** With DIG B busy it is the lowest free one, which may be A, HDMI-A-1's DIG. The guard then
   lets through that DIG's front end registers only.
7. **With a monitor in each socket at start, HDMI-A-1 is taken first.** A policy: the maintainer may want another.
8. **Batch 12's notes are seven lines** at every start, and at each plug of a socket 2 not driven. Once the values are
   recorded they could be cut to one.
9. **HPD 1's follower line keeps its old text.** `HPD 1 FOLLOWED ..., THE SECOND OUTPUT Lit` names the output's state, which
   may be HDMI-A-2's; HPD 2's line says which socket.
10. **Not addressed here: the hub on xHCI 1's port 1** that went away at 2054.3 s (the problem section above).

## Second round (2026-11-26): table 12 on PHY B writes one register of PHY A

**What was seen.** The owner session ran the PHY B case with the Ryzen's image, and it failed: stage 2 was refused at a
write to BAR byte 0x1783C and undone. The guard did its work.

**What 0x1783C is.** `RDPCSTX0_RDPCSTX_PHY_FUSE0`: segment 2 dword 0x294F, PHY A's RDPCS (`dpcs_2_1_0_offset.h`). Its
bits 18-19 are `RDPCS_PHY_DP_MPLLB_V2I` (`dpcs_2_1_0_sh_mask.h`).

**Why table 12 writes it for PHY B: the table's own sequence, not our parameters.**
- **Our parameters are Display Core's for transmitter B.**
  - `set_pixel_clock_v7` passes the PLL of the stream's clock source. DCN 2.x maps it by transmitter
    (`dcn20_add_stream_to_ctx`, then `resource_map_phy_clock_resources` and `find_matching_pll`: UNIPHY B takes PLL1).
    PLL1 becomes `ATOM_COMBOPHY_PLL1` = 21 (`command_table_helper2_dce112.c`).
  - The encoder byte is `encoder_id_to_atom(ENCODER_ID_INTERNAL_UNIPHY)` = 0x1E, the enum dropped, as for PHY A. The
    CRTC is 1, for OTG 1.
  - The transmitter control 1.6 takes phy 1 (UNIPHY B), hpd_sel 2 (`ATOM_TRANSMITTER_V6_HPD2_SEL`) and dig_fe_sel 2
    (`ATOM_TRANMSITTER_V6__DIGB_SEL`).
  - Ours are the same bytes: `C8 A8 16 00 15 1E 02 00 01 00 ...` for table 12.
- **The write is table 12's first instruction, for every PHY.** Traced with the image on LEG9 on a model that refuses
  nothing:
  - table 12 first reads `RDPCSTX0_RDPCSTX_PHY_FUSE0` and writes it back with `MPLLB_V2I` = 2, its other bits kept;
  - only then does it read the chosen PHY's `RDPCSTX_PHY_CNTL6` and call table 77 on that PHY's own block;
  - it does so for PLL ids 20, 21, 22 and 24 alike, and for CRTC 0 or 1.
  - The field set was seen from several values before it: 0, 0x40000, 0x80000 and 0xC0000 each became 0x00080000; a
    value with the field already 2 was written back unchanged; 0xFFFFFFFF became 0xFFFBFFFF.
- **For PHY A this register is its own, on its route**, and the hardware's own run made the same write. The watched run
  of boot 66 shows stage 2, table 12, writing 0x1783C first, with `0xB008003E`. That is the value it held before, the
  field already 2.
- **Linux:** with early DMUB firmware, Display Core ran this same bytecode on the host (the survey's "DMUB question"), so
  it made the same write for HDMI-A-2. With today's firmware the DMUB runs the command; whether by this bytecode is not
  known.

**What each table writes for HDMI-A-2 on the image** (a model with PHY B's statuses answered; cold: PHY B's
`RDPCSTX_SCRATCH` 0; warm: 0x300; steps, calls as the interpreter counts them, writes):

| Table (stage) | 1080p60 DVI, cold | warm | 3440x1440 HDMI, cold | warm | Outside HDMI-A-2's blocks |
|---|---|---|---|---|---|
| 12 (stage 2) | 530, 4, 111 | 308, 4, 33 | 530, 4, 111 | 308, 4, 33 | one, every time: `RDPCSTX0_RDPCSTX_PHY_FUSE0` (MPLLB_V2I := 2) |
| 4 (stage 6) | 31, 2, 5 | 31, 2, 5 | 33, 2, 5 | 33, 2, 5 | none |
| 76 (stage 8) | 286, 9, 23 | 238, 9, 18 | 301, 9, 23 | 253, 9, 18 | none |
| 12, pixel clock 0 (stage 2's undo) | 101, 2, 14 | 101, 2, 14 | 101, 2, 14 | 101, 2, 14 | none |
| 76 DISABLE (stage 8's undo) | 160, 6, 18 | 160, 6, 18 | 160, 6, 18 | 160, 6, 18 | none |

- Table 12 enters 77, 77 and 46; table 4 enters 20; table 76 enters 77, 4, 20, 77, 77, 4, 20 and 77.
- Everything else lies in PHY B's DPCSTX1/RDPCSTX1, DIG B, DP1, UNIPHYB, PHYPLLB, SYMCLKB and OTG 1's pixel rate.
- PHY A's runs give the same counts, all on PHY A's route.

**The mend: the write is left unmade when it would change nothing, and refused otherwise** (`light.rs`:
`TABLE12_SETS_FOR_EVERY_PHY`, `Adapter::leave`). Nothing joins HDMI-A-2's writable set.
- **When the rule applies:** table 12 writes PHY A's FUSE0 while it is off the route, as it is for HDMI-A-2.
  - **If the register holds the value already,** the write is not made, and the table goes on. Logged:
    `S2 T12 LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C AS IT IS: TABLE 12 SETS IT FOR EVERY PHY, AND IT HOLDS THAT VALUE (xxxxxxxx)`.
    The table's line ends `..., n WRITES, 1 LEFT AS IT IS`.
  - **If the write would change it,** it is refused, and stage 2 is undone as any refusal is. Logged:
    `S2 T12 REFUSED PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C: TABLE 12 SETS IT FOR EVERY PHY, AND THIS WRITE WOULD CHANGE IT FROM xxxxxxxx TO yyyyyyyy`.
    Then `S2 PIXEL CLOCK FAILED: A WRITE OFF THE ROUTE, TO 1783C, WAS REFUSED; ...`.
- **Where it does not apply:** any other table's write of it, and any other register, are refused as before. For
  HDMI-A-1 the register is PHY A's own, and the write is made as before.
- PHY A is never written by HDMI-A-2's stages: neither its value nor a write of the same value.

**Host cases.**
- The PHY B case with the image now runs four ways:
  - cold and warm, with PHY A's FUSE0 as boot 66 read it: all 8 stages, table 12 `530 STEPS ..., 110 WRITES, 1 LEFT AS
    IT IS` and `308 STEPS ..., 32 WRITES, 1 LEFT AS IT IS`, the long and the short path, nothing of HDMI-A-1's written;
  - with PHY A's FUSE0 at 0: refused at stage 2 and undone, PHY A's FUSE0 never written;
  - all 16 stages at 3440x1440 in HDMI, then an unplug: taken back whole, but PHY B's control register bus, left by
    design.
- A synthetic case, which runs without the image:
  - the write is left where the value is held, and refused where it is not;
  - the same write from table 76 is refused;
  - on HDMI-A-1's route the write is made.

**Open, from this round.**
- **PHY A's FUSE0 when HDMI-A-2's stage 2 runs.** The rule lets HDMI-A-2 light only if `MPLLB_V2I` is 2 already.
  - It was on boot 66 (`0xB008003E`, after a restart out of Linux).
  - After a power-on with no first light on HDMI-A-1 it is not known.
  - HDMI-A-1's own stage 2 sets it, but its undo (an unplug) puts back the value from before.
  - What to look at first, nothing written: survey-order logs of HDMI-A-1's first lights (boots 70 to 81) carry
    `S2 T12 WROTE AT 1783C = ... (WAS ...)`; the `WAS` is the value at each boot.
  - Or, on the maintainer's word, batch 12 could read this one register. The table reads it on every first light, but
    the survey's rule keeps RDPCS out of step 1, so it is not added.
- **If the value is found otherwise**, letting the table set the field on a PHY A that drives nothing is a decision for
  the maintainer. It is what table 12 does first for PHY A itself; it is not done now.

**Looked at since, by the owner session (2026-11-26): the saved survey-order logs.**
- Eight boots of 2026-11-21 and 22 have the line, each of them a first light on HDMI-A-1: boots 66, 74, 75, 76, 77, 78,
  79 and 81. In every one table 12's first write is `S2 T12 WROTE AT 1783C = B008003E (WAS B008003E)`: the register held
  the value already, `MPLLB_V2I` 2, before `display` wrote anything.
- So in those boots HDMI-A-2's stage 2 would have left the register as it is and gone on.
- All eight were starts out of Linux (the machine starts MIND Core from Ubuntu). A start not through Linux is still not
  known, as for table 12's cold path ([213-DRV-0097](213-DRV-0097-display-table-12s-cold-path-on-the-ryzen.md)).
- The same logs show the register changed later in stage 2, on HDMI-A-1's own route (`= B038003E (WAS B008003E)`): bits
  20 and 21, not the field in question.
- The real image was fetched from the machine's own log volume to the owner session's private directory for these
  host cases; it is not in the repository.

## The read-only run, first part: nothing in the second socket (boot 108, 2026-11-26)

Release 202611262015, boot 108: `claude/DRV-ryzen-bench-42-hdmi2` abe47faa4, which is `main` 9d6feac696 with this
branch, 213-DRV-0111 and the bench-only line of `init`. `HDMI_A_2` is `Read`. The monitor was in the first socket, the
second empty. It holds for that machine and that release (MC-12.1). Through QEMU before it: "x86: keys, shell, tools",
"x86: services, storage, audio" and the update from the release before.

**Batch 12, as logged at 3.6 to 3.7 s, before step 3 wrote anything:**
- `B12 HDMI-A-2 HPD 2 (HPD1_): SENSE 0, ENABLED 1; INT_STATUS 00000000, CONTROL 10FA09C4`
- `B12 HDMI-A-2 DIG B: FE FROM OTG 0, SYMCLK_FE_ON 0 (00000000); BE ENABLED 0, MODE 1, FROM FE MASK 0, SYMCLK_BE_ON 1 (00010000, 00000100)`
- `B12 HDMI-A-2 PHYPLLB: PIXCLK_ENABLE 0, DEEP COLOR 0 (00000000)`
- `B12 HDMI-A-2 DDC2: HW STATUS 0 (00000000); ENABLED 1, TIME LIMIT 48, PRESCALE 480, THRESHOLD 2 (SPEED 01E00002, SETUP 30000040)`
- `B12 HDMI-A-2 DDC2 PADS: AUX MODE 0; CLOCK MASK 0 EN 0 A 0 Y 1; DATA MASK 0 EN 0 A 0 Y 1 (MASK 00000000)`
- `B12 THE DDC PADS' I2C MODE: DDC1 1 DDC2 1; AUX PAD RXSEL 1:1 2:1 (DC_GPIO_AUX_CTRL_5 0001F2AA, PHY_AUX_CNTL 00155401)`
- `B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN`

**What they say:**
- No read hung, and all 15 registers answered.
- HPD 2 is enabled and senses no monitor, as the socket was empty.
- DIG back end B is not enabled and PHY PLL B's pixel clock is off: nothing drives the second socket.
- The I2C engine's DDC2 line is idle and set up (speed and setup hold values, not zeros).
- **Open question 2 is answered for this start:** DDC pad 2 is in I2C mode already (`DDC2 1`, `AUX PAD RXSEL … 2:1`),
  as pad 1 is. No write of those two registers is needed before an EDID read over DDC2.
- All of it is after a start out of Linux, as every boot of this machine so far.

**HDMI-A-1 beside it, unchanged:** `STEP 1 DONE: 11 OF 11 BATCHES READ, 143 REGISTERS`, the second screen offered
2012.8 ms after step 3 began, `HPD 1 FOLLOWED …: SENSE 1, THE SECOND OUTPUT Lit`, and
`HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 READ): SENSE 0; THE SECOND OUTPUT Lit ON HDMI-A-1`.

**The second part, the same boot: the monitor moved into the second socket and back** (the maintainer's hands, about
21:11 UTC; uptime in seconds).
- 3186.310 `STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR`, taken back in 29.9 ms.
- 3191.412 `HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE; THIS BUILD GOES NO FURTHER THAN ITS BLOCKS READ ONLY (HDMI_A_2 READ)`
- Batch 12 with the monitor there, 15 of 15 read, nothing written:
  - `B12 HDMI-A-2 HPD 2 (HPD1_): SENSE 1, ENABLED 1; INT_STATUS FF000013, CONTROL 10FA09C4`
  - DIG B, PHY PLL B, DDC2 and the pads exactly as with the socket empty: `BE ENABLED 0`, `PIXCLK_ENABLE 0`,
    `HW STATUS 0`, both pad lines read high (`Y 1`), `DDC1 1 DDC2 1`.
- 3200.342 `HDMI-A-2: ITS MONITOR TAKEN OUT, NOT DRIVEN`, after 8.9 s there; `SENSE 0`, `INT_STATUS FF0FF001`.
- 3205.881 `A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT Lit`, on HDMI-A-1 again.

**What it settles:**
- **The socket the maintainer calls the second one is HDMI-A-2**, VBIOS path 1: HPD 2 sensed his monitor there.
- A monitor there changes nothing in DIG B, PHY PLL B or the DDC2 engine by itself: the firmware and Linux have set up
  nothing for it on a boot where Linux did not see a monitor in that socket.
- Nothing was written for the second socket, and HDMI-A-1 lit again afterwards as before.

**Seen again, the second time of two:** when the cable goes into the second socket, the hub on xHCI 1's port 1
(05E3:0608) drops and comes back, with the mouse's receiver and the serial converter behind it (3190.902 s `THE
CONVERTER IS GONE`, 3191.067 s `PORT 1: CONNECTED`), half a second before HPD 2's sense. On boot 107 it was 2054.3 s.
Whether that hub is in the monitor is still asked of the maintainer. It costs the serial link about a second.

**Before first light there, known and not known:**
- **PHY B is most likely cold.** Table 12 then takes its long path (530 steps, 111 writes on the model with the real
  image), which has never run on this hardware for any PHY ([213-DRV-0097](213-DRV-0097-display-table-12s-cold-path-on-the-ryzen.md)).
  Whether it is cold is said by stage 2's own line; step 1 does not read RDPCS.
- So first light on the second socket is to be made with a person at the machine, in the survey's order and undone
  after its stages on the first run.

## Third round (2026-11-27): the true reason, the socket not taken read, the socket taken first

After boot 108's read-only run, with the maintainer's second monitor in HDMI-A-1 and a third in HDMI-A-2.

**1. The reason a socket is not driven, corrected.**
- "One spare pipe" was wrong. DCN 2.1 has four pipes (`dcn21_resource.c` `res_cap`), the firmware leaves all four HUBPs
  and DPPs powered, and Linux drove two monitors on all four.
- The lines now say `... AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW`: `display`'s own limit, one second pipe
  and one second screen (`display.wit` 1.3).
- Both at once is [213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md), a design for now.

**2. The EDID of a socket with a monitor but not taken** (`start.rs`: `untaken_edid`, `engine_now`, `Naming`;
`batches.rs`: `read_one`).
- **When it is read:**
  - from level `Edid` on, where HPD senses a monitor in the socket steps 3 and 4 do not take (a monitor in each socket,
    or one plugged into the socket not lit);
  - over its DDC line as step 2 reads one, the engine given back as found, every line named for the socket.
- **At start** it is read right after the taken socket's own step 2, before any stage.
- **At a plug** into the socket not lit, the engine is read again first: batches 1 and 11, and batch 12 for HDMI-A-2,
  read only.
- **At level `Read` there is no transaction.**
- **What a transaction on line 1 touches:**
  - the engine's shared `DC_I2C_CONTROL`, `ARBITRATION` and `TRANSACTION0/1`, put back as batch 11 found them, the engine
    released;
  - its `DATA` buffer;
  - line 1's own `DDC2_SPEED`, `DDC2_SETUP` and `DC_GPIO_DDC2_MASK`, put back as batch 12 found them.
- **What it does not touch:**
  - line 0's registers, which step 2's guard refuses;
  - nothing a lit TMDS output reads: the DIG, PHY, OTG, OPP, HUBP and HPD blocks;
  - SCDC is not used (no scrambling at or below 340 MHz).
  - Line 0's own set-up, and with it its EDID detection, is never written.
  - A correction to commit 2429a1343's message: the `0x30000040` it gives is DDC2's `SETUP` from boot 108, not DDC1's.
- **The failures:** an engine held by the hardware is refused before any write; a NACK or a timeout gives the engine
  back as any failure does.
- **Host cases:**
  - a monitor in each at `Edid`: HDMI-A-1's writes and offer the same as without the second monitor, DDC2's read before
    any stage, only DDC2's registers written and all as found;
  - `Read`: no transaction;
  - a monitor plugged into HDMI-A-2 while HDMI-A-1 is lit, with an EDID, a NACK and a held engine: no engine write,
    HDMI-A-1 still lit, eleven minutes of the standing check clean, the engine as found.

**3. `HDMI_FIRST`, the socket taken when both have a monitor** (`setting.rs` `HdmiFirst`, `main.rs`):
- committed `HdmiA1`, as before;
- with `HdmiA2`, HDMI-A-2 is taken first where `HDMI_A_2` lets display take it, and HDMI-A-1 stays dark, named, its EDID
  read as a socket not taken;
- with `HDMI_A_2` at `Read` it cannot take HDMI-A-2, and HDMI-A-1 is taken.
- Host cases both ways.

**The lines at level `Edid` with a monitor in each socket** (production order, `HDMI_FIRST` `HdmiA1`):
```
HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW
D STEP 2 DONE: ...                                    (HDMI-A-1's own EDID, as before)
HDMI-A-2 NOT TAKEN: ITS EDID READ OVER DDC2 ALL THE SAME, THE I2C ENGINE GIVEN BACK AS FOUND; NOTHING ELSE OF IT WRITTEN
HDMI-A-2: D STEP 2 DONE: n BLOCKS READ (m BYTES) IN ... US, ...; k WRITES PUT BACK, 6 OF 6 REGISTERS AS BATCHES 11 AND 12 READ THEM
HDMI-A-2: D EDID v.r, FNV-1A ...: MMM PRODUCT pppp, yyyy, "NAME"; ...
HDMI-A-2: D PREFERRED: WxH AT f HZ, k KHZ, IN ...
HDMI-A-2: D AN HDMI SINK: ...                         (or HDMI-A-2: D NO HDMI BLOCK: ...)
```
- A failure reads `HDMI-A-2: D STEP 2 NOT DONE: <why>; STEPS 3 AND 4 GO ON WITHOUT IT`. Its detail comes first in the
  production order: `HDMI-A-2: D STOPPED: ...`.
- A monitor plugged into HDMI-A-2 while HDMI-A-1 is lit gives the `NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-1 IS
  LIT, AND DISPLAY LIGHTS ...` line, then the `B1`, `B11` and `B12` reads (the B12 notes logged), and the same
  `HDMI-A-2 NOT TAKEN` and `HDMI-A-2: D ...` lines.
- With `HDMI_FIRST` `HdmiA2` the roles swap: `HDMI-A-1 NOT DRIVEN: HPD 1 SENSES A MONITOR THERE TOO; HDMI-A-2 IS TAKEN
  FIRST (HDMI_FIRST HDMI-A-2), ...`, then `HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, TAKEN FIRST; HDMI_A_2 <LEVEL>: ...`,
  and `HDMI-A-1 NOT TAKEN: ITS EDID READ OVER DDC1 ...` with `HDMI-A-1: D ...` lines.

**How the bench tree sets them**, in `display/src/main.rs`, uncommitted, as the other switches are:
- `const HDMI_A_2: setting::HdmiA2 = setting::HdmiA2::Edid;`
- `const HDMI_FIRST: setting::HdmiFirst = setting::HdmiFirst::HdmiA2;` (to take HDMI-A-2 first)

The build's first line then names them: `A BRING-UP BUILD, NOT A RELEASE: HDMI_A_2 EDID HDMI_FIRST HDMI-A-2; ONLY WHAT
LIGHT DOES CHANGES`.

## The second level on the Ryzen: the EDID over DDC2 (boot 109, 2026-11-26)

Release 202611262214, boot 109: `claude/DRV-ryzen-bench-43-edid` f7a63ca79, which is `main` 9d6feac696 with this branch
at 029649c43, 213-DRV-0111, the bench-only line of `init` and a bench-only `HDMI_A_2 = Edid`. A second monitor is in
HDMI-A-1 and a third in HDMI-A-2. It holds for that machine, those monitors and that release (MC-12.1). Through QEMU
before it: "x86: keys, shell, tools", "x86: services, storage, audio" and the update from the release before. The
maintainer was away; the level writes nothing of the second socket but the I2C engine's registers, which it puts back.

**The lines, 3.5 to 3.9 s:**
- `A BRING-UP BUILD, NOT A RELEASE: HDMI_A_2 EDID; ONLY WHAT LIGHT DOES CHANGES`
- `B12 HDMI-A-2 HPD 2 (HPD1_): SENSE 1, ENABLED 1; INT_STATUS 00000012, CONTROL 10FA09C4`, the other six notes as on
  boot 108, `B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN`
- `HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW`
- HDMI-A-1's own read first, as on every boot: `D STEP 2 DONE: 2 BLOCKS READ (256 BYTES) IN 25151 US … 18 WRITES PUT BACK, 6 OF 6 REGISTERS AS BATCH 11 READ THEM`
- `HDMI-A-2 NOT TAKEN: ITS EDID READ OVER DDC2 ALL THE SAME, THE I2C ENGINE GIVEN BACK AS FOUND; NOTHING ELSE OF IT WRITTEN`
- `HDMI-A-2: D STEP 2 DONE: 2 BLOCKS READ (256 BYTES) IN 25208 US, THE TRANSFERS 12687 AND 12141 US FROM GO; 18 WRITES PUT BACK, 6 OF 6 REGISTERS AS BATCHES 11 AND 12 READ THEM`
- `HDMI-A-2: D EDID 1.3, FNV-1A 6B7A8223DF75D9C7: TFG PRODUCT 2032, 2016, "HD"; 1 EXTENSIONS ANNOUNCED, 2 BLOCKS READ`
- `HDMI-A-2: D PREFERRED: 1366x768 AT 59.790 HZ, 85500 KHZ, IN 1792x798`
- `HDMI-A-2: D AN HDMI SINK: TMDS AT MOST 165 MHZ (HDMI 1.4 BLOCK 0 MHZ, HDMI FORUM BLOCK 0 MHZ)`

**What they say:**
- The read over DDC2 worked at the first try, in the same time as DDC1's (25.2 ms for two blocks), with no write of the
  pads' I2C mode. Both blocks' checksums were good, or step 2 would say `NOT DONE`.
- The engine was given back: 18 writes put back, 6 of 6 registers as the batches read them.
- **The third monitor** is another make and a small mode: 1366x768 at 85.5 MHz, an HDMI sink with no HDMI 1.4 clock
  figure, so at most 165 MHz. Its first light would be within DVI's range, a quarter of the first monitor's clock.
- **HDMI-A-1 beside it, undisturbed:** stage 15 `WATCHED 60 FRAMES IN 1001 MS CLEAN`, the second screen offered 2011.3 ms
  after step 3 began (2012.8 on boot 108), `HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 EDID): SENSE 1; THE SECOND
  OUTPUT Lit ON HDMI-A-1`. Step 2 as a whole took 240.0 ms with the two reads, against 113.2.

**Not shown yet:** the read at a plug into the undriven socket while HDMI-A-1 is lit (the monitor was there from the
start of this boot), and anything of first light.

## First light and the picture on HDMI-A-2 (boots 110 to 112, 2026-11-27)

Three bench images, each this branch at 029649c43 on `main` 9d6feac696 with 213-DRV-0111, the bench-only line of `init`
and one bench-only commit of switches: `HDMI_FIRST = HdmiA2`, so that HDMI-A-2 is taken first while a monitor also
sits in HDMI-A-1, and the survey's order. The third monitor (1366x768) is in HDMI-A-2, the second (3440x1440) in
HDMI-A-1, which these images leave dark by design. Each image went through QEMU first ("x86: keys, shell, tools",
"x86: services, storage, audio", the update from the release before it). It holds for that machine, those monitors and
those releases (MC-12.1). The maintainer was at the machine («продолжай»).

**Boot 110, release 202611270508 (13788a11f): `HDMI_A_2 FirstLight`, undone after its stages.**
- `HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, TAKEN FIRST; HDMI_A_2 FIRST_LIGHT: ITS EDID AND FIRST LIGHT'S 8 STAGES`;
  `HDMI-A-1 NOT DRIVEN: … HDMI-A-2 IS TAKEN FIRST (HDMI_FIRST HDMI-A-2) …`; `HDMI-A-1 NOT TAKEN: ITS EDID READ OVER DDC1 ALL THE SAME`.
- `STEP 3'S MODE: 1366x768 AT 85500 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING`.
- `S1 OTG CLOCKS DONE` to `S8 PHY DONE`, each with `THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`.
- **Stage 2:** `S2 T12 LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT 1783C AS IT IS: TABLE 12 SETS IT FOR EVERY PHY, AND IT HOLDS THAT VALUE (B008003E)`;
  `S2 TABLE 12 DONE: 318 STEPS, 4 CALLS, 10 US OF DELAYS, 32 WRITES, 1 LEFT AS IT IS`;
  `S2 THE PHY WAS SET UP ALREADY (SCRATCH 0x300): THE SHORT PATH, NO WRITE THROUGH ITS BUS`.
- **Stage 6:** table 4, 33 steps, 5 writes. **Stage 8:** table 76 with `01 01 03 04 66 21 00 00 02 02 0C …`, 289 steps,
  23 writes, among them PHY B's block (0x17B04 to 0x17BA8) and `DIG1_DIG_BE_EN_CNTL = 00000101 (WAS 00000100)`.
- `STEP 3 RAN 8 OF 16 STAGES (112 WRITES); UNDOING STAGES 8 TO 1 AS THIS BUILD ASKS`, `U8` to `U1` undone (28, 1, 24, 3,
  4, 20, 33 and 2 writes restored), `STEP 3 UNDONE AFTER STAGE 8 OF 16 …; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED`.

**Boot 111, release 202611270515 (e327fb61a): the same, left in place.** The same lines; `STEP 3 STOPPED AFTER STAGE 8
OF 16 AS THIS BUILD ALLOWS; 112 WRITES MADE ARE LEFT IN PLACE`; `HPD 2 FOLLOWED TOO … SENSE 1; THE SECOND OUTPUT Lit ON
HDMI-A-2`. **The maintainer's look:** «цветные квадратики на третьем, второй черный».

**Boot 112, release 202611270533 (7bd2f9829): `HDMI_A_2 Full`, all 16 stages.**
- `S9 OTG0 COUNTED 30 FRAMES IN 500251 US: 59.969 HZ, ITS PIXEL CLOCK 268492 KHZ; THE REFERENCE CLOCK 48000 KHZ`.
- `S10 DRAW DONE; … 1366x768 DRAWN AND READ BACK`; `S13 HUBP DONE; … THE PITCH READ BACK AS WRITTEN`.
- `S15 WATCHED 60 FRAMES OF OTG1 IN 990 MS (60.601 HZ) WITH THE PATTERN OFF: 0 PASSED BETWEEN TWO READS; THE LONGEST GAP BETWEEN READS 2470 US`,
  `S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 60 FRAMES`. The 60.601 is the watch's own start and end
  within a frame, not the mode's rate (59.790 Hz by its timing).
- `[COMPOSITOR] SECOND SCREEN 1366X768, PITCH 5632, Bgr, FROM PID 39: THE FIRST MIRRORED AT 1228X768 FROM 69,0`;
  `S16 OFFER DONE; … OFFERED 1366x768 PITCH 5632`; `STEP 3 STOPPED AFTER STAGE 16 OF 16 …; 178 WRITES MADE ARE LEFT IN PLACE`;
  the standing check running.
- Every stage ended with `THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`.
- **The maintainer's look:** «на третьем монитре (hdma1) видна картинка загрузочного текста mind-core-os».

**What it shows, and what it does not.**
- HDMI-A-2 lights in its monitor's own mode with the mirror on it, on the pipe HDMI-A-1 otherwise takes.
- The rule for PHY A's FUSE0 held on hardware: the write was left unmade, the value being there already.
- **Table 12's long path did not run:** PHY B was set up already on all three boots. Each release restarts through
  Linux, which lights that socket when a monitor is in it. The long path would be met by a monitor plugged into a socket
  Linux did not light. Not shown.
- Not shown either: a plug change on HDMI-A-2 while it is the lit one, the monitor moved between the sockets with
  both levels at `Full`, and the production order on HDMI-A-2 (these three boots used the survey's).

**After it, at the maintainer's word** («до поддержки пусть работает HDMI2 (который у нас заработал первым)»): the
machine is back on an image that takes HDMI-A-1 first (release 202611270607, boot 113: `HDMI-A-2 NOT DRIVEN … HDMI-A-1
IS TAKEN FIRST`, its EDID read, HDMI-A-1 lit as before). Both at once is
[213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md).

## Related

[213-DRV-0095](../issues-done/213-DRV-0095-display-the-second-outputs-mode-from-its-edid.done) (the mode from the EDID),
[213-DRV-0096](../issues-done/213-DRV-0096-display-the-second-output-plugged-and-unplugged.done) (the plug followed),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror),
[213-DRV-0078](../issues-done/213-DRV-0078-a-display-driver-for-the-ryzens-radeon.done) (the driver's main record).
