# 213-DRV-0095 — `display`: the second screen in its monitor's own mode, from its EDID

**Type:** driver (`display`, step 5) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

Steps 3 and 4 light HDMI-A-1 at the fixed CTA 1920x1080 at 60 Hz, 148.5 MHz (`timing::CEA_1080P60`), in DVI
signalling, whatever monitor is plugged in. Stage 10 draws at that size, and stage 16 offers it to the compositor.

The Ryzen's HDMI-A-1 monitor is a 3440x1440 HDMI 2.0 sink. It prefers 3440x1440 at 60 Hz, 324 MHz (213-DRV-0085;
Linux drove it so: OTG 0 at 3440x1440 in a total of 3600x1500). The mirror at 1920x1080 on it shows dark bands at the
edges. The maintainer remarked on it after boot 76 (213-DRV-0092): «второй экран имеет другое разрешение физическое
и картинка имеет темные полосы по краям».

The stretched mirror and the extended desktop he also asked for are the compositor's (KRN) and `wm`'s (APP). This task
is `display`'s part: the second screen in the monitor's own mode.

## What is known, and what must be read first

- **The EDID's bytes:** `display` has not read them. `edid.rs` parses an EDID: its base block, the CTA extension, the
  HDMI and HF vendor blocks and the maximum TMDS clock. The 17 VICs in `main.rs` came from Linux's copy (sysfs, read
  2026-11-21), and the bytes stay out of the repository.
- **No copy to take it from:**
  - The firmware lights only DP-2 (GOP 0, 213-DRV-0074), so UEFI's EDID protocols concern DP-2.
  - The VBIOS holds no external monitor's EDID.
  - The DMCUB, which reads it under Linux, is held in reset.

  So `display` reads it over HDMI-A-1's DDC.
