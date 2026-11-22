//! Step 3 of 213-DRV-0086: first light on HDMI-A-1, the engine's colour squares, in eight stages that a build lets go
//! one at a time (hwdocs/cezanne-display.md, "A route to first light"). The pipe is chosen from what step 1 read, and
//! the firmware's pipe that serves DP-2 is never written: a write goes only through `Guard`, which refuses its blocks
//! and everything off the route, and the firmware's registers are read before and after each stage. Each stage logs
//! every write it would make before it makes any; a stage that fails is undone with every stage before it, so the second
//! pipe is left off. The write path (`Poke`, `Writable`) is not step 1's read-only `Bus`: a build that may run no stage
//! makes no `Poke` at all and calls only `describe`. `tests/display_light_host.rs` checks it on a model.

use crate::atom::{self, Interpreter};
use crate::batches::{Bus, Out, Store, BOARD_PHYS};
use crate::dcn::{self, field::*, first_light, Output, Pattern, Pipe, Plan, PlanError, Protected, Reg, Signal, Step, Table, R, STAGES, STAGE_NAMES};
use crate::picture;
use crate::timing::Timing;
use core::fmt;

/// A table's run: its wall-clock limit (the waits the VBIOS leaves unbounded end here) and the time a status wait may
/// take; the time the log takes is not counted.
pub const TABLE_TIME_LIMIT_US: u64 = 500_000;
pub const POLL_US: u32 = 100;
/// After a stage's planned writes are logged and before the first is made; after a stage, before the next is logged.
pub const PAUSE_BEFORE_STAGE_MS: u32 = 1000;
pub const PAUSE_AFTER_STAGE_MS: u32 = 3000;
/// The writes step 3 can undo (table 12 alone makes 111 on the model).
pub const JOURNAL: usize = 1024;
/// The firmware's registers read before and after each stage.
pub const WATCH: usize = 64;

/// A register the guard lets step 3 write: made only by `Guard::check`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Writable(Reg);
impl Writable { pub fn reg(self) -> Reg { self.0 } }

/// The write path: only a `Writable`.
pub trait Poke { fn write(&mut self, reg: Writable, value: u32); }
/// A monotonic clock and a delay.
pub trait Clock { fn now_us(&mut self) -> u64; fn delay_us(&mut self, us: u32); }
/// Step 4's surface (BAR 0's slot 5): a row of pixels written, a pixel read back; false or None when it is not mapped.
pub trait Canvas { fn row(&mut self, y: usize, pixels: &[u32]) -> bool; fn sample(&mut self, x: usize, y: usize) -> Option<u32>; }
/// What steps 3 and 4 run on: reads, guarded writes, time, and step 4's surface.
pub trait Engine: Bus + Poke + Clock + Canvas {}
impl<T: Bus + Poke + Clock + Canvas> Engine for T {}

/// Step 4's reference (213-DRV-0092): the firmware's pipe as step 1's batches 2, 4, 9 and 10 read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference {
    pub pipe: u8, pub otg: u8,
    /// Its OTG's H_TOTAL and V_TOTAL (each the total less one), blank ends and its viewport's width.
    pub h_total: u32, pub v_total: u32, pub h_blank_end: u32, pub v_blank_end: u32, pub width: u32,
    /// Its DLG and TTU registers: REF_FREQ_TO_PIX_FREQ, REFCYC_PER_HTOTAL, REFCYC_H_BLANK_END, DLG_V_BLANK_END,
    /// REFCYC_PER_LINE_DELIVERY_L, REFCYC_PER_REQ_DELIVERY, and the PITCH field.
    pub ref_to_pix: u32, pub per_htotal: u32, pub refcyc_h_blank_end: u32, pub dlg_v_blank_end: u32, pub line_delivery: u32,
    pub req_delivery: u32, pub pitch_field: u32,
    /// The prefetch's line and request delivery (their ratio to the above is kept for pipe 1).
    pub line_delivery_pre: u32, pub req_delivery_pre: u32,
    /// The frame buffer's GPU address (DCN_VM_FB_LOCATION_BASE), for the surface's.
    pub fb_base: u64,
    /// Whether pipe 1's MPCC and its OPP's output mux are free (0xF).
    pub mpc_free: bool,
    /// The firmware's HUBP's HUBP_VREADY_AT_OR_AFTER_VSYNC (its DCHUBP_CNTL, batch 4).
    pub vready: u32,
    /// The firmware's pipe's registers stage 12 to 14 copy, in `COPIED`'s order.
    pub copies: [u32; COPIED.len()],
}

/// What pipe 1 takes from the firmware's pipe as it is: the same format, colour stages, scaler mode, requestor, VM
/// aperture, prefetch, QoS and cursor settings serve a linear 32-bit surface at 1:1 (213-DRV-0092). MPCC_CONTROL is
/// the firmware's MPCC's, the HUBP's of its own index.
pub const COPIED: [R; 38] = {
    use R::*;
    [CnvcSurfacePixelFormat, CnvcFormatControl, CmControl, CmIcscControl, CmGamutRemapControl, CmDgamControl, CmBlndgamControl,
     DsclSclMode, DsclControl, DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio, DsclLbDataFormat,
     HubpSurfaceConfig, HubpTilingConfig, HubpReqSizeConfig, HubpReqSizeConfigC, HubpreqExpansionMode, HubpretControl,
     HubpreqSurfaceControl, HubpreqFlipControl, HubpreqVmidSettings0, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh,
     HubpreqVmMxL1TlbCntl, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0, HubpreqVblankParameters1,
     HubpreqVblankParameters2, HubpreqVblankParameters3, HubpreqVblankParameters4, HubpreqTtuQosWm, HubpreqGlobalTtuCntl,
     HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1, HubpreqDstYDeltaDrqLimit, MpccControl]
};
fn copied(r: &Reference, reg: R) -> u32 { COPIED.iter().position(|&c| c == reg).map_or(0, |i| r.copies[i]) }

/// The reference from step 1's values, once all its batches were read and they name one firmware's pipe.
pub fn reference(s: &Store, f: &Firmware, c: &Choice) -> Option<Reference> {
    if s.batches() < crate::batches::BATCHES { return None; }
    let pipe = (0..4u8).find(|&n| f.hubps[n as usize])?;
    let t = f.otg;
    let mut copies = [0u32; COPIED.len()];
    for (slot, r) in copies.iter_mut().zip(COPIED) { *slot = s.get(r, pipe)?; }
    let (q, o) = (c.pipe.otg, c.pipe.opp);
    let mpc_free = s.field(R::MpccTopSel, q, MPCC_SEL)? == 0xF && s.field(R::MpcOutMux, o, MPCC_SEL)? == 0xF;
    Some(Reference {
        line_delivery_pre: s.field(R::HubpreqPerLineDeliveryPre, pipe, REFCYC_PER_LINE_DELIVERY_L)?,
        req_delivery_pre: s.field(R::HubpreqSurf0TtuCntl1, pipe, REFCYC_PER_REQ_DELIVERY)?,
        fb_base: (s.field(R::DcnVmFbLocationBase, 0, VM_16MIB)? as u64) << 24,
        mpc_free, copies, vready: s.field(R::HubpCntl, pipe, HUBP_VREADY_AT_OR_AFTER_VSYNC)?,
        pipe, otg: t,
        h_total: s.field(R::OtgHTotal, t, COUNTER_LOW)?, v_total: s.field(R::OtgVTotal, t, COUNTER_LOW)?,
        h_blank_end: s.field(R::OtgHBlankStartEnd, t, COUNTER_HIGH)?, v_blank_end: s.field(R::OtgVBlankStartEnd, t, COUNTER_HIGH)?,
        width: s.field(R::HubpPriViewportDimension, pipe, PRI_VIEWPORT_WIDTH)?,
        ref_to_pix: s.field(R::HubpreqRefFreqToPixFreq, pipe, REF_FREQ_TO_PIX_FREQ)?,
        per_htotal: s.field(R::HubpreqDstDimensions, pipe, REFCYC_PER_HTOTAL)?,
        refcyc_h_blank_end: s.field(R::HubpreqBlankOffset0, pipe, REFCYC_H_BLANK_END)?,
        dlg_v_blank_end: s.field(R::HubpreqBlankOffset0, pipe, DLG_V_BLANK_END)?,
        line_delivery: s.field(R::HubpreqPerLineDelivery, pipe, REFCYC_PER_LINE_DELIVERY_L)?,
        req_delivery: s.field(R::HubpreqSurf0TtuCntl0, pipe, REFCYC_PER_REQ_DELIVERY)?,
        pitch_field: s.field(R::HubpreqSurfacePitch, pipe, PITCH)?,
    })
}

/// What stage 9 found and derived, for the stages after it: the firmware's pixel and reference clocks, and pipe 1's
/// DLG and TTU values for step 3's mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Derived {
    pub firmware_pixel_khz: u32, pub reference_khz: u32,
    pub ref_to_pix: u32, pub per_htotal: u32, pub refcyc_h_blank_end: u32, pub dlg_v_blank_end: u32, pub line_delivery: u32,
    pub req_delivery: u32,
    /// MIN_DST_Y_NEXT_START (the blank's first line, two bits of fraction), the prefetch's delivery.
    pub min_dst_y_next_start: u32, pub line_delivery_pre: u32, pub req_delivery_pre: u32,
    /// The underflow statuses as stage 9 found them: HUBP and ODM of the firmware's pipe, then of pipe 1.
    pub underflow: [u32; 4],
}

/// The surface's place in BAR 0 (slot 5's start) and so past the frame buffer's base.
pub const SURFACE_OFFSET: u64 = 0x0100_0000;

