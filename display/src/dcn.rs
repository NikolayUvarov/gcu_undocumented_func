//! The Cezanne Radeon's display engine, DCN 2.1, as `display` drives it (213-DRV-0086): where its registers are in
//! BAR 5, the fields first light writes, the VBIOS command tables it calls, and the order of it all, as data that a
//! host test checks against a model of the blocks. Nothing here touches hardware.
//!
//! Offsets, fields and parameter layouts are facts from Linux v7.0's AMD headers (`renoir_ip_offset.h`,
//! `asic_reg/dcn/dcn_2_1_0_offset.h`, `dcn_2_1_0_sh_mask.h`, `atomfirmware.h`; MIT); the order follows what Display
//! Core does for a TMDS stream (`dcn20_enable_stream_timing`, `optc1_program_timing`, `optc2_enable_crtc`,
//! `opp2_set_disp_pattern_generator`, the DIG encoders). hwdocs/cezanne-display.md has the details; no code is taken.

use crate::timing::{Otg, Timing, TimingError, DVI_SINGLE_LINK_KHZ, TMDS_NO_SCRAMBLING_KHZ};

/// Dword bases of the engine's register segments 0, 1 and 2 in BAR 5 (DCN_BASE).
pub const SEGMENT: [u32; 3] = [0x12, 0xC0, 0x34C0];
/// The bytes of BAR 5 that hold the engine's registers (segments 0 to 2: DCN 2.1's blocks and its DPCS).
pub const DCN_BYTES: core::ops::Range<u32> = 0x48..0x1B000;
/// Timing generators, OPPs and pipes; DIG encoders and PHYs (A to E).
pub const PIPES: u8 = 4;
pub const DIGS: u8 = 5;

/// A register, as its byte offset in BAR 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg(pub u32);

/// The registers first light reads and writes, each of one instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R {
    OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax,
    OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl, OtgInterlaceControl, OtgStatus,
    OtgStatusPosition, OtgStatusFrameCount, OtgClockControl, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam,
    OptcDataSourceSelect, OptcDataFormatControl, OptcInputClockControl, VtgControl,
    OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment,
    DigFeCntl, DigBeCntl, OtgPixelRateCntl, OtgPhypllPixelRateCntl, PhypllPixclkResyncCntl,
    HpdIntStatus, HpdControl, DentistDispclkCntl,
}

impl R {
    /// The register of instance `inst`: OTG/ODM/VTG/OPP/DPG/pixel rate by pipe, DIG by encoder, PHY PLL by PHY, HPD by
    /// pin.
    pub fn at(self, inst: u8) -> Reg {
        use R::*;
        let i = inst as u32;
        let otg = |off: u32| (2, 0x1B2A + off + 0x80 * i);
        let (segment, dword) = match self {
            OtgHTotal => otg(0x00), OtgHBlankStartEnd => otg(0x01), OtgHSyncA => otg(0x02), OtgHSyncACntl => otg(0x03),
            OtgHTimingCntl => otg(0x04), OtgVTotal => otg(0x05), OtgVTotalMin => otg(0x06), OtgVTotalMax => otg(0x07),
            OtgVBlankStartEnd => otg(0x0C), OtgVSyncA => otg(0x0D), OtgVSyncACntl => otg(0x0E), OtgControl => otg(0x17),
            OtgBlankControl => otg(0x18), OtgInterlaceControl => otg(0x1A), OtgStatus => otg(0x1F),
            OtgStatusPosition => otg(0x20), OtgStatusFrameCount => otg(0x22), OtgClockControl => otg(0x5C),
            OtgVstartupParam => otg(0x5D), OtgVupdateParam => otg(0x5E), OtgVreadyParam => otg(0x5F),
            OptcDataSourceSelect => (2, 0x1ACB + 0x10 * i), OptcDataFormatControl => (2, 0x1ACC + 0x10 * i),
            OptcInputClockControl => (2, 0x1ACF + 0x10 * i), VtgControl => (2, 0x0528 + i),
            OppPipeControl => (2, 0x188C + 0x5A * i), DpgControl => (2, 0x1854 + 0x5A * i),
            DpgDimensions => (2, 0x1856 + 0x5A * i), DpgOffsetSegment => (2, 0x185A + 0x5A * i),
            DigFeCntl => (2, 0x2068 + 0x100 * i), DigBeCntl => (2, 0x20AF + 0x100 * i),
            OtgPixelRateCntl => (1, 0x80 + 4 * i), OtgPhypllPixelRateCntl => (1, 0x83 + 4 * i),
            PhypllPixclkResyncCntl => (1, if i < 4 { 0x40 + i } else { 0x4C }),
            HpdIntStatus => (2, 0x1F14 + 8 * i), HpdControl => (2, 0x1F16 + 8 * i),
            DentistDispclkCntl => (1, 0x64),
        };
        Reg((SEGMENT[segment] + dword) * 4)
    }
}