- **The line:** the VBIOS's display path 0 puts HDMI-A-1's DDC on I2C line 0, `DC_GPIO_DDC1` (213-DRV-0085).
- **The engine:** DCN 2.1's I2C engine, as Display Core drives it (`dce_i2c_hw.c`, Linux v7.0). Every register of it
  and of the DDC1 pads lies inside `display`'s window of BAR 5, [0x48, 0x1B000) (`dcn_2_1_0_offset.h`):

  | Register | Byte offset |
  |---|---|
  | `DC_I2C_CONTROL`, `_ARBITRATION`, `_SW_STATUS`, `_DDC1_HW_STATUS` | 0x14D60, 0x14D64, 0x14D6C, 0x14D70 |
  | `DC_I2C_DDC1_SPEED`, `_DDC1_SETUP` | 0x14D88, 0x14D8C |
  | `DC_I2C_TRANSACTION0` to `3`, `DC_I2C_DATA` | 0x14DB8 to 0x14DC4, 0x14DC8 |
  | `MICROSECOND_TIME_BASE_DIV` (the speed's base) | 0x004EC |
  | `DC_GPIO_DDC1_MASK`, `_A`, `_EN`, `_Y` (the pads) | 0x17640 to 0x1764C |

  DCN 2.1 has no I2C light-sleep fields in `DIO_MEM_PWR_CTRL`, so Display Core does not touch it here, and neither does
  `display`.
- **First, a read-only batch** (step 1's batch 11, `DDC1`, in this task's first commit). It reads the 15 registers
  above but `DC_I2C_DATA`, because a read of it moves its index. These are registers Display Core reads before every
  transfer. Batch 11 shows:
  - who holds the engine (software, the DMCU or nobody);
  - the last transfer's status;
  - DDC1's enable, time limit, prescale and threshold;
  - the time base;
  - the pads' mode (AUX or I2C), masks, enables and levels. Both lines high means idle with a monitor's pull-ups.

  Each value is logged as it is read, so that a hang would name its register. The survey's Linux sweep of 2026-11-21,
  which hung the machine once, also read I2C control among some 3675 registers. Which read hung it is not known
  (213-DRV-0085). Steps 3 and 4 still need only batches 1 to 10.
- **Then the read itself, a write sequence and its own stage, on the owner session's word:**
  - **The transfer:** Display Core's order (`dce_i2c_hw`):
    1. take the engine (`DC_I2C_SW_USE_I2C_REG_REQ`) and see it granted;
    2. DDC1's speed (prescale from the time base, 100 kHz) and set-up (enable, time limit);
    3. the transactions: write the offset 0 to 0x50, then read 128 bytes; and for the CTA block, the segment pointer
       0x30 when E-DDC needs it, or offset 128;
    4. `DC_I2C_GO`, then `DC_I2C_SW_DONE` polled with a time limit;
    5. the data read out of `DC_I2C_DATA` with its index;
    6. the engine given back (`DC_I2C_SW_DONE_USING_I2C_REG`).
  - **The pads:** if batch 11 shows them in AUX or GPIO mode, the I2C pad mode first (`AUX_PAD1_MODE` 0 and the masks
    0, as `hw_ddc` sets them). The undo puts every register back as batch 11 read it.
  - **Checks:** the EDID's checksums and header checked, its timings parsed by `edid.rs`, and its bytes logged as a
    hash only.

## Choosing the mode

The preferred timing, if all the limits below allow it; otherwise the largest the EDID lists that they allow; otherwise
1920x1080 at 60 Hz as now. For the Ryzen's monitor, 3440x1440 at 324 MHz:

| Limit | Value | 3440x1440 at 324 MHz |
|---|---|---|
| DVI on one link | 165 MHz (`DVI_SINGLE_LINK_KHZ`) | over: HDMI signalling is needed, so infoframes, and a new mode for tables 4 and 76 |
| HDMI without scrambling | 340 MHz (`TMDS_NO_SCRAMBLING_KHZ`; the sink's own maximum from its HF-VSDB too) | within |
| DCN 2.1's HDMI link encoder | 600 MHz (`max_hdmi_pixel_clock`, `dcn21_resource.c`); 6 Gbit/s only where the VBIOS's encoder caps say so | within |
| DISPCLK and DPPCLK the firmware left | `DENTIST_DISPCLK_CNTL` 0x141F1414: DID 0x14 for both, a divider of 5, so 720 MHz at Linux's default DENTIST VCO of 3600 MHz (`rn_clk_mgr.c`; the VCO itself not read) | within, for one pipe at 1:1 |
| Slot 5's surface | 20 MiB | 3456 x 1440 x 4 = 19,906,560 bytes fits |
| The pitch | the hardware rounds the field, to a unit of at most 128 pixels (213-DRV-0092, "The pitch"); Linux used 3456 for 3440 | 3456 pixels (13,824 bytes), 27 x 128 |

Two things are left to see before the mode is set:
- **One pipe or two:** see "One pipe or two" below. One DPP carries 3440 pixels a line; Linux's two were its split
  policy, not a limit.
- **The marks and the mirror** follow the offered size. The compositor fits DP-2's 2560x1600 into 3440x1440: 2304x1440
  from x 568, so the bands shrink but stay (KRN's stretched mode is the rest).

## Table 12 at another pixel clock

This is new ground for the PHY only in part. On the model with the Ryzen's own tables (the warm state of 213-DRV-0086's
host cases, a local probe with HDMI signalling above 165 MHz; MC-12.2: a model, not the hardware):
- **The short path (PHY A's mark 0x300 set) at 324 MHz** is the same as at 148.5 MHz: 308 steps, 33 writes, none
  through the PHY's control register bus.
  - Four of its writes carry other values. These are the RDPCS registers that set MPLLB for the clock: 0x1781C,
    0x1782C, 0x17830 and 0x1783C (for example 0x1782C 0x105A0 at 148.5 MHz, 0x10660 at 324 MHz).
  - So the short path does program the PLL for the new clock, through the RDPCS's own registers.
- **The long path (mark 0)** writes the same 37 values through the bus at both clocks. They do not depend on the clock;
  only the same four RDPCS values differ.
- **Tables 4 and 76 in HDMI mode** (encoder mode 3): 33 and 301 steps, against 31 and 286 at DVI. The writes are as
  many (5 and 23), with other values.
- **On the hardware,** every boot from 70 to 76 took the short path at 148.5 MHz. Each came after a Linux session,
  which drives PHY A at 324 MHz through the DMCUB (the owner session's note; 213-DRV-0085's survey). So the short path
  has already moved this PLL between the two clocks. 324 MHz is the clock Linux uses on this PHY.
- **What is new** is HDMI signalling (tables 4 and 76 in HDMI mode, the stream encoder's HDMI set-up and an AVI
  infoframe) from MIND Core. That is the run to ask the maintainer about, not the PHY's bus. Stage 2's line still names
  the path each boot.

## One pipe or two for 3440x1440 at 324 MHz

Linux drove HDMI-A-1 with two pipes, and DP-2 too. They were combined by **MPC**, not ODM:
- HDMI-A-1: MPCC 0 over MPCC 3 into OPP 0, HUBP 0 and 3 at 1720x1440 each;
- DP-2: HUBP 1 and 2 at 1280x1600 each.

These come from Display Core's dump of 2026-11-21 (hwdocs/cezanne-display.md). That is Display Core's split policy on
DCN 2.1, `pipe_split_policy = MPC_SPLIT_DYNAMIC` (dcn21_resource.c): four pipes for two planes, each plane split. The
firmware drives DP-2's 2560 with one pipe.

DML 2.1's limits for one DPP (display_mode_vba_21.c, dcn20_fpu.c's `dcn2_1_ip`, Linux v7.0):
- **The swath:** at most 8192 pixels for a linear surface (`MaximumSwathWidthSupport`).
- **The DET:** 164 KB, half for each swath. At 4 bytes a pixel and a linear swath of one line, that is over 20,000
  pixels, capped at 8192.
- **The line buffer:** 789,504 bits at 36 bits a pixel with one vertical tap, about 21,900 pixels.
- **`dpp_output_buffer_pixels` 2560** is not a limit. Past it, DML counts half a line of latency hiding in that buffer,
  not one.
- **The DPP clock:** one DPP needs the pixel clock at 1:1, 324 MHz x 1.005. That is within even DPM state 0's 400 MHz
  DPPCLK, and the firmware left 720 MHz. Without either, DML's `NoOfDPP` would be 2.

So one pipe carries 3440x1440 at 324 MHz, and no ODM path is needed. Stage 15's watch remains the check on the hardware,
with the shorter latency hiding and both pipes fetching (2.2 GB/s in all).

## Step 2: the EDID read (second commit)

`ddc.rs`, run by `display` between step 1 and step 3 when `EDID_ALLOWED` is set (committed false), after batch 11:
- **Before any write:** if batch 11 was not read, if HPD 1 shows no monitor, or if the engine is held by the hardware
  or the DMCU, nothing is written.
- **Display Core's order** (`dce_i2c_hw.c`, `hw_ddc.c`, `hw_gpio.c`):
  1. the pads to hardware I2C: each pin's mask 0, both pull-downs on, the pad out of AUX mode;
  2. out of soft reset, the engine asked for and its grant seen;
  3. 100 kHz from the microsecond time base (`set_speed`): a prescale of 480 from 48 MHz;
  4. the control, DDC1's set-up (time limit 3, enabled), the arbitration's queue bit;
  5. for each 128-byte block: transaction 0 writes its offset to 0x50 without a stop, transaction 1 reads 128 bytes;
     their bytes go into the buffer;
  6. the status reset, GO, and `DC_I2C_SW_STATUS` polled for at most 500 ms;
  7. the reply read out of the buffer from its index 3.
- **Blocks:** the base block, then its extension if it announces one; more would need E-DDC's segment pointer.
- **Checks:** the header, and each block's checksum.
- **Given back always**, after a read or any failure:
  - `release_engine`: a soft reset if a transfer is still the engine's, the status reset, then
    `DC_I2C_SW_DONE_USING_I2C_REG`, and again if the engine is still held;
  - then every register written put back, last first;
  - then each checked against what batch 11 read.

  `DC_I2C_DATA` is written as Display Core writes it, without a read first, and read only for the reply.
- **Writes:** only through `light::ddc_writable`, which lets through the I2C engine's control, arbitration, DDC1's
  speed and set-up, the transactions, the buffer and DDC1's pad mask, and nothing else.
- **What a failure leaves:** a NACK, a timeout, no answer or a bad block ends step 2 with the engine given back. Steps 3
  and 4 go on as without it, so the mirror comes up.
- **The log:**
  - the EDID's hash, its maker's code, product, year and name, and the extension count;
  - the preferred timing;
  - whether it is an HDMI sink and its maximum TMDS clock (the HDMI 1.4 and HDMI Forum blocks);
  - every timing it gives in full, with whether DVI carries it;
  - its standard timings, sizes and rates only;
  - its CTA VICs;
  - the DVI candidate below.

  Its bytes are not logged.
- **Host cases** (`display_ddc_host.rs`, 4) on a model of the engine (grant and release, the indexed buffer, the
  transactions on GO against a monitor at 0x50):
  - both blocks in Display Core's order, the engine as found after;
  - one block when no extension is announced;
  - a NACK, an engine that never ends a transfer, a bad checksum, each given back as found;
  - held by the DMCU, batch 11 not read, no monitor: not one write;
  - the guard;
  - what the EDID says.

## A mode within DVI first

Before HDMI signalling: the largest timing the monitor gives in full (a detailed timing, or a CTA VIC it lists) within
DVI's 165 MHz whose shape is nearer its preferred timing's than 1920x1080's (`Edid::nearer_shape_within`). If there is
one, it is a first, smaller step with no new signalling. Step 2's log names it, or says there is none.
- **What is known before the read:** Linux's mode list (sysfs, 2026-11-21) has 3440x1440 twice, then 3840x2160 (the
  CTA 4K VICs), 2560x1440, 1920x1200, 1920x1080 and smaller. Their rates are not in it.
- **3440x1440 at 30 Hz:** if the second 3440x1440 is 30 Hz in the same 3600x1500 total, it is 162 MHz and fits.
- **Nothing else fits** DVI and is nearer the 2.39:1 shape. 2560x1440, 1920x1200 and the 4K modes are 16:9 or 16:10,
  and 2560x1080 is not listed (185 MHz at 60 Hz in any case).
- Standard timings give no clock, so they are listed but not chosen.

## HDMI signalling, the step after

Display Core's writes for an HDMI stream (`enc1_stream_encoder_hdmi_set_stream_attribute`,
`enc2_update_hdmi_info_packet`, Linux v7.0). Each is an ordinary register write with its undo, logged before it is
made, as in the stages:
- table 4 (DIGxEncoderControl) in HDMI mode (encoder mode 3), and table 76 in HDMI mode;
- `HDMI_CONTROL`: packet generation version 1, keep-out mode, no deep colour, no scrambling and the clock channel at
  the character rate below 340 MHz, null packets filled;