/// What steps 3 and 4 put on the stack at most, by value: the interpreter and the journal as `Box::new` builds them before
/// they reach the heap, `Light` and its plans as they are moved between calls, step 1's values, stage 10's row. Half of
/// display's stack (linker.ld) bounds it, so that the frames' own locals keep the rest: on 2026-11-22 the 64 KiB a task
/// has by default faulted at its guard page as step 3 began (boot 69, release 202611220523).
pub const STEP3_STACK_BYTES: usize = core::mem::size_of::<Interpreter>() + core::mem::size_of::<Journal>()
    + 3 * core::mem::size_of::<Light>() + 3 * core::mem::size_of::<Plan>() + core::mem::size_of::<Store>() + 4 * picture::WIDTH;
/// The stack display asks for (linker.ld's PT_GNU_STACK).
pub const STACK_BYTES: usize = 0x40000;
const _: () = assert!(2 * STEP3_STACK_BYTES <= STACK_BYTES, "steps 3 and 4 need more stack than display asks for");

/// The mark a start of step 3 leaves in step 3's own OTG (OTG_VERTICAL_INTERRUPT2_POSITION, its interrupt off): a start
/// of display again in the same boot finds it and runs no stage. A machine boot clears OTG1's registers (step 1 read
/// them all 0 after every boot, though Linux had driven OTG1 before it), PHY A's not.
pub const RESTART_MARK: u32 = 0x5A5A;

/// How many frames stage 9 counts on the firmware's timing generator, and for at most how long.
pub const MEASURED_FRAMES: u32 = 30;
pub const MEASURE_LIMIT_US: u64 = 2_000_000;

/// The pipe and output the firmware drives DP-2 with, and the HUBPs that follow its timing generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Firmware { pub otg: u8, pub opp: u8, pub dig_fe: u8, pub phy: u8, pub hubps: [bool; 4] }
impl Firmware { pub fn protected(&self) -> Protected { Protected { otg: self.otg, opp: self.opp, dig_fe: self.dig_fe, phy: self.phy } } }

/// The pipe and output step 3 takes, and the powered HUBPs, not the firmware's, that follow its timing generator: blanked,
/// never written, read after the stages that run it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice { pub pipe: Pipe, pub output: Output, pub attached: [bool; 4] }

/// The first stage after which the attached HUBPs are read: the one that starts step 3's timing generator.
pub const ATTACHED_FROM: u8 = 5;

/// HDMI-A-1: VBIOS path 0, UNIPHY enum 1 on PHY A, HPD 1, connector HDMI type A.
pub const HDMI_A_1: Output = Output { phy: 0, hpd: 0, encoder_id: 0x1E, connector_id: 0x0C };
/// What the survey expects when step 1 has not been read: the GOP on OTG 0 and PHY E (DP-2's path 4), step 3 on OTG 1,
/// DIG front end A, PHY A, with HUBP 1 following OTG 1, blanked (as step 1 found on the Ryzen). Only for describing; a
/// stage runs only on what step 1 found.
pub const EXPECTED: (Firmware, Choice) = (
    Firmware { otg: 0, opp: 0, dig_fe: 4, phy: 4, hubps: [true, false, false, false] },
    Choice { pipe: Pipe { otg: 1, opp: 1, dig_fe: 0 }, output: HDMI_A_1, attached: [false, true, false, false] },
);

/// Why step 3 does not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// All of step 1's batches must have been read; this is the first that was not.
    NotRead { batch: u8 },
    /// Not exactly one timing generator running.
    RunningOtgs(u8),
    /// Not exactly one DIG back end of the board's PHYs enabled.
    BackEnds(u8),
    /// PHY A, HDMI-A-1's, is already driven.
    HdmiDriven,
    /// The firmware's DIG front end could not be told.
    FirmwareFrontEnd,
    /// HPD 1 does not sense a monitor on HDMI-A-1.
    NoMonitor,
    NoFreePipe,
    NoFreeFrontEnd,
    /// The chosen pipe's blocks overlap the firmware's (a DIG block both use).
    Overlap,
    /// A powered HUBP follows step 3's timing generator and is not blanked: it would fetch once the generator runs.
    AttachedHubp(u8),
    /// The display microcontroller is enabled and not held in reset: it may own the PHYs and pixel clocks the VBIOS
    /// tables would program, and Linux asks it rather than run them (`dmub_command_table`).
    DmcubRuns,
    Plan(PlanError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refusal::NotRead { batch } => write!(f, "STEP 1'S BATCH {} WAS NOT READ (STEP 3 NEEDS ALL {})", batch, crate::batches::BATCHES),
            Refusal::RunningOtgs(n) => write!(f, "{} TIMING GENERATORS RUN, NOT ONE", n),
            Refusal::BackEnds(n) => write!(f, "{} DIG BACK ENDS ARE ENABLED, NOT ONE", n),
            Refusal::HdmiDriven => f.write_str("PHY A (HDMI-A-1) IS ALREADY DRIVEN"),
            Refusal::FirmwareFrontEnd => f.write_str("THE FIRMWARE'S DIG FRONT END CANNOT BE TOLD"),
            Refusal::NoMonitor => f.write_str("HPD 1 SENSES NO MONITOR ON HDMI-A-1"),
            Refusal::NoFreePipe => f.write_str("NO FREE TIMING GENERATOR AND OPP"),
            Refusal::NoFreeFrontEnd => f.write_str("NO FREE DIG FRONT END"),
            Refusal::Overlap => f.write_str("THE CHOSEN PIPE SHARES A BLOCK WITH THE FIRMWARE'S"),
            Refusal::AttachedHubp(n) => write!(f, "HUBP{} FOLLOWS STEP 3'S TIMING GENERATOR AND IS NOT BLANKED", n),
            Refusal::DmcubRuns => f.write_str("THE DMCUB IS ENABLED AND NOT IN RESET: IT MAY OWN WHAT THE VBIOS TABLES PROGRAM"),
            Refusal::Plan(e) => write!(f, "THE PLAN: {:?}", e),
        }
    }
}

const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

/// The firmware's pipe and the pipe step 3 takes, from step 1's values; nothing assumed. All eight batches: step 3
/// reads and writes no register step 1 has not read on the same boot (batch 7 holds DCCG's pixel rate and PLL resync),
/// and it does not run while batch 8 shows the DMCUB enabled and out of reset.
pub fn assess(s: &Store) -> Result<(Firmware, Choice), Refusal> {
    if s.batches() < crate::batches::BATCHES { return Err(Refusal::NotRead { batch: s.batches() + 1 }); }
    if s.field(R::DmcubCntl, 0, DMCUB_ENABLE) != Some(0) && s.field(R::DmcubCntl, 0, DMCUB_SOFT_RESET) != Some(1) { return Err(Refusal::DmcubRuns); }
    let running = |t: u8| s.field(R::OtgControl, t, OTG_CURRENT_MASTER_EN_STATE) == Some(1);
    let runs = (0..4).filter(|&t| running(t)).count() as u8;
    let Some(otg) = (0..4).find(|&t| running(t)).filter(|_| runs == 1) else { return Err(Refusal::RunningOtgs(runs)) };
    let source = |t: u8| s.field(R::OptcDataSourceSelect, t, OPTC_SEG0_SRC_SEL).unwrap_or(0xF) as u8;
    let opp = source(otg);
    let enabled = |p: u8| s.field(R::DigBeEnCntl, p, DIG_ENABLE) == Some(1);
    let back_ends = BOARD_PHYS.iter().filter(|&&p| enabled(p)).count() as u8;
    let Some(phy) = BOARD_PHYS.iter().copied().find(|&p| enabled(p)).filter(|_| back_ends == 1) else { return Err(Refusal::BackEnds(back_ends)) };
    if phy == HDMI_A_1.phy { return Err(Refusal::HdmiDriven); }
    let fe_mask = |p: u8| s.field(R::DigBeCntl, p, DIG_FE_SOURCE_SELECT).unwrap_or(0);
    let fe_on = |d: u8| s.field(R::DigFeCntl, d, DIG_SYMCLK_FE_ON) == Some(1);
    let dig_fe = match (0..5).find(|d| fe_mask(phy) & 1 << d != 0) {
        Some(d) => d,
        None => (0..5).find(|&d| fe_on(d) && s.field(R::DigFeCntl, d, DIG_SOURCE_SELECT) == Some(otg as u32)).ok_or(Refusal::FirmwareFrontEnd)?,
    };
    let powered = |n: u8| s.field(R::DomainPgStatus, 2 * n, DOMAIN_PGFSM_PWR_STATUS) == Some(PGFSM_POWER_ON);
    let hubps = [0u8, 1, 2, 3].map(|n| powered(n) && s.field(R::HubpCntl, n, HUBP_VTG_SEL) == Some(otg as u32));
    if s.field(R::HpdIntStatus, HDMI_A_1.hpd, DC_HPD_SENSE) != Some(1) { return Err(Refusal::NoMonitor); }
    let opp_used = |o: u8| (0..4).any(|t| running(t) && source(t) == o);
    let t = (0..4).find(|&t| !running(t) && !opp_used(t)).ok_or(Refusal::NoFreePipe)?;
    let fe_used = |d: u8| d == dig_fe || d == phy || fe_on(d) || BOARD_PHYS.iter().any(|&p| enabled(p) && fe_mask(p) & 1 << d != 0);
    let d = (0..5).find(|&d| !fe_used(d)).ok_or(Refusal::NoFreeFrontEnd)?;
    let attached = [0u8, 1, 2, 3].map(|n| powered(n) && !hubps[n as usize] && s.field(R::HubpCntl, n, HUBP_VTG_SEL) == Some(t as u32));
    if let Some(n) = (0..4u8).find(|&n| attached[n as usize] && s.field(R::HubpCntl, n, HUBP_BLANK_EN) != Some(1)) { return Err(Refusal::AttachedHubp(n)); }
    Ok((Firmware { otg, opp, dig_fe, phy, hubps }, Choice { pipe: Pipe { otg: t, opp: t, dig_fe: d }, output: HDMI_A_1, attached }))
}