/// A field of a register: its lowest bit and width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field { pub shift: u8, pub width: u8 }

impl Field {
    pub const fn mask(self) -> u32 { (((1u64 << self.width) - 1) as u32) << self.shift }
    pub const fn put(self, value: u32) -> u32 { (value << self.shift) & self.mask() }
    pub const fn get(self, reg: u32) -> u32 { (reg & self.mask()) >> self.shift }
}

const fn f(shift: u8, width: u8) -> Field { Field { shift, width } }

/// The fields first light uses (dcn_2_1_0_sh_mask.h).
pub mod field {
    use super::{f, Field};
    pub const COUNTER_LOW: Field = f(0, 15); // H_TOTAL, V_TOTAL(_MIN/_MAX), blank and sync starts
    pub const COUNTER_HIGH: Field = f(16, 15); // blank and sync ends
    pub const SYNC_POL: Field = f(0, 1);
    pub const OTG_MASTER_EN: Field = f(0, 1);
    pub const OTG_DISABLE_POINT_CNTL: Field = f(8, 2);
    pub const OTG_START_POINT_CNTL: Field = f(12, 1);
    pub const OTG_FIELD_NUMBER_CNTL: Field = f(13, 1);
    pub const OTG_CURRENT_MASTER_EN_STATE: Field = f(16, 1);
    pub const OTG_INTERLACE_ENABLE: Field = f(0, 1);
    pub const OTG_H_TIMING_DIV_BY2: Field = f(0, 1);
    pub const OTG_CLOCK_EN: Field = f(0, 1);
    pub const OTG_CLOCK_GATE_DIS: Field = f(1, 1);
    pub const OTG_CLOCK_ON: Field = f(8, 1);
    pub const OTG_FRAME_COUNT: Field = f(0, 24);
    pub const VSTARTUP_START: Field = f(0, 10);
    pub const VUPDATE_OFFSET: Field = f(0, 16);
    pub const VUPDATE_WIDTH: Field = f(16, 10);
    pub const VREADY_OFFSET: Field = f(0, 16);
    pub const OPTC_INPUT_CLK_GATE_DIS: Field = f(0, 1);
    pub const OPTC_INPUT_CLK_EN: Field = f(1, 1);
    pub const OPTC_INPUT_CLK_ON: Field = f(2, 1);
    pub const OPTC_SEG0_SRC_SEL: Field = f(8, 4);
    pub const OPTC_DATA_FORMAT: Field = f(0, 2);
    pub const VTG0_FP2: Field = f(0, 15);
    pub const VTG0_VCOUNT_INIT: Field = f(16, 15);
    pub const VTG0_ENABLE: Field = f(31, 1);
    pub const OPP_PIPE_CLOCK_EN: Field = f(0, 1);
    pub const DPG_EN: Field = f(0, 1);
    pub const DPG_MODE: Field = f(4, 3);
    pub const DPG_DYNAMIC_RANGE: Field = f(8, 1);
    pub const DPG_BIT_DEPTH: Field = f(12, 2);
    pub const DPG_VRES: Field = f(16, 4);
    pub const DPG_HRES: Field = f(20, 4);
    pub const DPG_ACTIVE_HEIGHT: Field = f(0, 14);
    pub const DPG_ACTIVE_WIDTH: Field = f(16, 14);
    pub const WHOLE: Field = f(0, 32);
    pub const DIG_SOURCE_SELECT: Field = f(0, 3);
    pub const DIG_START: Field = f(10, 1);
    pub const TMDS_PIXEL_ENCODING: Field = f(28, 1);
    pub const TMDS_COLOR_FORMAT: Field = f(30, 2);
    pub const DIG_FE_SOURCE_SELECT: Field = f(8, 7);
    pub const DIG_MODE: Field = f(16, 3);
    pub const PHYPLL_DEEP_COLOR_CNTL: Field = f(4, 2);
    pub const DC_HPD_SENSE: Field = f(1, 1);
    pub const DENTIST_DISPCLK_WDIVIDER: Field = f(0, 7);
}