- `HDMI_VBI_PACKET_CONTROL`: the general control packet sent and continued, null packets sent, no ACP;
- `HDMI_GC`: AVMUTE 0;
- the AVI infoframe as generic packet 0: `AFMT_GENERIC_HDR` and `AFMT_GENERIC_0` to `7` through the AFMT index, then
  `HDMI_GENERIC_PACKET_CONTROL0` (send and continue) and `_CONTROL1` (its line);
- no audio: `HDMI_AUDIO_INFO_SEND` and the audio clock regeneration are left as they are.

The stage-by-stage runs and stage 15's watch are its safeguard, as for stage 15.

**Built (fourth commit), behind `HDMI_SIGNALLING` (committed false), at today's 1920x1080 (VIC 16, 148.5 MHz):**
- **The stages:** the same stages, clock, pipe and surface as now, so stage numbers and switches stay as they are.
- **Stage 6 (FRONT END):** table 4 in HDMI mode, then DIG_FE_CNTL, then Display Core's HDMI writes on DIG front end A,
  each logged before it is made and undone with the stage. They sit in stage 6 because
  `enc1_stream_encoder_hdmi_set_stream_attribute` itself runs table 4 and then sets the HDMI engine. There are 18
  writes and one wait:
  - `HDMI_CONTROL`;
  - `HDMI_VBI_PACKET_CONTROL`;
  - `HDMI_GC`;
  - `AFMT_CNTL`'s audio clock (`enc1_update_generic_info_packet` sets it for any generic packet);
  - the generic packets' conflict awaited clear and then cleared, index 0;
  - the AVI header and its 8 words;
  - immediate update;
  - `HDMI_GENERIC_PACKET_CONTROL0` (send and continue) and `_CONTROL1` (line 2).
