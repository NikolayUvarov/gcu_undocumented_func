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
- **Linux drove 3440x1440 with two pipes** (0 and 3 combined, 213-DRV-0085). Why Display Core split it is DML's choice
  and is not checked. One pipe at 720 MHz carries 324 MHz by the table, but DML's other limits are not ported:
  - the line buffer at 3440 pixels;
  - the DET;
  - the urgent latency with both pipes fetching, 2.2 GB/s in all.

  Stage 15's watch would show an underflow.
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

## Plan

1. **Batch 11, read-only** (this commit), then its run on the Ryzen on the owner session's word, with
   `BATCHES_ALLOWED` 11.
2. **The EDID read** as its own stage, before stage 1, with its undo, host cases on a model of the I2C engine, then a
   run.
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
- [ ] Batch 11's run on the Ryzen, with its values recorded here.
- [ ] The EDID read over DDC1 on the Ryzen: its checksums good, its preferred timing 3440x1440 at 324 MHz as Linux's
  copy says; host cases on a model of the engine, the undo included.
- [ ] HDMI-A-1 in its monitor's mode on the Ryzen, the mirror on it, stage 15's watch clean, and the maintainer's look
  (MC-12.1).

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror, the
pitch), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) (first light, table 12's two paths),
[213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md) (the paths, the monitors, the survey's hang),
[213-DRV-0096](213-DRV-0096-display-the-second-output-plugged-and-unplugged.md) (hot-plug, which reads the EDID
again).