/// VBIOS command tables first light calls, with the parameter revision DCN 2.1's VBIOS has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// `setpixelclock` 1.7: the PHY's PLL for a TMDS pixel clock, for one timing generator.
    SetPixelClock { pixclk_100hz: u32, pll_id: u8, encoder_id: u8, encoder_mode: u8, crtc_id: u8, deep_color: u8 },
    /// `digxencodercontrol` 1.5, stream setup: the DIG front end for the signal.
    DigEncoderStreamSetup { dig: u8, mode: u8, lanes: u8, pclk_10khz: u32, bpc: u8 },
    /// `dig1transmittercontrol` 1.6: a PHY (UNIPHY A-E) on or off.
    DigTransmitter { phy: u8, action: u8, mode: u8, lanes: u8, symclk_10khz: u32, hpd_sel: u8, dig_fe_sel: u8, connector_id: u8 },
}

/// The tables' indices in the master command table (`atom_master_list_of_command_functions_v2_1`).
pub const TABLE_DIGXENCODERCONTROL: u8 = 4;
pub const TABLE_SETPIXELCLOCK: u8 = 12;
pub const TABLE_DIG1TRANSMITTERCONTROL: u8 = 76;
pub const ENCODER_MODE_DVI: u8 = 2; // atom_encode_mode_def
pub const ENCODER_MODE_HDMI: u8 = 3;
pub const ENCODER_CMD_STREAM_SETUP: u8 = 0x0F; // atom_dig_encoder_control_action
pub const TRANSMITTER_DISABLE: u8 = 0; // atom_dig_transmitter_control_action
pub const TRANSMITTER_ENABLE: u8 = 1;
pub const PANEL_8BIT_PER_COLOR: u8 = 2; // atom_panel_bit_per_color
pub const COMBOPHY_PLL0: u8 = 20; // atom_ppll_def: ATOM_COMBOPHY_PLL0..4 are PHY A..E's PLLs

impl Table {
    /// The table's index in the master command table.
    pub fn index(&self) -> u8 {
        match self {
            Table::SetPixelClock { .. } => TABLE_SETPIXELCLOCK,
            Table::DigEncoderStreamSetup { .. } => TABLE_DIGXENCODERCONTROL,
            Table::DigTransmitter { .. } => TABLE_DIG1TRANSMITTERCONTROL,
        }
    }

    /// The parameter space the table is called with, little-endian, as `atomfirmware.h` lays it out
    /// (`set_pixel_clock_parameter_v1_7`, `dig_encoder_stream_setup_parameters_v1_5`,
    /// `dig_transmitter_control_ps_allocation_v1_6`): the bytes and their length.
    pub fn parameters(&self) -> ([u8; 32], usize) {
        let mut p = [0u8; 32];
        match *self {
            Table::SetPixelClock { pixclk_100hz, pll_id, encoder_id, encoder_mode, crtc_id, deep_color } => {
                p[0..4].copy_from_slice(&pixclk_100hz.to_le_bytes());
                p[4] = pll_id; p[5] = encoder_id; p[6] = encoder_mode; p[7] = 0; p[8] = crtc_id; p[9] = deep_color;
                (p, 16)
            }
            Table::DigEncoderStreamSetup { dig, mode, lanes, pclk_10khz, bpc } => {
                p[0] = dig; p[1] = ENCODER_CMD_STREAM_SETUP; p[2] = mode; p[3] = lanes;
                p[4..8].copy_from_slice(&pclk_10khz.to_le_bytes());
                p[8] = bpc;
                (p, 12)
            }
            Table::DigTransmitter { phy, action, mode, lanes, symclk_10khz, hpd_sel, dig_fe_sel, connector_id } => {
                p[0] = phy; p[1] = action; p[2] = mode; p[3] = lanes;
                p[4..8].copy_from_slice(&symclk_10khz.to_le_bytes());
                p[8] = hpd_sel; p[9] = dig_fe_sel; p[10] = connector_id;
                (p, 32)
            }
        }
    }
}

/// One step of a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Read the register, replace the bits of `mask` with `value`'s, write it back.
    Update { reg: Reg, mask: u32, value: u32 },
    /// Wait until `(register & mask) == value`, polling, for at most `timeout_us`.
    Wait { reg: Reg, mask: u32, value: u32, timeout_us: u32 },
    /// Run a VBIOS command table with these parameters.
    Call(Table),
    /// Wait for OTG `otg`'s frame counter to move `frames` times, for at most `timeout_us`: the generator runs.
    Frames { otg: u8, frames: u32, timeout_us: u32 },
}