- **Tables 12 and 76:** in HDMI mode too (encoder mode 3).
- **The AVI infoframe** (CTA-861 version 2, 13 bytes):
  - RGB, with the active format as the picture;
  - full-range RGB (Q = 2), since the pixels are full range;
  - VIC 16 and 16:9.

  For 3440x1440 it carries VIC 0 and no aspect code: CTA-861 has none for 64:27 outside its VICs.
- **No audio:** the audio infoframe stays off. Display Core turns it on even without audio.
- **On the model with the Ryzen's tables** (warm state), in HDMI mode at 148.5 MHz:
  - table 12 takes the short path in 310 steps (308 in DVI) and 33 writes, none through the PHY's bus;
  - table 4 takes 33 steps and 5 writes;
  - table 76 takes 289 steps and 23 writes;
  - everything is undone to the last register.

## Progress (2026-11-22)

**Boot 77 (release 202611220908, 3b67f7528, switches 11/16/false): batch 11 ran clean.**
- **The engine:** `B11 I2C ENGINE: HELD BY NOBODY (SW REQ 0, DMCU REQ 0); LAST TRANSFER STATUS 0, DONE 0`.
- **DDC1:** enabled 1, time limit 48, prescale 480, threshold 2.
- **The time base:** 48, with an XTAL reference divider of 2 (`MICROSECOND_TIME_BASE_DIV` 0x00120230).
- **The pads:** not in AUX mode, the masks 0 (already hardware), both lines high.
- **Step 1:** `STEP 1 DONE: 11 OF 11 BATCHES READ, 305 REGISTERS`. The mirror came up at 134.9 s, as before.