// --- the guard ---

/// Why the guard refused a write: the byte offset, and whose it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { Firmware(u32), OffRoute(u32) }

const fn seg1(d: u32) -> u32 { (dcn::SEGMENT[1] + d) * 4 }
const fn seg2(d: u32) -> u32 { (dcn::SEGMENT[2] + d) * 4 }

/// The register blocks of one pipe and output, as byte ranges [start, end) of BAR 5 (dcn_2_1_0_offset.h and
/// dpcs_2_1_0_offset.h, instance strides): OTG, ODM, VTG, OPP_PIPE and DPG, the DIG blocks of the front end and of the
/// PHY's back end and that DIG's `DPn_DP_LINK_CNTL` and `DPn_DP_DPHY_TRAINING_PATTERN_SEL` (table 76's DISABLE zeroes
/// both, for a TMDS output too: the off call of stage 8's undo), UNIPHY, the PHY's DPCS/RDPCS, and in DCCG the PHY's PLL
/// resync, the OTG's pixel rate and SYMCLK.
fn blocks(otg: u8, opp: u8, fe: u8, phy: u8, add: &mut impl FnMut(u32, u32)) {
    let (t, o, d, p) = (otg as u32, opp as u32, fe as u32, phy as u32);
    let mut span2 = |first: u32, n: u32| add(seg2(first), seg2(first + n));
    span2(0x1B2A + 0x80 * t, 0x80);
    span2(0x1ACA + 0x10 * t, 0x10);
    span2(0x528 + t, 1);
    span2(0x188C + 0x5A * o, 1);
    span2(0x1854 + 0x5A * o, 8);
    span2(0x2068 + 0x100 * d, 0x90);
    span2(0x2068 + 0x100 * p, 0x90);
    span2(0x2108 + 0x100 * p, 1);
    span2(0x2118 + 0x100 * p, 1);
    span2(0x286D + 2 * p, 2);
    span2(0x2928 + 0xD8 * p, 0xD8);
    let resync = if p < 4 { 0x40 + p } else { 0x4C };
    add(seg1(resync), seg1(resync + 1));
    add(seg1(0x80 + 4 * t), seg1(0x84 + 4 * t));
    add(seg1(0xA0 + p), seg1(0xA1 + p));
}

/// PHY `p`'s DPCS/RDPCS block, whose sequence the VBIOS owns.
pub fn phy_block(p: u8) -> (u32, u32) { (seg2(0x2928 + 0xD8 * p as u32), seg2(0x2928 + 0xD8 * (p as u32 + 1))) }

/// What step 3 may write: the chosen pipe's and output's blocks, never one of the firmware's.
#[derive(Clone, Copy, Debug)]
pub struct Guard { allow: [(u32, u32); 16], allows: usize, deny: [(u32, u32); 32], denies: usize, late: [(u32, u32); 4] }

impl Guard {
    pub fn new(firmware: &Firmware, choice: &Choice) -> Result<Guard, Refusal> {
        let mut g = Guard { allow: [(0, 0); 16], allows: 0, deny: [(0, 0); 32], denies: 0, late: [(0, 0); 4] };
        let mut deny = |a: u32, b: u32| { if g.denies < 32 { g.deny[g.denies] = (a, b); g.denies += 1; } };
        blocks(firmware.otg, firmware.opp, firmware.dig_fe, firmware.phy, &mut deny);
        for n in 0..4u32 {
            if firmware.hubps[n as usize] {
                deny(seg2(0x5E5 + 0xDC * n), seg2(0x5E5 + 0xDC * (n + 1))); // HUBP n
                deny(seg2(0xCC5 + 0x16B * n), seg2(0xCC5 + 0x16B * (n + 1))); // DPP n
                deny(seg2(0x1271 + 0x1B * n), seg2(0x1271 + 0x1B * (n + 1))); // MPCC n
            }
        }
        let (Pipe { otg, opp, dig_fe }, phy) = (choice.pipe, choice.output.phy);
        let mut allow = |a: u32, b: u32| { if g.allows < 16 { g.allow[g.allows] = (a, b); g.allows += 1; } };
        blocks(otg, opp, dig_fe, phy, &mut allow);
        // Step 4's (from stage 11): pipe 1's HUBP, DPP and MPCC, and its OPP's output mux (the index of its OTG).
        let (q, o) = (otg as u32, opp as u32);
        g.late = [(seg2(0x5E5 + 0xDC * q), seg2(0x5E5 + 0xDC * (q + 1))), (seg2(0xCC5 + 0x16B * q), seg2(0xCC5 + 0x16B * (q + 1))),
                  (seg2(0x1271 + 0x1B * q), seg2(0x1271 + 0x1B * (q + 1))), (seg2(0x1385 + 4 * o), seg2(0x1386 + 4 * o))];
        let overlap = g.allow[..g.allows].iter().chain(g.late.iter()).any(|&(a, b)| g.deny[..g.denies].iter().any(|&(c, d)| a < d && c < b));
        if overlap { Err(Refusal::Overlap) } else { Ok(g) }
    }

    /// A write to `reg` in stage `n`: as `check`, and from stage 11 pipe 1's HUBP, DPP, MPCC and output mux too.
    pub fn check_at(&self, reg: Reg, n: u8) -> Result<Writable, Refused> {
        match self.check(reg) {
            Err(Refused::OffRoute(o)) if n >= dcn::PIPE_WRITES_FROM && self.late.iter().any(|&(a, b)| (a..b).contains(&o)) => Ok(Writable(reg)),
            other => other,
        }
    }

    /// A write to `reg`: the firmware's blocks first, then the route's.
    pub fn check(&self, reg: Reg) -> Result<Writable, Refused> {
        let o = reg.offset();
        if self.deny[..self.denies].iter().any(|&(a, b)| (a..b).contains(&o)) { return Err(Refused::Firmware(o)); }
        if self.allow[..self.allows].iter().any(|&(a, b)| (a..b).contains(&o)) { Ok(Writable(reg)) } else { Err(Refused::OffRoute(o)) }
    }
}

// --- the firmware's registers, watched ---

/// Bits of a watched register that move by themselves, whole fields by dcn_2_1_0_sh_mask.h's masks: OTG_BUSY;
/// OPTC_DOUBLE_BUFFER_PENDING; the DIG front end's metadata-missed flag; DCHUBP_CNTL's outstanding-request (0x2, the
/// XRQ's 0xF0000), in-blank, timeout (0xF00000) and underflow (0x70000000) statuses.
fn steady(r: R) -> u32 {
    match r {
        R::OtgClockControl => !(1 << 16),
        R::OptcInputGlobalControl => !(1 << 31),
        R::DigFeCntl => !(1 << 19),
        R::HubpCntl => !(0x2 | 0x8 | 0x000F_0000 | 0x00F0_0000 | 0x7000_0000),
        _ => !0,
    }
}

/// The firmware's registers that step 1 read and step 3 watches.
#[derive(Clone, Copy, Debug)]
pub struct Watch { regs: [Option<(R, u8, Reg)>; WATCH], len: usize }

impl Watch {
    pub fn of(firmware: &Firmware) -> Watch {
        let mut w = Watch { regs: [None; WATCH], len: 0 };
        let mut push = |r: R, i: u8| if let (Some(reg), true) = (r.at(i), w.len < WATCH) { w.regs[w.len] = Some((r, i, reg)); w.len += 1; };
        let p = firmware.protected();
        // Step 1's batches 2 and 7 for the firmware's timing generator: its state, clocks, source and pixel rate.
        const BY_PIPE: [R; 21] = {
            use R::*;
            [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax, OtgVTotalControl,
             OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgClockControl, OtgVerticalInterrupt1Control,
             OtgVerticalInterrupt1Position, OtgVerticalInterrupt2Control, OtgVerticalInterrupt2Position, OptcInputGlobalControl,
             OptcDataSourceSelect, OtgPixelRateCntl, OtgPhypllPixelRateCntl]
        };
        for r in BY_PIPE { push(r, p.otg); }
        push(R::DigFeCntl, p.dig_fe);
        push(R::DigBeCntl, p.phy);
        push(R::DigBeEnCntl, p.phy);
        push(R::PhypllPixclkResyncCntl, p.phy);
        for n in 0..4u8 {
            if firmware.hubps[n as usize] {
                for r in [R::HubpreqPrimarySurfaceAddress, R::HubpreqPrimarySurfaceAddressHigh, R::HubpSurfaceConfig, R::HubpreqSurfacePitch, R::HubpCntl] { push(r, n); }
            }
        }
        w
    }
    /// The DCHUBP_CNTL of each HUBP that follows step 3's timing generator.
    pub fn attached(choice: &Choice) -> Watch {
        let mut w = Watch { regs: [None; WATCH], len: 0 };
        for n in 0..4u8 {
            if let (true, Some(reg)) = (choice.attached[n as usize], R::HubpCntl.at(n)) { w.regs[w.len] = Some((R::HubpCntl, n, reg)); w.len += 1; }
        }
        w
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    fn read(&self, bus: &mut impl Bus) -> Result<[u32; WATCH], Failure> {
        let mut v = [0u32; WATCH];
        for (slot, (r, _, reg)) in v.iter_mut().zip(self.regs[..self.len].iter().flatten()) {
            *slot = bus.read(*reg).ok_or(Failure::Unreadable(reg.offset()))? & steady(*r);
        }
        Ok(v)
    }
    fn changed(&self, before: &[u32; WATCH], after: &[u32; WATCH]) -> Option<(Reg, u32, u32)> {
        self.regs[..self.len].iter().flatten().zip(before.iter().zip(after)).find(|(_, (a, b))| a != b).map(|((_, _, reg), (a, b))| (*reg, *a, *b))
    }
}

// --- the journal ---

/// Every write step 3 made, with the value before it and its stage, for the undo.
pub struct Journal { items: [Option<(Writable, u32, u8)>; JOURNAL], len: usize }
impl Default for Journal { fn default() -> Journal { Journal { items: [None; JOURNAL], len: 0 } } }
impl Journal {
    fn push(&mut self, w: Writable, old: u32, stage: u8) -> bool {
        if self.len == JOURNAL { return false; }
        self.items[self.len] = Some((w, old, stage));
        self.len += 1;
        true
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

// --- names in the log ---

/// A register's name in the log: the map's own, else its offset.
pub struct Name(pub Reg);
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for r in dcn::ALL {
            for i in 0..r.instances() {
                if r.at(i) == Some(self.0) {
                    return match r.block() { "" if r.instances() > 1 => write!(f, "{}{} AT {:05X}", r.name(), i, self.0.offset()), "" => write!(f, "{} AT {:05X}", r.name(), self.0.offset()), b => write!(f, "{}{}_{} AT {:05X}", b, i, r.name(), self.0.offset()) };
                }
            }
        }
        write!(f, "AT {:05X}", self.0.offset())
    }
}

fn table_name(index: u8) -> &'static str {
    match index { dcn::TABLE_SETPIXELCLOCK => "SETPIXELCLOCK 1.7", dcn::TABLE_DIGXENCODERCONTROL => "DIGXENCODERCONTROL 1.5", dcn::TABLE_DIG1TRANSMITTERCONTROL => "DIG1TRANSMITTERCONTROL 1.6", _ => "?" }
}