/// The pipe a plan drives: its timing generator, OPP and DIG front end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pipe { pub otg: u8, pub opp: u8, pub dig_fe: u8 }

/// The output: its DIG back end and PHY (the same letter on DCN 2.1), its HPD pin and its VBIOS object ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    /// The PHY and DIG back end, 0 to 4 for A to E.
    pub phy: u8,
    /// The HPD pin, 0 to 4 (the table's hpd_sel is this plus one).
    pub hpd: u8,
    /// The encoder's object id byte (0x1E UNIPHY, 0x20 UNIPHY1, 0x21 UNIPHY2).
    pub encoder_id: u8,
    /// The connector's object id byte (0x0C HDMI type A).
    pub connector_id: u8,
}

/// The signal on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// DVI signalling, which every HDMI sink takes: no infoframes, no audio, at most 165 MHz on one link.
    Dvi,
    /// HDMI signalling without scrambling (up to 340 MHz).
    Hdmi,
}

/// The engine's test pattern: the OPP's display pattern generator (DPG; DCN 2.1's OTG has none of its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern { ColorSquares, VerticalBars, HorizontalBars }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    Timing(TimingError),
    /// The plan would touch the pipe or output the firmware drives.
    Protected,
    /// An instance beyond the engine's.
    NoSuchInstance,
    /// A pixel clock beyond what the signal carries.
    ClockTooHigh,
    /// More steps than a plan holds.
    Full,
}

pub const MAX_STEPS: usize = 64;

/// An ordered list of steps.
#[derive(Clone, Copy, Debug)]
pub struct Plan { steps: [Option<Step>; MAX_STEPS], len: usize }

impl Default for Plan { fn default() -> Self { Plan { steps: [None; MAX_STEPS], len: 0 } } }

impl Plan {
    pub fn steps(&self) -> impl Iterator<Item = Step> + '_ { self.steps[..self.len].iter().flatten().copied() }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    fn push(&mut self, step: Step) -> Result<(), PlanError> {
        if self.len == MAX_STEPS { return Err(PlanError::Full); }
        self.steps[self.len] = Some(step);
        self.len += 1;
        Ok(())
    }
    fn update(&mut self, reg: Reg, fields: &[(Field, u32)]) -> Result<(), PlanError> {
        let (mask, value) = fields.iter().fold((0, 0), |(m, v), (field, x)| (m | field.mask(), v | field.put(*x)));
        self.push(Step::Update { reg, mask, value })
    }
    fn wait(&mut self, reg: Reg, field: Field, value: u32, timeout_us: u32) -> Result<(), PlanError> {
        self.push(Step::Wait { reg, mask: field.mask(), value: field.put(value), timeout_us })
    }
    /// The registers the plan writes.
    pub fn written(&self) -> impl Iterator<Item = Reg> + '_ {
        self.steps().filter_map(|s| match s { Step::Update { reg, .. } => Some(reg), _ => None })
    }
}

/// What the firmware drives and must not be touched: its timing generator, OPP, DIG front end and PHY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protected { pub otg: u8, pub opp: u8, pub dig_fe: u8, pub phy: u8 }

impl Protected {
    /// The registers of the protected instances among those first light uses: read before and after, never written.
    pub fn registers(&self) -> impl Iterator<Item = Reg> + '_ {
        use R::*;
        const BY_PIPE: [R; 24] = [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal,
            OtgVTotalMin, OtgVTotalMax, OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl,
            OtgInterlaceControl, OtgClockControl, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam, OptcDataSourceSelect,
            OptcDataFormatControl, OptcInputClockControl, VtgControl, OtgPixelRateCntl, OtgPhypllPixelRateCntl];
        let pipe = BY_PIPE.iter().map(move |r| r.at(self.otg));
        let opp = [OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment].into_iter().map(move |r| r.at(self.opp));
        let dig = [DigFeCntl.at(self.dig_fe), DigBeCntl.at(self.phy), PhypllPixclkResyncCntl.at(self.phy)];
        pipe.chain(opp).chain(dig)
    }
}