**Boot 78 (release 202611220916, f13c247a8 with `EDID_ALLOWED`, switches 11/16/false): step 2 read the EDID.**
- **Both blocks** came back with their checksums good.
- **The speed:** step 2 wrote a prescale of 240 (`DC_I2C_DDC1_SPEED` 0x00F00202): 48 MHz / 2 / 100 kHz by
  `set_speed`'s formula. The firmware's 480 is its 50 kHz.
- **Given back:** `THE ENGINE GIVEN BACK: 18 WRITES PUT BACK; 6 OF 6 REGISTERS AS BATCH 11 READ THEM`, then `STEP 2
  DONE: 2 BLOCKS READ (256 BYTES)`. The mirror came up at 136.6 s, as before.
- **"THE TRANSFER ENDED IN 7 US"** was not the transfer's length.
  - The poll's clock started after the GO write's own log line, about 21 ms of pacing (60.799 to 60.820 s).
  - The transfer, about 13 ms on the wire at 100 kHz, was over by then, and the first read of the status found it
    done: the 7 us was that read.
  - The status being DONE right at the GO write is not shown by this log, nor ruled out.
  - Since the third commit, the clock is taken right after the GO write, and its line is logged after the transfer.
    The line reads `FROM GO TO THE TRANSFER'S END n US, OVER m READS OF THE STATUS 100 US APART`.
- **What the EDID says:**
  - `EDID 1.3, FNV-1A 1AC83DA3B8F9235D: TRG PRODUCT 3418, 2023, "KQ34C144C"`, with one extension.
  - `PREFERRED: 3440x1440 AT 60.000 HZ, 324000 KHZ, IN 3600x1500`; `AN HDMI SINK: TMDS AT MOST 680 MHZ`.
  - The timings in full: 3440x1440 at 60 Hz (324 MHz), 2560x1440 at 59.951 Hz (241.5 MHz), 3440x1440 at 100 Hz
    (540 MHz) and 2560x1440 at 119.817 Hz (497 MHz). None is within DVI's 165 MHz.
  - The standard timings run from 1152x864 to 1920x1200, at 60 Hz.
  - The CTA VICs are 1, 2, 3, 4, 5, 16, 17, 18, 19, 20, 31, 63, 93, 94, 95, 96 and 97.
  - `WITHIN DVI'S 165 MHZ, NO TIMING IT GIVES IN FULL IS NEARER ITS SHAPE THAN 1920x1080`.
- **So there is no smaller step.** The monitor's own mode needs HDMI signalling. The owner session asked for HDMI
  signalling first at today's 1920x1080 (VIC 16, 148.5 MHz), then the mode.

## The mode from the EDID (fifth commit)

Behind `MODE_FROM_EDID` (committed false), with `EDID_ALLOWED`. `light::mode_from_edid` takes the EDID's preferred
timing in HDMI signalling when:
- the sink has an HDMI block;
- the timing is progressive and one an OTG takes;
- its clock is within HDMI's 340 MHz without scrambling and the sink's own limit;
- its surface fits slot 5.

Otherwise it takes today's 1920x1080 in the signalling `HDMI_SIGNALLING` asks for. `display` logs which, and why.

**What follows the mode:**
- **The surface** (`picture::Size`): its pitch is rounded up to 64 pixels. For 3440x1440 that is 3456 pixels (13,824
  bytes), 19,906,560 bytes in all; a larger one is refused before any stage.