struct Bytes<'a>(&'a [u8]);
impl fmt::Display for Bytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, b) in self.0.iter().enumerate() { if i > 0 { f.write_str(" ")?; } write!(f, "{:02X}", b)?; }
        Ok(())
    }
}

// --- the plan ---

/// Step 3 ready to run: the plan of the eight stages, the guard and the watch, for one firmware and choice.
pub struct Light {
    pub firmware: Firmware, pub choice: Choice, pub timing: Timing, plan: Plan, starts: [usize; STAGES as usize + 1], guard: Guard,
    watch: Watch, attached: Watch,
    /// Step 4's reference, when step 1 read it (stage 9 refuses without it), and the build stage 10's picture names.
    pub reference: Option<Reference>, pub build: &'static str,
}

impl Light {
    pub fn new(firmware: Firmware, choice: Choice, timing: Timing) -> Result<Light, Refusal> {
        let guard = Guard::new(&firmware, &choice)?;
        let mut plan = first_light(choice.pipe, choice.output, &timing, Signal::Dvi, Pattern::ColorSquares, firmware.protected()).map_err(Refusal::Plan)?;
        dcn::second_pipe(&mut plan).map_err(Refusal::Plan)?;
        let mut starts = [plan.len(); STAGES as usize + 1];
        for (i, s) in plan.steps().enumerate() { if let Step::Begin(n) = s { if (1..=STAGES).contains(&n) { starts[n as usize - 1] = i + 1; } } }
        Ok(Light { firmware, choice, timing, plan, starts, guard, watch: Watch::of(&firmware), attached: Watch::attached(&choice), reference: None, build: "UNKNOWN" })
    }
    /// With step 4's reference, from step 1's values.
    pub fn with_reference(mut self, r: Option<Reference>) -> Light { self.reference = r; self }
    /// With the build stage 10's picture names (02_build.sh's MIND_BUILD_COMMIT).
    pub fn with_build(mut self, build: &'static str) -> Light { self.build = build; self }
    pub fn guard(&self) -> &Guard { &self.guard }
    pub fn watch(&self) -> &Watch { &self.watch }
    pub fn attached(&self) -> &Watch { &self.attached }
    /// Stage `n`'s steps (markers left out).
    pub fn stage(&self, n: u8) -> impl Iterator<Item = Step> + '_ {
        let (a, b) = (self.starts[n as usize - 1], self.starts.get(n as usize).copied().unwrap_or(self.plan.len()).saturating_sub(if n < STAGES { 1 } else { 0 }));
        (a..b.max(a)).filter_map(|i| self.plan.get(i)).filter(|s| !matches!(s, Step::Begin(_)))
    }
    /// The VBIOS call that turns stage `n`'s part off before its writes are restored: the PHY's PLL for stage 2 (table 12
    /// with pixel clock 0, as Display Core's clock-source power-down calls it), the PHY for stage 8 (table 76's DISABLE).
    pub fn off_call(&self, n: u8) -> Option<Table> {
        let (o, d) = (self.choice.output, self.choice.pipe.dig_fe);
        match n {
            2 => Some(Table::SetPixelClock { pixclk_100hz: 0, pll_id: dcn::COMBOPHY_PLL0 + o.phy, encoder_id: 0, encoder_mode: 0, crtc_id: 0xFF, deep_color: 0, misc: 0x01 }),
            8 => Some(Table::DigTransmitter { phy: o.phy, action: dcn::TRANSMITTER_DISABLE, mode: dcn::ENCODER_MODE_DVI, lanes: 0, symclk_10khz: 0, hpd_sel: o.hpd + 1, dig_fe_sel: 1 << d, connector_id: o.connector_id }),
            _ => None,
        }
    }

    fn intro(&self, allowed: u8, expected: bool, out: &mut dyn Out) {
        let (f, c) = (self.firmware, self.choice);
        out.line(format_args!("STEP 3: FIRST LIGHT ON HDMI-A-1 IN {} STAGES, STEP 4 IN {} MORE (MIND CORE'S PICTURE); THIS BUILD MAY RUN {}",
            dcn::FIRST_LIGHT, STAGES - dcn::FIRST_LIGHT, allowed));
        out.line(format_args!("S0 THE FIRMWARE'S PIPE ({}): OTG{} OPP{} DIG {} FRONT END, PHY {}, HUBPS {}{}{}{}; NEVER WRITTEN, READ AFTER EACH STAGE ({} REGISTERS)",
            if expected { "AS THE SURVEY EXPECTS: STEP 1 NOT READ" } else { "AS STEP 1 FOUND" }, f.otg, f.opp, LETTERS[f.dig_fe as usize], LETTERS[f.phy as usize],
            f.hubps[0] as u8, f.hubps[1] as u8, f.hubps[2] as u8, f.hubps[3] as u8, self.watch.len()));
        out.line(format_args!("S0 STEP 3'S PIPE: OTG{} OPP{} DIG {} FRONT END, PHY {} (HDMI-A-1), HPD {}; MODE {}x{} AT {} MHZ IN DVI SIGNALLING (CTA VIC 16)",
            c.pipe.otg, c.pipe.opp, LETTERS[c.pipe.dig_fe as usize], LETTERS[c.output.phy as usize], c.output.hpd + 1,
            self.timing.h_active, self.timing.v_active, self.timing.pixel_khz as f32 / 1000.0));
        for n in (0..4u8).filter(|&n| c.attached[n as usize]) {
            out.line(format_args!("S0 HUBP{} FOLLOWS OTG{}, POWERED AND BLANKED: NEVER WRITTEN, ITS DCHUBP_CNTL READ AFTER EACH STAGE FROM {} ON", n, c.pipe.otg, ATTACHED_FROM));
        }
    }

    fn step_line(&self, n: u8, verb: &str, step: Step, out: &mut dyn Out) {
        match step {
            Step::Update { reg, mask, value } => out.line(format_args!("S{} {} WRITE {}: BITS {:08X} = {:08X}", n, verb, Name(reg), mask, value)),
            Step::Wait { reg, mask, value, timeout_us } => out.line(format_args!("S{} {} WAIT FOR {}: BITS {:08X} = {:08X}, AT MOST {} US", n, verb, Name(reg), mask, value, timeout_us)),
            Step::Call(t) => {
                let (p, len) = t.parameters();
                out.line(format_args!("S{} {} RUN TABLE {} ({}) WITH {} BYTES: {}", n, verb, t.index(), table_name(t.index()), len, Bytes(&p[..len])));
            }
            Step::Frames { otg, frames, timeout_us } => out.line(format_args!("S{} {} WATCH OTG{}'S FRAME COUNT MOVE {} TIMES WITHIN {} MS", n, verb, otg, frames, timeout_us / 1000)),
            Step::Reference => out.line(format_args!("S{} {} COUNT {} FRAMES OF THE FIRMWARE'S OTG{} (AT MOST {} MS) AND DERIVE PIPE {}'S DLG AND TTU FROM PIPE {}'S; NOTHING WRITTEN",
                n, verb, MEASURED_FRAMES, self.firmware.otg, MEASURE_LIMIT_US / 1000, self.choice.pipe.otg,
                self.reference.map_or(0, |r| r.pipe))),
            Step::Draw => out.line(format_args!("S{} {} DRAW {}x{} INTO SLOT 5 (BAR 0 + 0x1000000, PITCH {} PIXELS): BORDER, DIAGONALS, CROSS, ITS NAME AND BUILD {}; NO REGISTER WRITTEN",
                n, verb, picture::WIDTH, picture::HEIGHT, picture::PITCH, self.build)),
            Step::NoUnderflow => out.line(format_args!("S{} {} CHECK FOR A NEW UNDERFLOW ON BOTH PIPES", n, verb)),
            Step::Late | Step::Begin(_) => {}
        }
    }