/// First light: the engine's own test pattern on `output` through `pipe` at `timing`, the firmware's pipe and PHY left
/// alone. The order is Display Core's for a TMDS stream: the timing generator's clocks, the pixel clock, the timing,
/// the OPP and its pattern, the generator started and seen running, then the front end, the back end and the PHY.
/// It is a plan, not a guarantee (MC-12.3): it has run nowhere yet.
pub fn first_light(pipe: Pipe, output: Output, timing: &Timing, signal: Signal, pattern: Pattern, protected: Protected) -> Result<Plan, PlanError> {
    use field::*;
    use R::*;
    if pipe.otg >= PIPES || pipe.opp >= PIPES || pipe.dig_fe >= DIGS || output.phy >= DIGS || output.hpd >= DIGS {
        return Err(PlanError::NoSuchInstance);
    }
    if pipe.otg == protected.otg || pipe.opp == protected.opp || pipe.dig_fe == protected.dig_fe || output.phy == protected.phy {
        return Err(PlanError::Protected);
    }
    let otg = timing.otg().map_err(PlanError::Timing)?;
    let limit = match signal { Signal::Dvi => DVI_SINGLE_LINK_KHZ, Signal::Hdmi => TMDS_NO_SCRAMBLING_KHZ };
    if timing.pixel_khz > limit { return Err(PlanError::ClockTooHigh); }
    let mode = match signal { Signal::Dvi => ENCODER_MODE_DVI, Signal::Hdmi => ENCODER_MODE_HDMI };
    let (t, o, d) = (pipe.otg, pipe.opp, pipe.dig_fe);
    let mut plan = Plan::default();

    // The timing generator's clocks (optc1_enable_optc_clock).
    plan.update(OptcInputClockControl.at(t), &[(OPTC_INPUT_CLK_EN, 1), (OPTC_INPUT_CLK_GATE_DIS, 1)])?;
    plan.wait(OptcInputClockControl.at(t), OPTC_INPUT_CLK_ON, 1, 1000)?;
    plan.update(OtgClockControl.at(t), &[(OTG_CLOCK_EN, 1), (OTG_CLOCK_GATE_DIS, 1)])?;
    plan.wait(OtgClockControl.at(t), OTG_CLOCK_ON, 1, 1000)?;

    // The pixel clock: the PHY's PLL through the VBIOS, then 8-bit colour on its resync (dce112_program_pix_clk).
    plan.push(Step::Call(Table::SetPixelClock {
        pixclk_100hz: timing.pixel_khz * 10, pll_id: COMBOPHY_PLL0 + output.phy, encoder_id: output.encoder_id,
        encoder_mode: mode, crtc_id: t, deep_color: 0,
    }))?;
    plan.update(PhypllPixclkResyncCntl.at(output.phy), &[(PHYPLL_DEEP_COLOR_CNTL, 0)])?;

    // The timing (optc1_program_timing), the generator stopped.
    write_timing(&mut plan, t, &otg)?;

    // The OPP's clock and its pattern generator, before the generator starts (opp2_set_disp_pattern_generator).
    plan.update(OppPipeControl.at(o), &[(OPP_PIPE_CLOCK_EN, 1)])?;
    plan.update(DpgDimensions.at(o), &[(DPG_ACTIVE_WIDTH, timing.h_active as u32), (DPG_ACTIVE_HEIGHT, timing.v_active as u32)])?;
    plan.update(DpgOffsetSegment.at(o), &[(WHOLE, 0)])?;
    let (dpg_mode, res) = match pattern { Pattern::ColorSquares => (0, 6), Pattern::VerticalBars => (3, 0), Pattern::HorizontalBars => (4, 0) };
    plan.update(DpgControl.at(o), &[(DPG_EN, 1), (DPG_MODE, dpg_mode), (DPG_DYNAMIC_RANGE, 0), (DPG_BIT_DEPTH, 1), (DPG_VRES, res), (DPG_HRES, res)])?;

    // The generator started on its OPP (optc2_enable_crtc) and seen counting frames.
    plan.update(OptcDataSourceSelect.at(t), &[(OPTC_SEG0_SRC_SEL, o as u32)])?;
    plan.update(VtgControl.at(t), &[(VTG0_ENABLE, 1)])?;
    plan.update(OtgControl.at(t), &[(OTG_DISABLE_POINT_CNTL, 3), (OTG_MASTER_EN, 1)])?;
    plan.wait(OtgControl.at(t), OTG_CURRENT_MASTER_EN_STATE, 1, 50_000)?;
    plan.push(Step::Frames { otg: t, frames: 2, timeout_us: 200_000 })?;

    // The DIG front end on that generator, RGB 8 bits (enc1_stream_encoder_dvi_set_stream_attribute).
    plan.push(Step::Call(Table::DigEncoderStreamSetup { dig: d, mode, lanes: 4, pclk_10khz: timing.pixel_khz / 10, bpc: PANEL_8BIT_PER_COLOR }))?;
    plan.update(DigFeCntl.at(d), &[(DIG_SOURCE_SELECT, t as u32), (TMDS_PIXEL_ENCODING, 0), (TMDS_COLOR_FORMAT, 0)])?;

    // The back end's mode and front end (dcn10_link_encoder_setup, connect_dig_be_to_fe), then the PHY.
    plan.update(DigBeCntl.at(output.phy), &[(DIG_MODE, mode as u32), (DIG_FE_SOURCE_SELECT, 1 << d)])?;
    plan.push(Step::Call(Table::DigTransmitter {
        phy: output.phy, action: TRANSMITTER_ENABLE, mode, lanes: 4, symclk_10khz: timing.pixel_khz / 10,
        hpd_sel: output.hpd + 1, dig_fe_sel: 1 << d, connector_id: output.connector_id,
    }))?;
    // The front end's FIFO started once its clock runs (the DIG_START pulse of the DCN 1 stream encoder).
    plan.update(DigFeCntl.at(d), &[(DIG_START, 1)])?;
    plan.update(DigFeCntl.at(d), &[(DIG_START, 0)])?;
    Ok(plan)
}