- **Stage 10's picture** is drawn at that size, its text naming it, and read back at points that follow the size.
- **Stage 13** writes the pitch field 3455, the viewport, recout and MPC size 3440x1440.
- **Stage 16** offers 3440x1440 at a pitch of 13,824 bytes.
- **At 1920x1080** everything is as before: the same samples, pitch and lines.

**On the model at 3440x1440, 324 MHz**, with boot 70's reference at 48 MHz, all 16 stages run and are undone whole:
- **Stage 9:** REF/PIX 0.1481, PER_HTOTAL 136533, H_BLANK_END 16, V_BLANK_END 57, LINE 509, REQ 9664 (54
  requests a line), next start 5988.
- **The global sync:** VSTARTUP 52 (stage 3's), VUPDATE offset 900 and width 387, VREADY 362.
- **QoS:** high watermark 2133.
- **The AVI infoframe:** VIC 0, no aspect code.

**With the Ryzen's tables** (warm state), at 324 MHz:
- table 12 takes the short path in 308 steps and 33 writes, among them the four RDPCS values for 324 MHz (0x1783C
  0xB028003E, 0x1782C 0x00010660, 0x17830 0x00000011, 0x1781C 0x80000001), none through the PHY's bus;
- table 4 takes 33 steps;
- table 76 takes 300 steps.

**Seen on the hardware:** boot 81, below.

**Boot 79 (2026-11-22, release 202611220957, 356af608b, `HDMI_SIGNALLING` and `EDID_ALLOWED` true, switches 11/16/false):
HDMI signalling at 1920x1080 went through at the first try.**
- **Step 2:** a block took `FROM GO TO THE TRANSFER'S END 12106 US, OVER 117 READS OF THE STATUS 100 US APART`, which is
  the transfer itself, about 12 ms on the wire. Both blocks were read.
- **Step 3's first line:** `MODE 1920x1080 AT 148.5 MHZ IN HDMI SIGNALLING (CTA VIC 16)`.
- **The tables match the model's counts exactly:**
  - table 12: 310 steps, 33 writes, the PHY's short path;
  - table 4: 33 steps, 5 writes;
  - table 76: 289 steps, 23 writes.
- **Stage 6's HDMI writes:** `DIG0_HDMI_CONTROL = 00010019 (WAS 00010011)`, the VBI packets 0x31, AVMUTE 0,
  `DIG0_AFMT_GENERIC_0 = 0828101F` (the AVI packet's first word), and the packet controls.
- **Stage 15:** 300 frames clean.
- **The mirror:** `SECOND SCREEN 1920X1080, PITCH 7680 … MIRRORED AT 1728X1080 FROM 96,0`; 179 writes left in place.
- **What the maintainer saw** of it is not reported yet.

**Boot 81 (2026-11-22, release 202611221107, a2ee81ca5, `EDID_ALLOWED` and `MODE_FROM_EDID` true, switches 11/16/false):
the monitor's own mode went through at the first try.**
- **The mode:** `STEP 3'S MODE: 3440x1440 AT 324000 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING`, and `MODE
  3440x1440 AT 324 MHZ IN HDMI SIGNALLING (CTA VIC 0)`.
- **The tables match the model's counts exactly:**
  - table 12: 308 steps, 33 writes, the PHY's short path;
  - table 4: 33 steps, 5 writes;
  - table 76: 300 steps, 23 writes.
- **Stage 9:** `PIPE 1 FOR 3440x1440 AT 324000 KHZ: REF/PIX 0.1481, PER HTOTAL 136533, H_BLANK_END 16, V_BLANK_END 57,
  LINE 509, REQ 9664 (54 REQUESTS A LINE)`.
- **Stage 10:** `DRAWN: 3440x1440 AT SLOT 5'S START, PITCH 3456 PIXELS (13824 BYTES); 9 SAMPLES READ BACK AS DRAWN`. Its
  drawing took tens of milliseconds at either size: the second between its lines is the stage's pause.
- **Stage 13:** `READ BACK HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 00000D7F, AS WRITTEN: FIELD 3455`.
- **Stage 15:** `WATCHED 300 FRAMES OF OTG1 IN 4985 MS (60.172 HZ)`, the longest gap between reads 14 us, then `NO
  UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES`. One pipe carries the mode, as DML's limits said.
- **Stage 16:** `[COMPOSITOR] SECOND SCREEN 3440X1440, PITCH 13824, Bgr, FROM PID 38: THE FIRST MIRRORED AT 2304X1440
  FROM 568,0`, then `STEP 3 STOPPED AFTER STAGE 16 OF 16`, 179 writes left in place.
- **What the maintainer sees** on the monitor is asked; the owner session passes on his words.

## For a successor (state on 2026-11-22)

Superseded by the section of the same name in [213-DRV-0103](213-DRV-0103-display-a-standing-underflow-check.md), which adds
0098's production order, 0096's `HOTPLUG` and 0103's standing check. What follows is the state before them.

- **Branch:** `claude/213-DRV-0095-display-the-monitors-own-mode`, made from 213-DRV-0092's head (3e68b77b9, in the
  landing queue). Code: `display/src/` (`ddc.rs` step 2, `light.rs` steps 3 and 4, `dcn.rs` registers and plan,
  `edid.rs`, `picture.rs`, `screen.rs`, `main.rs`).
- **The switches in `main.rs`,** all committed off:
  - `BATCHES_ALLOWED` (11 reads batch 11);
  - `EDID_ALLOWED` (step 2);
  - `HDMI_SIGNALLING` (HDMI at today's 1080p);
  - `MODE_FROM_EDID` (the monitor's mode, needs `EDID_ALLOWED`);
  - `STAGES_ALLOWED` (16 for all);
  - `UNDO_AFTER_STAGES`.

  The Ryzen runs used 11/16/false with the flags each run names above. Nothing runs on the machine without the owner
  session's word.
- **Host cases:**
  - `tests/display_{light,ddc,picture,batches,screen}_host.rs` and `tests/display_host.rs`;
  - with `MIND_AMD_VBIOS` naming the Ryzen's VBIOS image (not in the repository) they also run the Ryzen's own tables
    on the warm state;
  - `scripts/host_tests.sh` runs them all.
- **Hardware evidence so far:** boots 77 (batch 11), 78 (the EDID), 79 (HDMI at 1080p) and 81 (3440x1440). The logs are
  the owner session's, under its handoff directory.
- **Open:**
  - the maintainer's look at 3440x1440 (criterion 4);
  - the remaining tasks: hot-plug 213-DRV-0096, the cold path 0097, start in seconds 0098, both screens by default 0099;
  - the stretched mirror and the extended desktop, which are KRN's and APP's.

## Plan

1. **Batch 11, read-only** (first commit), then its run on the Ryzen on the owner session's word, with
   `BATCHES_ALLOWED` 11.
2. **The EDID read, step 2** (second commit), on the model; then its run, with `EDID_ALLOWED`, on the owner session's
   word.
3. **The mode, from what the read gives:**
   - the timing;
   - tables 12, 4 and 76's parameters, in HDMI signalling above 165 MHz;
   - pipe 1's DLG, TTU, sync and QoS (stage 9's formulas with the new pixel clock and width);
   - the surface's shape and pitch;
   - stage 10's picture at that size;
   - `offer_screen` with the new shape. Offering again replaces the screen.
4. **Stages and their undo as now;** nothing runs on the machine without the owner session's word.

## Acceptance criteria

- [x] Batch 11 reads HDMI-A-1's DDC line and the I2C engine, read-only and never `DC_I2C_DATA`, with host cases; steps 3
  and 4 still need only batches 1 to 10.
- [x] Batch 11's run on the Ryzen, with its values recorded here (boot 77).
- [x] Step 2 reads the EDID over DDC1 in Display Core's order and gives the engine back as found after a read and after
  every failure; host cases on a model of the engine (`display_ddc_host.rs`).
- [x] Step 2 on the Ryzen: its checksums good, its preferred timing 3440x1440 at 324 MHz as Linux's mode says, the
  engine as found after; what it says of a mode within DVI recorded here (boot 78: none).
- [ ] HDMI-A-1 in its monitor's mode on the Ryzen, the mirror on it, stage 15's watch clean, and the maintainer's look
  (MC-12.1).
  Boot 81's log shows the mode, the mirror at 2304x1440 from x 568 and the watch clean; the maintainer's look is still
  to come.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror, the
pitch), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light, table 12's two paths),
[213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md) (the paths, the monitors, the survey's hang),
[213-DRV-0096](213-DRV-0096-display-the-second-output-plugged-and-unplugged.md) (hot-plug, which reads the EDID
again).