    fn plan_lines(&self, n: u8, verb: &str, late: Option<&Plan>, out: &mut dyn Out) {
        let built = self.stage(n).any(|s| matches!(s, Step::Late));
        match (built, late) {
            (true, Some(p)) => {
                for step in dcn::steps_in(p, n) { self.step_line(n, verb, step, out); }
                if n == 13 {
                    out.line(format_args!("S{} PITCH FIELD {} ({} PIXELS LESS ONE, AS DISPLAY CORE WRITES IT); THE FIRMWARE'S HUBP{} HOLDS {}",
                        n, picture::PITCH - 1, picture::PITCH, self.reference.map_or(0, |r| r.pipe), self.reference.map_or(0, |r| r.pitch_field)));
                }
            }
            (true, None) => {
                let (q, o, f) = (self.choice.pipe.otg, self.choice.pipe.opp, self.reference.map_or(0, |r| r.pipe));
                match n {
                    11 => out.line(format_args!("S{} {} TURN ON DPP{}'S AND HUBP{}'S CLOCKS (ITS WRITES SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, q)),
                    12 => out.line(format_args!("S{} {} SET DPP{} AS DPP{}: FORMAT, COLOUR STAGES, SCALER 1:1, RECOUT {}x{} (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, f, self.timing.h_active, self.timing.v_active)),
                    13 => out.line(format_args!("S{} {} SET HUBP{}, BLANKED: THE SURFACE AT FB + 0x1000000, PITCH {}, ITS DLG AND TTU (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, picture::PITCH - 1)),
                    14 => out.line(format_args!("S{} {} CONNECT MPCC{} FROM DPP{} TO OPP{} (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, q, o)),
                    _ => out.line(format_args!("S{} {} UNBLANK HUBP{}, THEN TURN OPP{}'S PATTERN OFF; NO NEW UNDERFLOW ON EITHER PIPE (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, o)),
                }
            }
            _ => for step in self.stage(n) { self.step_line(n, verb, step, out); },
        }
        if self.stage(n).any(|s| matches!(s, Step::Call(_))) {
            out.line(format_args!("S{} ITS TABLES' WRITES ARE LOGGED AS MADE; EACH RUN STOPS AT {} STEPS OR {} MS", n, atom::MAX_STEPS, TABLE_TIME_LIMIT_US / 1000));
        }
        match self.off_call(n) {
            Some(t) => { let (p, len) = t.parameters(); out.line(format_args!("S{} UNDO: TABLE {} WITH {} BYTES {}, THEN EVERY WRITE RESTORED", n, t.index(), len, Bytes(&p[..len.min(12)]))) }
            None => out.line(format_args!("S{} UNDO: ITS WRITES RESTORED, LAST FIRST", n)),
        }
    }
}

/// A build that may run no stage: every stage named with what it would write; no device.
pub fn describe(light: &Light, expected: bool, out: &mut dyn Out) {
    light.intro(0, expected, out);
    for n in 1..=STAGES {
        out.line(format_args!("S{} {}: NOT RUN (THIS BUILD MAY RUN 0); WOULD:", n, STAGE_NAMES[n as usize - 1]));
        light.plan_lines(n, "WOULD", None, out);
    }
    out.line(format_args!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", STAGES));
}

// --- running ---

/// Why a stage failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreadable(u32),
    Refused(Refused),
    /// display started again in this boot: step 3's OTG holds the mark of a start before.
    StartedAgain,
    /// Stage 9 without step 1's batches 9 and 10 (no reference), or its measure of the firmware's frames failed.
    NoReference,
    /// Stages 11 to 15 could not be built from the reference (pipe 1's MPCC busy, or the plan).
    PipeNotFree,
    /// An underflow, new since stage 9, at this HUBP's or ODM's register.
    Underflow { offset: u32, value: u32 },
    /// Stage 10: the surface is not mapped, or a sample read back differs from what was drawn.
    NoSurface,
    SurfaceDiffers { x: u32, y: u32 },
    /// A status wait ran out.
    Timeout(u32),
    /// The timing generator's frame count did not move.
    NoFrames(u32),
    Table(atom::Error),
    FirmwareChanged { offset: u32, before: u32, after: u32 },
    /// A HUBP that follows step 3's timing generator changed (beyond its statuses): it may have left its blank.
    AttachedChanged { offset: u32, before: u32, after: u32 },
    JournalFull,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Failure::Unreadable(o) => write!(f, "{:05X} COULD NOT BE READ", o),
            Failure::StartedAgain => f.write_str("DISPLAY HAS STARTED STEP 3 ALREADY IN THIS BOOT (THE MARK IN ITS OTG): THE STAGES RUN ONCE A BOOT"),
            Failure::NoReference => f.write_str("NO REFERENCE: STEP 1'S BATCHES 9 AND 10 NOT READ, OR THE FIRMWARE'S FRAMES NOT COUNTED"),
            Failure::NoSurface => f.write_str("SLOT 5'S SURFACE IS NOT MAPPED"),
            Failure::PipeNotFree => f.write_str("PIPE 1'S MPCC OR ITS OPP'S OUTPUT MUX IS IN USE, OR ITS PLAN DOES NOT FIT"),
            Failure::Underflow { offset, value } => write!(f, "AN UNDERFLOW AT {:05X} ({:08X})", offset, value),
            Failure::SurfaceDiffers { x, y } => write!(f, "THE SURFACE READ BACK DIFFERS AT {},{}", x, y),
            Failure::Refused(Refused::Firmware(o)) => write!(f, "A WRITE TO THE FIRMWARE'S {:05X} WAS REFUSED", o),
            Failure::Refused(Refused::OffRoute(o)) => write!(f, "A WRITE OFF THE ROUTE, TO {:05X}, WAS REFUSED", o),
            Failure::Timeout(o) => write!(f, "{:05X} NEVER SHOWED ITS STATUS", o),
            Failure::NoFrames(n) => write!(f, "THE FRAME COUNT MOVED {} TIMES ONLY", n),
            Failure::Table(e) => write!(f, "THE TABLE STOPPED: {:?}", e),
            Failure::FirmwareChanged { offset, before, after } => write!(f, "THE FIRMWARE'S {:05X} CHANGED FROM {:08X} TO {:08X}", offset, before, after),
            Failure::AttachedChanged { offset, before, after } => write!(f, "THE ATTACHED HUBP'S {:05X} CHANGED FROM {:08X} TO {:08X}", offset, before, after),
            Failure::JournalFull => f.write_str("MORE WRITES THAN THE UNDO CAN HOLD"),
        }
    }
}

/// How step 3 ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Stages 1 to `done` ran; what they wrote is left in place.
    Stopped { done: u8 },
    /// Stage `stage` failed; it and every stage before it were undone.
    Failed { stage: u8, why: Failure },
    /// Stages 1 to `done` ran, then were undone, last first, as the build asks.
    Undone { done: u8 },
}

/// The interpreter's machine for step 3: reads, writes through the guard into the journal, each write logged; the time
/// the log takes is not counted against the table.
struct Adapter<'a, E: Engine> {
    engine: &'a mut E, guard: &'a Guard, journal: &'a mut Journal, out: &'a mut dyn Out,
    tag: (char, u8, u8), record: bool, writes: u32, refused: Option<Refused>, unreadable: Option<u32>, full: bool, log_us: u64,
}

impl<E: Engine> atom::Machine for Adapter<'_, E> {
    fn read(&mut self, reg: Reg) -> Option<u32> { self.engine.read(reg) }
    fn write(&mut self, reg: Reg, value: u32) -> bool {
        let w = match self.guard.check(reg) { Ok(w) => w, Err(r) => { self.refused = Some(r); return false; } };
        let Some(old) = self.engine.read(reg) else { self.unreadable = Some(reg.offset()); return false };
        if self.record && !self.journal.push(w, old, self.tag.1) { self.full = true; return false; }
        self.engine.write(w, value);
        self.writes += 1;
        let t0 = self.engine.now_us();
        self.out.line(format_args!("{}{} T{} WROTE {} = {:08X} (WAS {:08X})", self.tag.0, self.tag.1, self.tag.2, Name(reg), value, old));
        self.log_us += self.engine.now_us().saturating_sub(t0);
        true
    }
    fn delay_us(&mut self, us: u32) { self.engine.delay_us(us) }
    fn now_us(&mut self) -> u64 { self.engine.now_us().saturating_sub(self.log_us) }
}

/// Runs table `t` for stage `n`; `record` journals its writes for the undo.
fn call<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, t: Table, prefix: char, n: u8, record: bool, out: &mut dyn Out) -> Result<(), Failure> {
    let (p, len) = t.parameters();
    out.line(format_args!("{}{} RUNNING TABLE {} ({}) NOW", prefix, n, t.index(), table_name(t.index())));
    let mut a = Adapter { engine: e, guard: &light.guard, journal, out, tag: (prefix, n, t.index()), record, writes: 0, refused: None, unreadable: None, full: false, log_us: 0 };
    interp.set_time_limit(TABLE_TIME_LIMIT_US);
    let result = interp.run(t.index(), &p[..len], &mut a);
    let (writes, refused, unreadable, full) = (a.writes, a.refused, a.unreadable, a.full);
    match result {
        Ok(o) => { out.line(format_args!("{}{} TABLE {} DONE: {} STEPS, {} CALLS, {} US OF DELAYS, {} WRITES", prefix, n, t.index(), o.steps, o.calls, o.delay_us, writes)); Ok(()) }
        Err(_) if refused.is_some() => Err(Failure::Refused(refused.unwrap_or(Refused::OffRoute(0)))),
        Err(_) if full => Err(Failure::JournalFull),
        Err(_) if unreadable.is_some() => Err(Failure::Unreadable(unreadable.unwrap_or(0))),
        Err(atom::Error::Unreadable { offset, .. }) => Err(Failure::Unreadable(offset)),
        Err(err) => Err(Failure::Table(err)),
    }
}

fn read(e: &mut impl Bus, reg: Reg) -> Result<u32, Failure> { e.read(reg).ok_or(Failure::Unreadable(reg.offset())) }

fn stage<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, derived: &mut Option<Derived>,
                    late: Option<&Plan>, out: &mut dyn Out) -> Result<(), Failure> {
    let built = light.stage(n).any(|s| matches!(s, Step::Late));
    let steps = match (built, late) { (true, Some(p)) => dcn::steps_in(p, n), (true, None) => return Err(Failure::NoReference), _ => dcn::steps_in(&light.plan, n) };
    for step in steps {
        match step {
            Step::Update { reg, mask, value } => {
                let w = light.guard.check_at(reg, n).map_err(Failure::Refused)?;
                let old = read(e, reg)?;
                let new = (old & !mask) | value;
                if !journal.push(w, old, n) { return Err(Failure::JournalFull); }
                e.write(w, new);
                out.line(format_args!("S{} WROTE {} = {:08X} (WAS {:08X})", n, Name(reg), new, old));
            }
            Step::Wait { reg, mask, value, timeout_us } => {
                let start = e.now_us();
                loop {
                    if read(e, reg)? & mask == value { break; }
                    if e.now_us().saturating_sub(start) > timeout_us as u64 { return Err(Failure::Timeout(reg.offset())); }
                    e.delay_us(POLL_US);
                }
                out.line(format_args!("S{} {} SHOWED ITS STATUS IN {} US", n, Name(reg), e.now_us().saturating_sub(start)));
            }
            Step::Frames { otg, frames, timeout_us } => {
                let reg = R::OtgStatusFrameCount.at(otg).ok_or(Failure::NoFrames(0))?;
                let (first, start) = (read(e, reg)? & 0xFF_FFFF, e.now_us());
                let mut moved = 0;
                while moved < frames {
                    if e.now_us().saturating_sub(start) > timeout_us as u64 { return Err(Failure::NoFrames(moved)); }
                    e.delay_us(1000);
                    moved = (read(e, reg)?.wrapping_sub(first)) & 0xFF_FFFF;
                }
                out.line(format_args!("S{} OTG{} COUNTED {} FRAMES IN {} MS", n, otg, moved, e.now_us().saturating_sub(start) / 1000));
            }
            Step::Call(t) => {
                let from = journal.len;
                call(light, e, interp, journal, t, 'S', n, true, out)?;
                table_path(light, journal, from, t, n, out);
            }
            Step::Reference => *derived = Some(reference_stage(light, e, n, out)?),
            Step::Draw => draw_stage(light, e, n, out)?,
            Step::NoUnderflow => {
                let (Some(r), Some(d)) = (light.reference, derived.as_ref()) else { return Err(Failure::NoReference) };
                let q = light.choice.pipe.otg;
                let regs = [(R::HubpCntl, r.pipe, HUBP_UNDERFLOW_STATUS), (R::OptcInputGlobalControl, r.otg, OPTC_UNDERFLOW_OCCURRED_STATUS),
                            (R::HubpCntl, q, HUBP_UNDERFLOW_STATUS), (R::OptcInputGlobalControl, q, OPTC_UNDERFLOW_OCCURRED_STATUS)];
                for (i, (reg, inst, field)) in regs.into_iter().enumerate() {
                    let x = reg.at(inst).ok_or(Failure::NoReference)?;
                    let v = read(e, x)?;
                    if field.get(v) & !field.get(d.underflow[i]) != 0 { return Err(Failure::Underflow { offset: x.offset(), value: v }); }
                }
                out.line(format_args!("S{} NO NEW UNDERFLOW: HUBP{} AND ODM{} (THE FIRMWARE'S), HUBP{} AND ODM{}", n, r.pipe, r.otg, q, q));
            }
            Step::Late | Step::Begin(_) => {}
        }
    }
    Ok(())
}

/// Which path a table took through PHY A's control register bus: for table 12, whether the PHY was set up already (its
/// `RDPCSTX_SCRATCH` at 0x300, the VBIOS's mark) and the short path ran, or the long one with its writes through the bus.
fn table_path(light: &Light, journal: &Journal, from: usize, t: Table, n: u8, out: &mut dyn Out) {
    let p = light.choice.output.phy as u32;
    let (cr_addr, cr_data, scratch) = (seg2(0x2934 + 0xD8 * p), seg2(0x2935 + 0xD8 * p), seg2(0x2937 + 0xD8 * p));
    let made = || journal.items[from..journal.len].iter().flatten();
    let bus = made().filter(|(w, _, _)| w.reg().offset() == cr_addr || w.reg().offset() == cr_data).count();
    let data = made().filter(|(w, _, _)| w.reg().offset() == cr_data).count();
    let mark = made().find(|(w, _, _)| w.reg().offset() == scratch).map(|(_, old, _)| *old);
    match (t, mark) {
        (Table::SetPixelClock { pixclk_100hz, .. }, _) if pixclk_100hz > 0 && bus == 0 => out.line(format_args!(
            "S{} THE PHY WAS SET UP ALREADY (SCRATCH {:#X}): THE SHORT PATH, NO WRITE THROUGH ITS BUS", n, mark.unwrap_or(0))),
        (Table::SetPixelClock { pixclk_100hz, .. }, _) if pixclk_100hz > 0 => out.line(format_args!(
            "S{} THE PHY WAS NOT SET UP (SCRATCH {:#X}): THE LONG PATH: {} WRITES THROUGH ITS BUS ({} INTO THE PHY)", n, mark.unwrap_or(0), bus, data)),
        _ if bus > 0 => out.line(format_args!("S{} TABLE {} MADE {} WRITES THROUGH PHY {}'S CONTROL REGISTER BUS ({} INTO THE PHY)",
            n, t.index(), bus, LETTERS[p as usize], data)),
        _ => {}
    }
}

/// Stage 9: the firmware's timing generator counted for its pixel clock; with REF_FREQ_TO_PIX_FREQ, the reference clock;
/// the DLG and TTU formulas checked against what the firmware set for its pipe, then pipe 1's values derived for step
/// 3's mode. Nothing written.
fn reference_stage<E: Engine>(light: &Light, e: &mut E, n: u8, out: &mut dyn Out) -> Result<Derived, Failure> {
    let r = light.reference.ok_or(Failure::NoReference)?;
    out.line(format_args!("S{} PIPE {} AS THE FIRMWARE SET IT: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, LINE {}, REQ {}, PITCH FIELD {} (WIDTH {})",
        n, r.pipe, crate::batches::Ratio19(Some(r.ref_to_pix)), r.per_htotal, r.refcyc_h_blank_end, r.dlg_v_blank_end, r.line_delivery,
        r.req_delivery, r.pitch_field, r.width));
    // Its frame rate: from one frame's edge, MEASURED_FRAMES frames.
    let count = R::OtgStatusFrameCount.at(r.otg).ok_or(Failure::NoReference)?;
    let frame = |e: &mut E| read(e, count).map(|v| v & 0xFF_FFFF);
    let begin = e.now_us();
    let first = frame(e)?;
    let mut edge = first;
    while edge == first {
        if e.now_us().saturating_sub(begin) > MEASURE_LIMIT_US { return Err(Failure::NoFrames(0)); }
        e.delay_us(100);
        edge = frame(e)?;
    }
    let start = e.now_us();
    let mut frames = 0;
    while frames < MEASURED_FRAMES {
        if e.now_us().saturating_sub(begin) > MEASURE_LIMIT_US { return Err(Failure::NoFrames(frames)); }
        e.delay_us(100);
        frames = frame(e)?.wrapping_sub(edge) & 0xFF_FFFF;
    }
    let us = e.now_us().saturating_sub(start).max(1);
    let (h0, v0) = ((r.h_total + 1) as u64, (r.v_total + 1) as u64);
    let pixel_khz = (h0 * v0 * frames as u64 * 1000 / us) as u32;
    let reference_khz = ((r.ref_to_pix as u64 * pixel_khz as u64) >> 19) as u32;
    let millihertz = frames as u64 * 1_000_000_000 / us;
    out.line(format_args!("S{} OTG{} COUNTED {} FRAMES IN {} US: {}.{:03} HZ, ITS PIXEL CLOCK {} KHZ; THE REFERENCE CLOCK {} KHZ",
        n, r.otg, frames, us, millihertz / 1000, millihertz % 1000, pixel_khz, reference_khz));
    // The formulas (Display Core's dml21_rq_dlg_get_dlg_params, display_rq_dlg_calc_21.c, Linux v7.0) on the firmware's own pipe.
    let at = |ratio: u32, x: u64| ((ratio as u64 * x) >> 19) as u32;
    let (per0, hbe0, line0) = (at(r.ref_to_pix, h0 * 256), at(r.ref_to_pix, r.h_blank_end as u64), at(r.ref_to_pix, r.width as u64));
    out.line(format_args!("S{} PIPE {} BY FORMULA: PER HTOTAL {} (SET {}), H_BLANK_END {} (SET {}), LINE {} (SET {}), V_BLANK_END {} (DLG {})",
        n, r.pipe, per0, r.per_htotal, hbe0, r.refcyc_h_blank_end, line0, r.line_delivery, r.v_blank_end, r.dlg_v_blank_end));
    // Pipe 1 for step 3's mode, with the firmware's requests per line scaled to its width.
    let otg = light.timing.otg().map_err(|_| Failure::NoReference)?;
    let pix1 = light.timing.pixel_khz.max(1) as u64;
    let ratio1 = ((reference_khz as u64) << 19) / pix1;
    let h1 = otg.h_total as u64 + 1;
    let line1 = ((ratio1 * light.timing.h_active as u64) >> 19) as u32;
    let round = |a: u64, b: u64| (a + b / 2) / b.max(1);
    let requests0 = if r.req_delivery > 0 { round(r.line_delivery as u64 * 1024, r.req_delivery as u64) } else { 0 };
    let requests1 = round(requests0 * light.timing.h_active as u64, r.width.max(1) as u64).max(1);
    let req1 = (line1 as u64 * 1024 / requests1) as u32;
    // The prefetch's delivery in the same proportion to the line's as the firmware's.
    let scaled = |x: u32, num: u32, den: u32| if den == 0 { x } else { (x as u64 * num as u64 / den as u64) as u32 };
    // The underflow statuses now, so that stage 15 fails on a new one only.
    let q = light.choice.pipe.otg;
    let status = |e: &mut E, reg: R, i: u8| reg.at(i).ok_or(Failure::NoReference).and_then(|x| read(e, x));
    let underflow = [status(e, R::HubpCntl, r.pipe)?, status(e, R::OptcInputGlobalControl, r.otg)?, status(e, R::HubpCntl, q)?,
                     status(e, R::OptcInputGlobalControl, q)?];
    let d = Derived {
        firmware_pixel_khz: pixel_khz, reference_khz, ref_to_pix: ratio1 as u32, per_htotal: ((ratio1 * h1 * 256) >> 19) as u32,
        refcyc_h_blank_end: ((ratio1 * otg.h_blank_end as u64) >> 19) as u32, dlg_v_blank_end: otg.v_blank_end, line_delivery: line1,
        req_delivery: req1, min_dst_y_next_start: otg.v_blank_start * 4,
        line_delivery_pre: scaled(line1, r.line_delivery_pre, r.line_delivery), req_delivery_pre: scaled(req1, r.req_delivery_pre, r.req_delivery),
        underflow,
    };
    out.line(format_args!("S{} PIPE {} FOR {}x{} AT {} KHZ: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, LINE {}, REQ {} ({} REQUESTS A LINE)",
        n, light.choice.pipe.otg, light.timing.h_active, light.timing.v_active, pix1, crate::batches::Ratio19(Some(d.ref_to_pix)),
        d.per_htotal, d.refcyc_h_blank_end, d.dlg_v_blank_end, d.line_delivery, d.req_delivery, requests1));
    out.line(format_args!("S{} PIPE {}: NEXT START {} (V_BLANK_START {} x 4), PREFETCH LINE {} REQ {}; UNDERFLOW STATUSES {:08X} {:08X} {:08X} {:08X}",
        n, q, d.min_dst_y_next_start, otg.v_blank_start, d.line_delivery_pre, d.req_delivery_pre, underflow[0], underflow[1], underflow[2], underflow[3]));
    Ok(d)
}

/// Stages 11 to 15 for pipe 1, from the reference and what stage 9 derived, in Display Core's order (dcn20_program_pipe,
/// dcn20_update_dchubp_dpp, hubp21_setup, mpc1_insert_plane, hubp2_set_blank; Linux v7.0): its clocks; its DPP as the
/// firmware's with a 1920x1080 recout; its HUBP, still blanked, on the surface with the firmware's requestor and
/// prefetch and the derived DLG and TTU; its MPCC to its OPP; then unblanked and the pattern off.
pub fn pipe_plan(light: &Light, r: &Reference, d: &Derived) -> Result<Plan, Failure> {
    use R::*;
    if !r.mpc_free { return Err(Failure::PipeNotFree); }
    let (q, o) = (light.choice.pipe.otg, light.choice.pipe.opp);
    let (w, h) = (light.timing.h_active as u32, light.timing.v_active as u32);
    let at = |reg: R, i: u8| reg.at(i).ok_or(Failure::PipeNotFree);
    let mut p = Plan::default();
    let put = |p: &mut Plan, reg: R, i: u8, fields: &[(dcn::Field, u32)]| -> Result<(), Failure> { p.update(at(reg, i)?, fields).map_err(|_| Failure::PipeNotFree) };
    let mark = |p: &mut Plan, n: u8| p.push(Step::Begin(n)).map_err(|_| Failure::PipeNotFree);
    mark(&mut p, 11)?;
    put(&mut p, DppControl, q, &[(DPP_CLOCK_ENABLE, 1)])?;
    put(&mut p, HubpClkCntl, q, &[(HUBP_CLOCK_ENABLE, 1)])?;
    mark(&mut p, 12)?;
    for reg in [CnvcSurfacePixelFormat, CnvcFormatControl, CmControl, CmIcscControl, CmGamutRemapControl, CmDgamControl, CmBlndgamControl,
                DsclSclMode, DsclControl, DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio, DsclLbDataFormat] {
        put(&mut p, reg, q, &[(WHOLE, copied(r, reg))])?;
    }
    put(&mut p, DsclRecoutStart, q, &[(WHOLE, 0)])?;
    put(&mut p, DsclRecoutSize, q, &[(WHOLE, h << 16 | w)])?;
    put(&mut p, DsclMpcSize, q, &[(WHOLE, h << 16 | w)])?;
    mark(&mut p, 13)?;
    for reg in [HubpSurfaceConfig, HubpTilingConfig, HubpReqSizeConfig, HubpReqSizeConfigC, HubpreqExpansionMode, HubpretControl,
                HubpreqSurfaceControl, HubpreqFlipControl, HubpreqVmidSettings0, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh,
                HubpreqVmMxL1TlbCntl, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0, HubpreqVblankParameters1,
                HubpreqVblankParameters2, HubpreqVblankParameters3, HubpreqVblankParameters4, HubpreqTtuQosWm, HubpreqGlobalTtuCntl,
                HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1, HubpreqDstYDeltaDrqLimit] {
        put(&mut p, reg, q, &[(WHOLE, copied(r, reg))])?;
    }
    let surface = r.fb_base + SURFACE_OFFSET;
    put(&mut p, HubpreqSurfacePitch, q, &[(WHOLE, picture::PITCH as u32 - 1)])?;
    put(&mut p, HubpreqPrimarySurfaceAddressHigh, q, &[(WHOLE, (surface >> 32) as u32)])?;
    put(&mut p, HubpreqPrimarySurfaceAddress, q, &[(WHOLE, surface as u32)])?;
    put(&mut p, HubpPriViewportStart, q, &[(WHOLE, 0)])?;
    put(&mut p, HubpPriViewportDimension, q, &[(WHOLE, h << 16 | w)])?;
    put(&mut p, HubpreqBlankOffset0, q, &[(REFCYC_H_BLANK_END, d.refcyc_h_blank_end), (DLG_V_BLANK_END, d.dlg_v_blank_end)])?;
    put(&mut p, HubpreqBlankOffset1, q, &[(WHOLE, d.min_dst_y_next_start)])?;
    put(&mut p, HubpreqDstDimensions, q, &[(WHOLE, d.per_htotal)])?;
    put(&mut p, HubpreqRefFreqToPixFreq, q, &[(WHOLE, d.ref_to_pix)])?;
    put(&mut p, HubpreqPerLineDelivery, q, &[(WHOLE, d.line_delivery)])?;
    put(&mut p, HubpreqPerLineDeliveryPre, q, &[(WHOLE, d.line_delivery_pre)])?;
    put(&mut p, HubpreqSurf0TtuCntl0, q, &[(REFCYC_PER_REQ_DELIVERY, d.req_delivery)])?;
    put(&mut p, HubpreqSurf0TtuCntl1, q, &[(REFCYC_PER_REQ_DELIVERY, d.req_delivery_pre)])?;
    put(&mut p, HubpCntl, q, &[(HUBP_VREADY_AT_OR_AFTER_VSYNC, r.vready)])?;
    mark(&mut p, 14)?;
    put(&mut p, MpccBotSel, q, &[(WHOLE, 0xF)])?;
    put(&mut p, MpccControl, q, &[(WHOLE, copied(r, MpccControl))])?;
    put(&mut p, MpccTopSel, q, &[(WHOLE, q as u32)])?;
    put(&mut p, MpccOppId, q, &[(WHOLE, o as u32)])?;
    put(&mut p, MpcOutMux, o, &[(MPCC_SEL, q as u32)])?;
    mark(&mut p, 15)?;
    put(&mut p, HubpCntl, q, &[(HUBP_BLANK_EN, 0), (HUBP_TTU_DISABLE, 0)])?;
    p.push(Step::Frames { otg: light.choice.pipe.otg, frames: 3, timeout_us: 200_000 }).map_err(|_| Failure::PipeNotFree)?;
    p.push(Step::NoUnderflow).map_err(|_| Failure::PipeNotFree)?;
    put(&mut p, DpgControl, o, &[(DPG_EN, 0)])?;
    p.push(Step::Frames { otg: light.choice.pipe.otg, frames: 3, timeout_us: 200_000 }).map_err(|_| Failure::PipeNotFree)?;
    p.push(Step::NoUnderflow).map_err(|_| Failure::PipeNotFree)?;
    Ok(p)
}

/// The samples stage 10 reads back: the corners, the border's middle, the cross, both diagonals and the first letter.
pub const SAMPLES: [(usize, usize); 9] = [(0, 0), (1919, 1079), (5, 540), (960, 100), (178, 100), (1741, 100), (300, 100), (695, 340), (705, 340)];

/// Stage 10: the picture drawn row by row into slot 5's memory, a few pixels read back. No register written.
fn draw_stage<E: Engine>(light: &Light, e: &mut E, n: u8, out: &mut dyn Out) -> Result<(), Failure> {
    let lines = picture::Lines { build: light.build };
    let mut row = [0u32; picture::WIDTH];
    for y in 0..picture::HEIGHT {
        picture::row(&lines, y, &mut row);
        if !e.row(y, &row) { return Err(Failure::NoSurface); }
    }
    for (x, y) in SAMPLES {
        if e.sample(x, y) != Some(picture::pixel(&lines, x, y)) { return Err(Failure::SurfaceDiffers { x: x as u32, y: y as u32 }); }
    }
    out.line(format_args!("S{} DRAWN: {}x{} AT SLOT 5'S START, PITCH {} PIXELS ({} BYTES); {} SAMPLES READ BACK AS DRAWN", n,
        picture::WIDTH, picture::HEIGHT, picture::PITCH, picture::PITCH * 4, SAMPLES.len()));
    out.line(format_args!("S{} ITS TEXT: MIND CORE / DISPLAY / HDMI-A-1 1920X1080 / BUILD {}", n, light.build));
    Ok(())
}

/// Undoes stage `n`: its off call, if it has one, brings the output down in the VBIOS's own order; then every register
/// the stage wrote is put back, last first, and then every register only the off call wrote, so that all are as before
/// the stage (PHY A was off). The PHY's own control register bus (`RDPCS_TX_CR_ADDR`/`_DATA`) is left as it is: a
/// write to its data register is a write into the PHY, so putting it back would write stale values into the PHY. What
/// the stage set inside the PHY stays; the undo says how much.
fn undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, out: &mut dyn Out) {
    let mut off = Journal::default();
    if let Some(t) = light.off_call(n) {
        if let Err(why) = call(light, e, interp, &mut off, t, 'U', n, true, out) {
            out.line(format_args!("U{} TABLE {} STOPPED ({}); RESTORING STAGE {}'S WRITES ANYWAY", n, t.index(), why, n));
        }
    }
    let p = light.choice.output.phy as u32;
    let (cr_addr, cr_data) = (seg2(0x2934 + 0xD8 * p), seg2(0x2935 + 0xD8 * p));
    let (mut restored, mut addresses, mut data) = (0, 0, 0);
    let staged = |o: u32| journal.items[..journal.len].iter().flatten().any(|(w, _, s)| *s == n && w.reg().offset() == o);
    let mut i = journal.len;
    while i > 0 {
        i -= 1;
        let Some((w, old, stage)) = journal.items[i] else { continue };
        if stage != n { continue; }
        let o = w.reg().offset();
        if o == cr_addr { addresses += 1; continue; }
        if o == cr_data { data += 1; continue; }
        e.write(w, old);
        restored += 1;
        out.line(format_args!("U{} RESTORED {} = {:08X}", n, Name(w.reg()), old));
    }
    let mut j = off.len;
    while j > 0 {
        j -= 1;
        let Some((w, old, _)) = off.items[j] else { continue };
        let o = w.reg().offset();
        if o == cr_addr || o == cr_data || staged(o) { continue; }
        e.write(w, old);
        restored += 1;
        out.line(format_args!("U{} RESTORED {} = {:08X} (WRITTEN BY THE OFF CALL ONLY)", n, Name(w.reg()), old));
    }
    if addresses + data > 0 {
        out.line(format_args!("U{} PHY {}'S CONTROL REGISTER BUS LEFT AS IT IS: {} ADDRESS AND {} DATA WRITES; WHAT THEY SET INSIDE THE PHY STAYS",
            n, LETTERS[p as usize], addresses, data));
    }
    if n == 10 { out.line(format_args!("U{} THE SURFACE IS LEFT AS IT IS: NOTHING SCANS IT YET", n)); }
    out.line(format_args!("U{} {} UNDONE: {} WRITES RESTORED", n, STAGE_NAMES[n as usize - 1], restored));
}

/// Step 3: stages 1 to `allowed`, each logged with its writes before any is made, the firmware's pipe read before and
/// after; a failure undoes the stage and every one before it.
pub fn run<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, out: &mut dyn Out) -> Outcome {
    run_with(light, e, interp, journal, allowed, false, out)
}

/// Step 3 as `run`, then, when no stage failed, the stages that ran undone, last first: the undo is seen on the hardware,
/// and nothing is left for the next boot.
pub fn run_then_undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, out: &mut dyn Out) -> Outcome {
    run_with(light, e, interp, journal, allowed, true, out)
}