fn write_timing(plan: &mut Plan, t: u8, otg: &Otg) -> Result<(), PlanError> {
    use field::*;
    use R::*;
    plan.update(OtgControl.at(t), &[(OTG_MASTER_EN, 0)])?;
    plan.update(OtgHTotal.at(t), &[(COUNTER_LOW, otg.h_total)])?;
    plan.update(OtgHSyncA.at(t), &[(COUNTER_LOW, 0), (COUNTER_HIGH, otg.h_sync_end)])?;
    plan.update(OtgHBlankStartEnd.at(t), &[(COUNTER_LOW, otg.h_blank_start), (COUNTER_HIGH, otg.h_blank_end)])?;
    plan.update(OtgHSyncACntl.at(t), &[(SYNC_POL, otg.h_sync_negative as u32)])?;
    plan.update(OtgVTotal.at(t), &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(OtgVTotalMin.at(t), &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(OtgVTotalMax.at(t), &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(OtgVSyncA.at(t), &[(COUNTER_LOW, 0), (COUNTER_HIGH, otg.v_sync_end)])?;
    plan.update(OtgVBlankStartEnd.at(t), &[(COUNTER_LOW, otg.v_blank_start), (COUNTER_HIGH, otg.v_blank_end)])?;
    plan.update(OtgVSyncACntl.at(t), &[(SYNC_POL, otg.v_sync_negative as u32)])?;
    plan.update(OtgInterlaceControl.at(t), &[(OTG_INTERLACE_ENABLE, 0)])?;
    plan.update(VtgControl.at(t), &[(VTG0_ENABLE, 0)])?;
    plan.update(OtgControl.at(t), &[(OTG_START_POINT_CNTL, 0), (OTG_FIELD_NUMBER_CNTL, 0)])?;
    // Global sync with no plane behind the pipe: VSTARTUP inside the vertical blank, after the sync.
    let vstartup = (otg.v_blank_end - otg.v_sync_end).clamp(1, 0x3FF);
    plan.update(OtgVstartupParam.at(t), &[(VSTARTUP_START, vstartup)])?;
    plan.update(OtgVupdateParam.at(t), &[(VUPDATE_OFFSET, 0), (VUPDATE_WIDTH, 1)])?;
    plan.update(OtgVreadyParam.at(t), &[(VREADY_OFFSET, 1)])?;
    let fp2 = (vstartup as i64 - 1 - otg.v_blank_end as i64).max(0) as u32;
    plan.update(VtgControl.at(t), &[(VTG0_FP2, fp2), (VTG0_VCOUNT_INIT, otg.vcount_init)])?;
    plan.update(OptcDataFormatControl.at(t), &[(OPTC_DATA_FORMAT, 0)])?;
    plan.update(OtgHTimingCntl.at(t), &[(OTG_H_TIMING_DIV_BY2, 0)])?;
    Ok(())
}