fn run_with<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, then_undo: bool, out: &mut dyn Out) -> Outcome {
    let allowed = allowed.min(STAGES);
    light.intro(allowed, false, out);
    if then_undo { out.line(format_args!("S0 THIS BUILD UNDOES THE STAGES IT RAN, LAST FIRST, BEFORE IT STOPS")); }
    let baseline = match light.watch.read(e) {
        Ok(v) => v,
        Err(why) => { out.line(format_args!("STEP 3 NOT RUN: THE FIRMWARE'S PIPE: {}", why)); return Outcome::Failed { stage: 0, why }; }
    };
    // Once a boot: a start of display again in this boot (init restarts a program that faults) finds the mark and runs no
    // stage; else the mark is left before stage 1, and never undone.
    if allowed > 0 {
        let Some(mark) = R::OtgVerticalInterrupt2Position.at(light.choice.pipe.otg) else { return Outcome::Failed { stage: 0, why: Failure::StartedAgain } };
        let was = match read(e, mark) { Ok(v) => v, Err(why) => { out.line(format_args!("STEP 3 NOT RUN: ITS MARK: {}", why)); return Outcome::Failed { stage: 0, why }; } };
        if was & 0x7FFF == RESTART_MARK {
            out.line(format_args!("STEP 3 NOT RUN: {} ({} HOLDS {:04X})", Failure::StartedAgain, Name(mark), was));
            return Outcome::Failed { stage: 0, why: Failure::StartedAgain };
        }
        let Ok(w) = light.guard.check(mark) else { return Outcome::Failed { stage: 0, why: Failure::StartedAgain } };
        e.write(w, RESTART_MARK);
        out.line(format_args!("S0 MARKED {} = {:04X} (WAS {:08X}): A START OF DISPLAY AGAIN IN THIS BOOT RUNS NO STAGE; A MACHINE BOOT CLEARS IT",
            Name(mark), RESTART_MARK, was));
    }
    let attached = match light.attached.read(e) {
        Ok(v) => v,
        Err(why) => { out.line(format_args!("STEP 3 NOT RUN: THE ATTACHED HUBPS: {}", why)); return Outcome::Failed { stage: 0, why }; }
    };
    let mut derived: Option<Derived> = None;
    let mut late: Option<Plan> = None;
    for n in 1..=STAGES {
        let name = STAGE_NAMES[n as usize - 1];
        if n > allowed {
            out.line(format_args!("S{} {}: NOT RUN (THIS BUILD MAY RUN {}); WOULD:", n, name, allowed));
            light.plan_lines(n, "WOULD", late.as_ref(), out);
            continue;
        }
        out.line(format_args!("S{} {}: STAGE {} OF {} NEXT; ITS WRITES:", n, name, n, STAGES));
        light.plan_lines(n, "WILL", late.as_ref(), out);
        out.pause(PAUSE_BEFORE_STAGE_MS);
        let result = stage(light, e, interp, journal, n, &mut derived, late.as_ref(), out).and_then(|()| {
            let now = light.watch.read(e)?;
            if let Some((reg, before, after)) = light.watch.changed(&baseline, &now) { return Err(Failure::FirmwareChanged { offset: reg.offset(), before, after }); }
            // Stage 9 done: stages 11 to 15 built from its reference.
            if let (9, Some(r), Some(d)) = (n, light.reference, derived) { late = Some(pipe_plan(light, &r, &d)?); }
            // The attached HUBP is guarded until step 4 takes it, from stage 11.
            if n < ATTACHED_FROM || n >= dcn::PIPE_WRITES_FROM || light.attached.is_empty() { return Ok(()); }
            let now = light.attached.read(e)?;
            match light.attached.changed(&attached, &now) { Some((reg, before, after)) => Err(Failure::AttachedChanged { offset: reg.offset(), before, after }), None => Ok(()) }
        });
        match result {
            Ok(()) => {
                out.line(format_args!("S{} {} DONE; THE FIRMWARE'S PIPE UNCHANGED ({} REGISTERS)", n, name, light.watch.len()));
                if (ATTACHED_FROM..dcn::PIPE_WRITES_FROM).contains(&n) && !light.attached.is_empty() { out.line(format_args!("S{} THE ATTACHED HUBPS UNCHANGED, BLANKED ({} REGISTERS)", n, light.attached.len())); }
            }
            Err(why) => {
                out.line(format_args!("S{} {} FAILED: {}; UNDOING STAGES {} TO 1", n, name, why, n));
                for m in (1..=n).rev() { undo(light, e, interp, journal, m, out); }
                let after = light.watch.read(e).ok();
                let same = after.map(|a| light.watch.changed(&baseline, &a).is_none());
                out.line(format_args!("STEP 3 FAILED AT STAGE {} AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE {}", n,
                    match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }));
                return Outcome::Failed { stage: n, why };
            }
        }
        out.pause(PAUSE_AFTER_STAGE_MS);
    }
    if then_undo && allowed > 0 {
        out.line(format_args!("STEP 3 RAN {} OF {} STAGES ({} WRITES); UNDOING STAGES {} TO 1 AS THIS BUILD ASKS", allowed, STAGES, journal.len, allowed));
        for m in (1..=allowed).rev() { undo(light, e, interp, journal, m, out); }
        let same = light.watch.read(e).ok().map(|a| light.watch.changed(&baseline, &a).is_none());
        out.line(format_args!("STEP 3 UNDONE AFTER STAGE {} OF {} AS THIS BUILD ASKS; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE {}", allowed, STAGES,
            match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }));
        return Outcome::Undone { done: allowed };
    }
    out.line(format_args!("STEP 3 STOPPED AFTER STAGE {} OF {} AS THIS BUILD ALLOWS; {} WRITES MADE ARE LEFT IN PLACE", allowed, STAGES, journal.len));
    Outcome::Stopped { done: allowed }
}
