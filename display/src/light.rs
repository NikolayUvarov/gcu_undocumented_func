//! Step 3 of 213-DRV-0086: first light on HDMI-A-1, the engine's colour squares, in eight stages that a build lets go
//! one at a time (hwdocs/cezanne-display.md, "A route to first light"). The pipe is chosen from what step 1 read, and
//! the firmware's pipe that serves DP-2 is never written: a write goes only through `Guard`, which refuses its blocks
//! and everything off the route, and the firmware's registers are read before and after each stage. Each stage logs
//! every write it would make before it makes any; a stage that fails is undone with every stage before it, so the second
//! pipe is left off. The write path (`Poke`, `Writable`) is not step 1's read-only `Bus`: a build that may run no stage
//! makes no `Poke` at all and calls only `describe`. Its order is its `Out`'s (213-DRV-0098): the survey's logs every line
//! and pauses around each stage, the production one logs a line a stage with its time and the detail of a stage that
//! fails; the reads, writes, waits and checks are the same. The output is a parameter (213-DRV-0112): HDMI-A-1 or
//! HDMI-A-2, each with its PHY, back end, HPD and DDC line. `tests/display_light_host.rs` checks it on a model.

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
pub trait Canvas { fn row(&mut self, y: usize, pitch: usize, pixels: &[u32]) -> bool; fn sample(&mut self, x: usize, y: usize, pitch: usize) -> Option<u32>; }
/// Why the compositor did not take the second screen (213-KRN-0272): no client of it was granted (it does not run), its
/// own answers (`display.wit` 1.3's errors), or the call itself failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRefused { Unavailable, NoScreen, NoMemory, Denied, BadScreen, Call }
impl fmt::Display for ScreenRefused {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ScreenRefused::Unavailable => "NO CLIENT OF IT (SLOT_DISPLAY) WAS GRANTED", ScreenRefused::NoScreen => "NO SCREEN",
            ScreenRefused::NoMemory => "NO MEMORY TO MAP IT", ScreenRefused::Denied => "DENIED", ScreenRefused::BadScreen => "THE SHAPE DOES NOT FIT THE RANGE",
            ScreenRefused::Call => "THE CALL FAILED",
        })
    }
}
/// Step 4's second screen offered to the compositor, and taken back: the surface's shape; its pixels are slot 5's.
pub trait Screens {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused>;
    fn gone(&mut self) -> Result<(), ScreenRefused>;
}
/// What steps 3 and 4 run on: reads, guarded writes, time, step 4's surface, and the compositor.
pub trait Engine: Bus + Poke + Clock + Canvas + Screens {}
impl<T: Bus + Poke + Clock + Canvas + Screens> Engine for T {}

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
    /// The MPC tree as batch 10 read it: pipe 1's MPCC (TOP_SEL, OPP_ID), its OPP's output mux, and the firmware's
    /// OPP's mux and its MPCC's BOT_SEL. The firmware wires every MPCC n from DPP n to OPP n (213-DRV-0092, boot 70).
    pub mpcc_top: u32, pub mpcc_opp: u32, pub mux: u32, pub firmware_mux: u32, pub firmware_bot: u32,
    /// REFCLK_CNTL (0: the crystal) and DCHUBBUB_GLOBAL_TIMER_CNTL (its reference divider), the reference clock's route.
    pub refclk_cntl: u32, pub timer_cntl: u32,
    /// The firmware's HUBP's HUBP_VREADY_AT_OR_AFTER_VSYNC (its DCHUBP_CNTL, batch 4).
    pub vready: u32,
    /// The firmware's OTG's global sync (OTG_VSTARTUP_PARAM, OTG_VUPDATE_PARAM, OTG_VREADY_PARAM) and its HUBP's
    /// HUBPREQ_DEBUG_DB, batch 9.
    pub sync: [u32; 3], pub debug_db: u32,
    /// Its requests' QoS level (DCN_SURF0_TTU_CNTL0's QoS_LEVEL_FIXED).
    pub qos_level: u32,
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
    if s.batches() < crate::batches::STEP3_BATCHES { return None; }
    let pipe = (0..4u8).find(|&n| f.hubps[n as usize])?;
    let t = f.otg;
    let mut copies = [0u32; COPIED.len()];
    for (slot, r) in copies.iter_mut().zip(COPIED) { *slot = s.get(r, pipe)?; }
    let (q, o) = (c.pipe.otg, c.pipe.opp);
    let sel = |r: R, i: u8| s.field(r, i, MPCC_SEL);
    Some(Reference {
        line_delivery_pre: s.field(R::HubpreqPerLineDeliveryPre, pipe, REFCYC_PER_LINE_DELIVERY_L)?,
        req_delivery_pre: s.field(R::HubpreqSurf0TtuCntl1, pipe, REFCYC_PER_REQ_DELIVERY)?,
        fb_base: (s.field(R::DcnVmFbLocationBase, 0, VM_16MIB)? as u64) << 24,
        mpcc_top: sel(R::MpccTopSel, q)?, mpcc_opp: sel(R::MpccOppId, q)?, mux: sel(R::MpcOutMux, o)?,
        firmware_mux: sel(R::MpcOutMux, f.opp)?, firmware_bot: sel(R::MpccBotSel, pipe)?,
        refclk_cntl: s.get(R::RefclkCntl, 0)?, timer_cntl: s.get(R::DchubbubGlobalTimerCntl, 0)?,
        copies, vready: s.field(R::HubpCntl, pipe, HUBP_VREADY_AT_OR_AFTER_VSYNC)?,
        sync: [s.get(R::OtgVstartupParam, t)?, s.get(R::OtgVupdateParam, t)?, s.get(R::OtgVreadyParam, t)?],
        debug_db: s.get(R::HubpreqDebugDb, pipe)?, qos_level: s.field(R::HubpreqSurf0TtuCntl0, pipe, QOS_LEVEL_FIXED)?,
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
    /// Pipe 1's OTG's global sync for its plane (VUPDATE's offset and width, VREADY's offset, in pixels), the HUBP's
    /// HUBP_VREADY_AT_OR_AFTER_VSYNC by hubp2_vready_at_or_After_vsync's rule, and its QoS high watermark.
    pub vupdate_offset: u32, pub vupdate_width: u32, pub vready_offset: u32, pub vready_after_vsync: u32, pub qos_high_wm: u32,
    /// How long the count of the firmware's frames took, from the first read of its frame count (213-DRV-0098).
    pub count_us: u64,
}

/// What hubp2_vready_at_or_After_vsync writes to HUBPREQ_DEBUG_DB before each setup: "disable_dlg_test_mode", bit 8 set.
pub const DLG_TEST_MODE_OFF: u32 = 1 << 8;
/// Display Core's QoS level for a plane's and the cursor's requests (dml21's qos_level_fixed_l and _cur0).
pub const QOS_LEVEL: u32 = 8;
/// Stage 15's watch with the pattern off, in OTG1's frames: five seconds at 60 Hz.
pub const WATCH_FRAMES: u32 = 300;
/// The watch's length when the standing check (213-DRV-0103) follows it: one second at 60 Hz, never without it.
pub const SHORT_WATCH_FRAMES: u32 = 60;
/// How often the standing check reads the two pipes' statuses after the offer.
pub const STANDING_US: u64 = 1_000_000;
/// How often it says how many checks it made, at most.
pub const STANDING_SAID_US: u64 = 600_000_000;
/// The watch's mark for a frame that passed between two reads.
const SKIPPED: u8 = 0x80;

/// The surface's place in BAR 0 (slot 5's start) and so past the frame buffer's base.
pub const SURFACE_OFFSET: u64 = 0x0100_0000;
/// Slot 5's length: [0x0100_0000, 0x0240_0000) of BAR 0 (213-KRN-0271).
pub const SURFACE_BYTES: usize = 0x0140_0000;

/// What steps 3 and 4 put on the stack at most, by value: the interpreter and the journal as `Box::new` builds them before
/// they reach the heap, `Light` and its plans as they are moved between calls, step 1's values, stage 10's row, and the
/// times and a stage's tally (213-DRV-0098; the production order's held lines are on the heap), and a plug's step 1 values
/// besides the first start's (213-DRV-0096). Half of display's stack
/// (linker.ld) bounds it, so that the frames' own locals keep the rest: on 2026-11-22 the 64 KiB a task has by default
/// faulted at its guard page as step 3 began (boot 69, release 202611220523).
pub const STEP3_STACK_BYTES: usize = core::mem::size_of::<Interpreter>() + core::mem::size_of::<Journal>()
    + 3 * core::mem::size_of::<Light>() + 3 * core::mem::size_of::<Plan>() + 2 * core::mem::size_of::<Store>() + 4 * picture::MAX_WIDTH
    + 2 * core::mem::size_of::<Times>() + 2 * core::mem::size_of::<Tally>();
/// The stack display asks for (linker.ld's PT_GNU_STACK).
pub const STACK_BYTES: usize = 0x40000;
const _: () = assert!(2 * STEP3_STACK_BYTES <= STACK_BYTES, "steps 3 and 4 need more stack than display asks for");

/// Where step 3's mode came from (213-DRV-0095).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeFrom {
    /// The EDID's preferred timing, in HDMI signalling.
    Edid,
    /// Today's mode instead: the EDID not read, the sink not HDMI, no preferred timing or a bad one, its clock over HDMI's
    /// 340 MHz without scrambling or the sink's own limit, or its surface over slot 5.
    NotRead, NotHdmi, NoPreferred, Bad, OverClock { khz: u32, limit: u32 }, Surface { bytes: usize },
}
impl fmt::Display for ModeFrom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ModeFrom::Edid => f.write_str("THE EDID'S PREFERRED TIMING"),
            ModeFrom::NotRead => f.write_str("TODAY'S: THE EDID WAS NOT READ"),
            ModeFrom::NotHdmi => f.write_str("TODAY'S: THE SINK HAS NO HDMI BLOCK"),
            ModeFrom::NoPreferred => f.write_str("TODAY'S: THE EDID HAS NO PREFERRED TIMING"),
            ModeFrom::Bad => f.write_str("TODAY'S: ITS PREFERRED TIMING IS INTERLACED OR NOT ONE AN OTG TAKES"),
            ModeFrom::OverClock { khz, limit } => write!(f, "TODAY'S: ITS PREFERRED TIMING'S {} KHZ IS OVER {} KHZ", khz, limit),
            ModeFrom::Surface { bytes } => write!(f, "TODAY'S: ITS SURFACE'S {} BYTES DO NOT FIT SLOT 5'S {}", bytes, SURFACE_BYTES),
        }
    }
}

/// The mode for step 3: the EDID's preferred timing in HDMI signalling, when the sink is HDMI and the timing fits HDMI
/// without scrambling (340 MHz), the sink's own TMDS limit, and slot 5's surface at its pitch; else `fallback` in
/// `signal`, with why.
pub fn mode_from_edid(edid: Option<&crate::edid::Edid>, fallback: Timing, signal: Signal) -> (Timing, Signal, ModeFrom) {
    let today = |why| (fallback, signal, why);
    let Some(e) = edid else { return today(ModeFrom::NotRead) };
    if e.hdmi.is_none() { return today(ModeFrom::NotHdmi); }
    let Some(p) = e.preferred() else { return today(ModeFrom::NoPreferred) };
    if p.interlaced || p.otg().is_err() { return today(ModeFrom::Bad); }
    let limit = crate::timing::TMDS_NO_SCRAMBLING_KHZ.min(e.max_tmds_khz());
    if p.pixel_khz > limit { return today(ModeFrom::OverClock { khz: p.pixel_khz, limit }); }
    let s = picture::Size::for_mode(p.h_active as usize, p.v_active as usize);
    if s.bytes() > SURFACE_BYTES || s.width > picture::MAX_WIDTH { return today(ModeFrom::Surface { bytes: s.bytes() }); }
    (p, Signal::Hdmi, ModeFrom::Edid)
}

/// The mode under the `light` setting (213-DRV-0099): the EDID decides. An HDMI sink gets its preferred timing in HDMI
/// when it fits (`mode_from_edid`); anything else `fallback`, in HDMI for an HDMI sink and in DVI otherwise (no EDID read,
/// or a sink with no HDMI block).
pub fn mode_for(edid: Option<&crate::edid::Edid>, fallback: Timing) -> (Timing, Signal, ModeFrom) {
    let signal = if edid.is_some_and(|e| e.hdmi.is_some()) { Signal::Hdmi } else { Signal::Dvi };
    mode_from_edid(edid, fallback, signal)
}

/// The mark of stages in place, which a start of step 3 leaves in step 3's own OTG (OTG_VERTICAL_INTERRUPT2_POSITION, its
/// interrupt off): a start of display again finds it and runs no stage. A failure's undo leaves it, so a failed stage
/// is not tried again; an unplug's taking back clears it (213-DRV-0096), so that a monitor plugged in lights the output
/// again. A machine boot clears OTG1's registers (step 1 read them all 0 after every boot, though Linux had driven OTG1
/// before it), PHY A's not.
pub const RESTART_MARK: u32 = 0x5A5A;

/// How many frames stage 9 counts on the firmware's timing generator at least, and for at most how long.
pub const MEASURED_FRAMES: u32 = 30;
pub const MEASURE_LIMIT_US: u64 = 2_000_000;
/// Between polls where nothing is timed, stage 9's frames between its two edges and stage 15's watch: asleep, as the
/// service shares its CPU with others (213-DRV-0111). A millisecond or more is a sleep to the clock, less is waited awake.
pub const ASLEEP_POLL_US: u32 = 1000;
/// Between polls for one of stage 9's two edges, awake for a frame at most: the count is as exact as this is short.
pub const EDGE_POLL_US: u32 = 20;

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
/// HDMI-A-2 (213-DRV-0112): VBIOS path 1, encoder 221E (UNIPHY enum 2) on PHY B, HPD 2, connector 340C (HDMI type A).
pub const HDMI_A_2: Output = Output { phy: 1, hpd: 1, encoder_id: 0x1E, connector_id: 0x0C };
/// The board's two HDMI sockets, in the VBIOS's order.
pub const SOCKETS: [Output; 2] = [HDMI_A_1, HDMI_A_2];
/// An output's name in the log, Linux's connector name.
pub fn name(o: &Output) -> &'static str { if *o == HDMI_A_1 { "HDMI-A-1" } else if *o == HDMI_A_2 { "HDMI-A-2" } else { "AN OUTPUT OFF THE BOARD" } }
/// The other HDMI socket.
pub fn other(o: &Output) -> Option<Output> { if *o == HDMI_A_1 { Some(HDMI_A_2) } else if *o == HDMI_A_2 { Some(HDMI_A_1) } else { None } }
/// What the survey expects when step 1 has not been read: the GOP on OTG 0 and PHY E (DP-2's path 4), step 3 on OTG 1,
/// DIG front end A, PHY A, with HUBP 1 following OTG 1, blanked (as step 1 found on the Ryzen). Only for describing; a
/// stage runs only on what step 1 found.
pub const EXPECTED: (Firmware, Choice) = (
    Firmware { otg: 0, opp: 0, dig_fe: 4, phy: 4, hubps: [true, false, false, false] },
    Choice { pipe: Pipe { otg: 1, opp: 1, dig_fe: 0 }, output: HDMI_A_1, attached: [false, true, false, false] },
);
/// The survey's expectation for output `o`: as `EXPECTED`, with the DIG front end of the output's own letter.
pub fn expected(o: Output) -> (Firmware, Choice) {
    let (f, c) = EXPECTED;
    (f, Choice { pipe: Pipe { dig_fe: o.phy, ..c.pipe }, output: o, ..c })
}

/// Step 3 ready for its run on HDMI-A-1, as `prepare_on`.
pub fn prepare(s: &Store, timing: Timing, signal: Signal, build: &'static str) -> Result<Light, Refusal> { prepare_on(s, HDMI_A_1, timing, signal, build) }

/// Step 3 ready for its run on `output` from step 1's values, as display starts it and as a monitor plugged in starts it
/// again (213-DRV-0096): the pipes `assess_for` finds, the plan for `timing` in `signal`, step 4's reference, the build.
pub fn prepare_on(s: &Store, output: Output, timing: Timing, signal: Signal, build: &'static str) -> Result<Light, Refusal> {
    let (f, c) = assess_for(s, output)?;
    Ok(Light::new_with(f, c, timing, signal)?.with_reference(reference(s, &f, &c)).with_build(build))
}

/// Why step 3 does not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// All of step 1's batches must have been read; this is the first that was not.
    NotRead { batch: u8 },
    /// Not exactly one timing generator running.
    RunningOtgs(u8),
    /// Not exactly one DIG back end of the board's PHYs enabled.
    BackEnds(u8),
    /// The output's PHY is already driven.
    HdmiDriven(Output),
    /// The firmware's DIG front end could not be told.
    FirmwareFrontEnd,
    /// The output's HPD does not sense a monitor.
    NoMonitor(Output),
    NoFreePipe,
    NoFreeFrontEnd,
    /// The chosen pipe's blocks overlap the firmware's (a DIG block both use).
    Overlap,
    /// The mode's surface does not fit slot 5 (213-DRV-0095): its bytes.
    Surface { bytes: usize },
    /// A powered HUBP follows step 3's timing generator and is not blanked: it would fetch once the generator runs.
    AttachedHubp(u8),
    /// The display microcontroller is enabled and not held in reset: it may own the PHYs and pixel clocks the VBIOS
    /// tables would program, and Linux asks it rather than run them (`dmub_command_table`).
    DmcubRuns,
    /// Beside display's lit pipe (213-DRV-0113): the firmware's timing generator no longer runs, or its back end is off.
    FirmwareMoved,
    /// Display's lit timing generator does not hold its mark.
    NotMarked(u8),
    /// Another timing generator holds display's mark: a pipe display does not know.
    MarkedElsewhere(u8),
    /// A timing generator runs that is neither the firmware's nor display's.
    OtherRunning(u8),
    /// A back end of the board's is enabled that is neither the firmware's nor display's lit one.
    OtherBackEnd(u8),
    Plan(PlanError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refusal::NotRead { batch } => write!(f, "STEP 1'S BATCH {} WAS NOT READ (STEP 3 NEEDS ITS FIRST {})", batch, crate::batches::STEP3_BATCHES),
            Refusal::RunningOtgs(n) => write!(f, "{} TIMING GENERATORS RUN, NOT ONE", n),
            Refusal::BackEnds(n) => write!(f, "{} DIG BACK ENDS ARE ENABLED, NOT ONE", n),
            Refusal::HdmiDriven(o) => write!(f, "PHY {} ({}) IS ALREADY DRIVEN", LETTERS[o.phy as usize % 5], name(o)),
            Refusal::FirmwareFrontEnd => f.write_str("THE FIRMWARE'S DIG FRONT END CANNOT BE TOLD"),
            Refusal::NoMonitor(o) => write!(f, "HPD {} SENSES NO MONITOR ON {}", o.hpd + 1, name(o)),
            Refusal::NoFreePipe => f.write_str("NO FREE TIMING GENERATOR AND OPP"),
            Refusal::NoFreeFrontEnd => f.write_str("NO FREE DIG FRONT END"),
            Refusal::Overlap => f.write_str("THE CHOSEN PIPE SHARES A BLOCK WITH THE FIRMWARE'S"),
            Refusal::AttachedHubp(n) => write!(f, "HUBP{} FOLLOWS STEP 3'S TIMING GENERATOR AND IS NOT BLANKED", n),
            Refusal::DmcubRuns => f.write_str("THE DMCUB IS ENABLED AND NOT IN RESET: IT MAY OWN WHAT THE VBIOS TABLES PROGRAM"),
            Refusal::FirmwareMoved => f.write_str("THE FIRMWARE'S PIPE IS NOT AS IT WAS FOUND AT START (ITS OTG STOPPED OR ITS BACK END OFF)"),
            Refusal::NotMarked(t) => write!(f, "DISPLAY'S LIT OTG{} DOES NOT HOLD ITS MARK", t),
            Refusal::MarkedElsewhere(t) => write!(f, "OTG{} HOLDS DISPLAY'S MARK, BUT IS NOT A PIPE THIS DISPLAY LIT", t),
            Refusal::OtherRunning(t) => write!(f, "OTG{} RUNS, AND IS NEITHER THE FIRMWARE'S NOR DISPLAY'S", t),
            Refusal::OtherBackEnd(p) => write!(f, "DIG BACK END {} IS ENABLED, AND IS NEITHER THE FIRMWARE'S NOR DISPLAY'S", LETTERS[*p as usize % 5]),
            Refusal::Plan(e) => write!(f, "THE PLAN: {:?}", e),
            Refusal::Surface { bytes } => write!(f, "THE MODE'S SURFACE, {} BYTES, DOES NOT FIT SLOT 5'S {}", bytes, SURFACE_BYTES),
        }
    }
}

const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

/// The firmware's pipe and the pipe step 3 takes for HDMI-A-1, as `assess_for`.
pub fn assess(s: &Store) -> Result<(Firmware, Choice), Refusal> { assess_for(s, HDMI_A_1) }

/// The firmware's pipe and the pipe step 3 takes for `output`, from step 1's values; nothing assumed. Step 1's first ten
/// batches must have been read, and step 3 does not run while batch 8 shows the DMCUB enabled and out of reset. The
/// front end is the output's own letter's when free, else the lowest free one (213-DRV-0112; for HDMI-A-1 the same as
/// before, A being the lowest). The stages also read registers step 1 does not (each written one before its write, the
/// frame counts, the statuses they wait for).
pub fn assess_for(s: &Store, output: Output) -> Result<(Firmware, Choice), Refusal> {
    if s.batches() < crate::batches::STEP3_BATCHES { return Err(Refusal::NotRead { batch: s.batches() + 1 }); }
    if s.field(R::DmcubCntl, 0, DMCUB_ENABLE) != Some(0) && s.field(R::DmcubCntl, 0, DMCUB_SOFT_RESET) != Some(1) { return Err(Refusal::DmcubRuns); }
    let running = |t: u8| s.field(R::OtgControl, t, OTG_CURRENT_MASTER_EN_STATE) == Some(1);
    let runs = (0..4).filter(|&t| running(t)).count() as u8;
    let Some(otg) = (0..4).find(|&t| running(t)).filter(|_| runs == 1) else { return Err(Refusal::RunningOtgs(runs)) };
    let source = |t: u8| s.field(R::OptcDataSourceSelect, t, OPTC_SEG0_SRC_SEL).unwrap_or(0xF) as u8;
    let opp = source(otg);
    let enabled = |p: u8| s.field(R::DigBeEnCntl, p, DIG_ENABLE) == Some(1);
    let back_ends = BOARD_PHYS.iter().filter(|&&p| enabled(p)).count() as u8;
    let Some(phy) = BOARD_PHYS.iter().copied().find(|&p| enabled(p)).filter(|_| back_ends == 1) else { return Err(Refusal::BackEnds(back_ends)) };
    if phy == output.phy { return Err(Refusal::HdmiDriven(output)); }
    let fe_mask = |p: u8| s.field(R::DigBeCntl, p, DIG_FE_SOURCE_SELECT).unwrap_or(0);
    let fe_on = |d: u8| s.field(R::DigFeCntl, d, DIG_SYMCLK_FE_ON) == Some(1);
    let dig_fe = match (0..5).find(|d| fe_mask(phy) & 1 << d != 0) {
        Some(d) => d,
        None => (0..5).find(|&d| fe_on(d) && s.field(R::DigFeCntl, d, DIG_SOURCE_SELECT) == Some(otg as u32)).ok_or(Refusal::FirmwareFrontEnd)?,
    };
    let powered = |n: u8| s.field(R::DomainPgStatus, 2 * n, DOMAIN_PGFSM_PWR_STATUS) == Some(PGFSM_POWER_ON);
    let hubps = [0u8, 1, 2, 3].map(|n| powered(n) && s.field(R::HubpCntl, n, HUBP_VTG_SEL) == Some(otg as u32));
    if s.field(R::HpdIntStatus, output.hpd, DC_HPD_SENSE) != Some(1) { return Err(Refusal::NoMonitor(output)); }
    let opp_used = |o: u8| (0..4).any(|t| running(t) && source(t) == o);
    let t = (0..4).find(|&t| !running(t) && !opp_used(t)).ok_or(Refusal::NoFreePipe)?;
    let fe_used = |d: u8| d == dig_fe || d == phy || fe_on(d) || BOARD_PHYS.iter().any(|&p| enabled(p) && fe_mask(p) & 1 << d != 0);
    let d = Some(output.phy).filter(|&d| d < 5 && !fe_used(d)).or_else(|| (0..5).find(|&d| !fe_used(d))).ok_or(Refusal::NoFreeFrontEnd)?;
    let attached = [0u8, 1, 2, 3].map(|n| powered(n) && !hubps[n as usize] && s.field(R::HubpCntl, n, HUBP_VTG_SEL) == Some(t as u32));
    if let Some(n) = (0..4u8).find(|&n| attached[n as usize] && s.field(R::HubpCntl, n, HUBP_BLANK_EN) != Some(1)) { return Err(Refusal::AttachedHubp(n)); }
    Ok((Firmware { otg, opp, dig_fe, phy, hubps }, Choice { pipe: Pipe { otg: t, opp: t, dig_fe: d }, output, attached }))
}

/// The pipe step 3 takes for `output` beside `lit`, the pipe display has lit already (213-DRV-0113), from step 1's values read
/// again: the firmware's pipe as `firmware` found it at start, kept; display's own told by its mark in its OTG. Refused
/// where step 1 does not find them so, or as `assess_for` refuses. The timing generator is the lowest free one, the front
/// end the output's own letter's when free.
pub fn assess_beside(s: &Store, output: Output, firmware: &Firmware, lit: &Choice) -> Result<(Firmware, Choice), Refusal> {
    if s.batches() < crate::batches::STEP3_BATCHES { return Err(Refusal::NotRead { batch: s.batches() + 1 }); }
    if s.field(R::DmcubCntl, 0, DMCUB_ENABLE) != Some(0) && s.field(R::DmcubCntl, 0, DMCUB_SOFT_RESET) != Some(1) { return Err(Refusal::DmcubRuns); }
    let running = |t: u8| s.field(R::OtgControl, t, OTG_CURRENT_MASTER_EN_STATE) == Some(1);
    let marked = |t: u8| s.get(R::OtgVerticalInterrupt2Position, t).map(|v| v & 0x7FFF == RESTART_MARK);
    let (fw, ours) = (firmware.otg, lit.pipe.otg);
    if !running(fw) { return Err(Refusal::FirmwareMoved); }
    if !running(ours) || marked(ours) != Some(true) { return Err(Refusal::NotMarked(ours)); }
    if let Some(t) = (0..4).find(|&t| t != ours && marked(t) == Some(true)) { return Err(Refusal::MarkedElsewhere(t)); }
    if let Some(t) = (0..4).find(|&t| t != fw && t != ours && running(t)) { return Err(Refusal::OtherRunning(t)); }
    let enabled = |p: u8| s.field(R::DigBeEnCntl, p, DIG_ENABLE) == Some(1);
    if !enabled(firmware.phy) { return Err(Refusal::FirmwareMoved); }
    if enabled(output.phy) { return Err(Refusal::HdmiDriven(output)); }
    if let Some(p) = BOARD_PHYS.iter().copied().find(|&p| enabled(p) && p != firmware.phy && p != lit.output.phy) { return Err(Refusal::OtherBackEnd(p)); }
    if s.field(R::HpdIntStatus, output.hpd, DC_HPD_SENSE) != Some(1) { return Err(Refusal::NoMonitor(output)); }
    let source = |t: u8| s.field(R::OptcDataSourceSelect, t, OPTC_SEG0_SRC_SEL).unwrap_or(0xF) as u8;
    let opp_used = |o: u8| (0..4).any(|t| running(t) && source(t) == o);
    let t = (0..4).find(|&t| !running(t) && !opp_used(t) && marked(t) != Some(true)).ok_or(Refusal::NoFreePipe)?;
    let fe_mask = |p: u8| s.field(R::DigBeCntl, p, DIG_FE_SOURCE_SELECT).unwrap_or(0);
    let fe_on = |d: u8| s.field(R::DigFeCntl, d, DIG_SYMCLK_FE_ON) == Some(1);
    let fe_used = |d: u8| d == firmware.dig_fe || d == lit.pipe.dig_fe || d == firmware.phy || d == lit.output.phy || fe_on(d)
        || BOARD_PHYS.iter().any(|&p| enabled(p) && fe_mask(p) & 1 << d != 0);
    let d = Some(output.phy).filter(|&d| d < 5 && !fe_used(d)).or_else(|| (0..5).find(|&d| !fe_used(d))).ok_or(Refusal::NoFreeFrontEnd)?;
    let powered = |n: u8| s.field(R::DomainPgStatus, 2 * n, DOMAIN_PGFSM_PWR_STATUS) == Some(PGFSM_POWER_ON);
    let attached = [0u8, 1, 2, 3].map(|n| powered(n) && !firmware.hubps[n as usize] && n != ours && s.field(R::HubpCntl, n, HUBP_VTG_SEL) == Some(t as u32));
    if let Some(n) = (0..4u8).find(|&n| attached[n as usize] && s.field(R::HubpCntl, n, HUBP_BLANK_EN) != Some(1)) { return Err(Refusal::AttachedHubp(n)); }
    Ok((*firmware, Choice { pipe: Pipe { otg: t, opp: t, dig_fe: d }, output, attached }))
}

/// Step 3 ready for `output` beside `first`, display's lit run (213-DRV-0113): `assess_beside`'s pipe, guarded against
/// `first`'s, which is read after each stage.
pub fn prepare_beside(s: &Store, output: Output, first: &Light, timing: Timing, signal: Signal, build: &'static str) -> Result<Light, Refusal> {
    let (f, c) = assess_beside(s, output, &first.firmware, &first.choice)?;
    Ok(Light::new_with(f, c, timing, signal)?.beside(&first.choice)?.with_reference(reference(s, &f, &c)).with_build(build))
}

// --- the guard ---

/// Why the guard refused a write: the byte offset, and whose it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { Firmware(u32), OffRoute(u32), Lit(u32) }

/// Step 2's writes (213-DRV-0095, `ddc.rs`): the I2C engine's control, arbitration, DDC1's speed and set-up, its four
/// transactions and buffer, and DDC1's pad mask; nothing else, whatever the pipe.
pub const DDC_WRITES: [R; 7] = [R::DcI2cControl, R::DcI2cArbitration, R::DcI2cDdc1Speed, R::DcI2cDdc1Setup, R::DcI2cTransaction,
                                R::DcI2cData, R::DcGpioDdc1Mask];
/// Step 2's writes on HDMI-A-2's DDC line (213-DRV-0112): the engine's, DDC2's speed and set-up and DDC2's pad mask.
pub const DDC2_WRITES: [R; 7] = [R::DcI2cControl, R::DcI2cArbitration, R::DcI2cDdc2Speed, R::DcI2cDdc2Setup, R::DcI2cTransaction,
                                 R::DcI2cData, R::DcGpioDdc2Mask];
/// Step 2's writes for an output: its DDC line's.
pub fn ddc_writes(o: &Output) -> Option<&'static [R; 7]> { if *o == HDMI_A_1 { Some(&DDC_WRITES) } else if *o == HDMI_A_2 { Some(&DDC2_WRITES) } else { None } }
/// A write of step 2 on HDMI-A-1's line: one of `DDC_WRITES`' registers (any of the four transactions), else refused.
pub fn ddc_writable(reg: Reg) -> Result<Writable, Refused> { ddc_writable_on(&DDC_WRITES, reg) }
/// A write of step 2 on a line: one of `writes`' registers (any of the four transactions), else refused, the other
/// line's speed, set-up and pads with the rest.
pub fn ddc_writable_on(writes: &[R], reg: Reg) -> Result<Writable, Refused> {
    let o = reg.offset();
    let ok = writes.iter().any(|r| (0..r.instances()).any(|i| r.at(i).map(|x| x.offset()) == Some(o)));
    if ok { Ok(Writable(reg)) } else { Err(Refused::OffRoute(o)) }
}

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

/// The blocks of HDMI socket `o` (213-DRV-0112), as byte ranges [start, end) of BAR 5: its DIG block (only its back
/// end's DIG_BE_CNTL, DIG_BE_EN_CNTL and DIG_LANE_ENABLE when the chosen front end is that DIG, whose front end registers
/// table 4 writes), its DP link registers, UNIPHY, DPCS/RDPCS, PLL resync and SYMCLK, its HPD block, and its DDC line's
/// status, speed, set-up and pads (dcn_2_1_0_offset.h, dpcs_2_1_0_offset.h; the board's DDC line n is PHY n's).
fn socket_blocks(o: &Output, fe_shared: bool, add: &mut impl FnMut(u32, u32)) {
    let (p, h) = (o.phy as u32, o.hpd as u32);
    if fe_shared {
        add(seg2(0x20AF + 0x100 * p), seg2(0x20B1 + 0x100 * p));
        add(seg2(0x20E1 + 0x100 * p), seg2(0x20E2 + 0x100 * p));
    } else {
        add(seg2(0x2068 + 0x100 * p), seg2(0x20F8 + 0x100 * p));
    }
    add(seg2(0x2108 + 0x100 * p), seg2(0x2109 + 0x100 * p));
    add(seg2(0x2118 + 0x100 * p), seg2(0x2119 + 0x100 * p));
    add(seg2(0x286D + 2 * p), seg2(0x286F + 2 * p));
    add(seg2(0x2928 + 0xD8 * p), seg2(0x2928 + 0xD8 * (p + 1)));
    add(seg2(0x1F14 + 8 * h), seg2(0x1F1C + 8 * h));
    add(seg2(0x1E9C + p), seg2(0x1E9D + p));
    add(seg2(0x1EA2 + 2 * p), seg2(0x1EA4 + 2 * p));
    add(seg2(0x28D0 + 4 * p), seg2(0x28D4 + 4 * p));
    add(seg1(0x40 + p), seg1(0x41 + p));
    add(seg1(0xA0 + p), seg1(0xA1 + p));
}

/// PHY A's RDPCSTX_PHY_FUSE0, which the Ryzen's table 12 sets first, for every PHY (PLL ids 20 to 24 alike): its
/// RDPCS_PHY_DP_MPLLB_V2I to 2, the other bits kept (dpcs_2_1_0_offset.h and _sh_mask.h; 213-DRV-0112). Off HDMI-A-2's
/// route, so that table's write of it is left unmade where the register holds that value already, and refused otherwise.
pub const TABLE12_SETS_FOR_EVERY_PHY: u32 = seg2(0x294F);

/// PHY `p`'s DPCS/RDPCS block, whose sequence the VBIOS owns.
pub fn phy_block(p: u8) -> (u32, u32) { (seg2(0x2928 + 0xD8 * p as u32), seg2(0x2928 + 0xD8 * (p as u32 + 1))) }

/// What step 3 may write: the chosen pipe's and output's blocks, never one of the firmware's, nor one of the other HDMI
/// socket's (213-DRV-0112), which is refused as off the route before the route's blocks are looked at.
#[derive(Clone, Copy, Debug)]
pub struct Guard {
    allow: [(u32, u32); 16], allows: usize, deny: [(u32, u32); 32], denies: usize, late: [(u32, u32); 4], other: [(u32, u32); 16], others: usize,
    /// The other pipe display has lit (213-DRV-0113): its blocks, refused as the firmware's are.
    lit: [(u32, u32); 24], lits: usize,
}

impl Guard {
    pub fn new(firmware: &Firmware, choice: &Choice) -> Result<Guard, Refusal> {
        let mut g = Guard { allow: [(0, 0); 16], allows: 0, deny: [(0, 0); 32], denies: 0, late: [(0, 0); 4], other: [(0, 0); 16], others: 0, lit: [(0, 0); 24], lits: 0 };
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
        if let Some(o) = other(&choice.output) {
            let mut refuse = |a: u32, b: u32| { if g.others < 16 { g.other[g.others] = (a, b); g.others += 1; } };
            socket_blocks(&o, dig_fe == o.phy, &mut refuse);
        }
        // Step 4's (from stage 11): pipe 1's HUBP, DPP and MPCC, and its OPP's output mux (the index of its OTG).
        let (q, o) = (otg as u32, opp as u32);
        g.late = [(seg2(0x5E5 + 0xDC * q), seg2(0x5E5 + 0xDC * (q + 1))), (seg2(0xCC5 + 0x16B * q), seg2(0xCC5 + 0x16B * (q + 1))),
                  (seg2(0x1271 + 0x1B * q), seg2(0x1271 + 0x1B * (q + 1))), (seg2(0x1385 + 4 * o), seg2(0x1386 + 4 * o))];
        let overlap = g.allow[..g.allows].iter().chain(g.late.iter()).any(|&(a, b)| g.deny[..g.denies].iter().any(|&(c, d)| a < d && c < b));
        if overlap { Err(Refusal::Overlap) } else { Ok(g) }
    }

    /// The guard with the pipe and output of `lit`, display's other lit one, refused too (213-DRV-0113): its OTG, ODM, VTG,
    /// OPP and DPG, its DIG blocks, PHY, PLL resync, pixel rate and SYMCLK, its HUBP, DPP, MPCC and MPC_OUT mux. Refused
    /// when this route shares a block with it.
    pub fn beside(mut self, lit: &Choice) -> Result<Guard, Refusal> {
        let (Pipe { otg, opp, dig_fe }, phy) = (lit.pipe, lit.output.phy);
        let mut refuse = |a: u32, b: u32| { if self.lits < 24 { self.lit[self.lits] = (a, b); self.lits += 1; } };
        blocks(otg, opp, dig_fe, phy, &mut refuse);
        let (q, o) = (otg as u32, opp as u32);
        refuse(seg2(0x5E5 + 0xDC * q), seg2(0x5E5 + 0xDC * (q + 1)));
        refuse(seg2(0xCC5 + 0x16B * q), seg2(0xCC5 + 0x16B * (q + 1)));
        refuse(seg2(0x1271 + 0x1B * q), seg2(0x1271 + 0x1B * (q + 1)));
        refuse(seg2(0x1385 + 4 * o), seg2(0x1386 + 4 * o));
        let overlap = self.allow[..self.allows].iter().chain(self.late.iter()).any(|&(a, b)| self.lit[..self.lits].iter().any(|&(c, d)| a < d && c < b));
        if overlap { Err(Refusal::Overlap) } else { Ok(self) }
    }

    /// A write to `reg` in stage `n`: as `check`, and from stage 11 pipe 1's HUBP, DPP, MPCC and output mux too.
    pub fn check_at(&self, reg: Reg, n: u8) -> Result<Writable, Refused> {
        match self.check(reg) {
            Err(Refused::OffRoute(o)) if n >= dcn::PIPE_WRITES_FROM && !self.others_(o) && self.late.iter().any(|&(a, b)| (a..b).contains(&o)) => Ok(Writable(reg)),
            other => other,
        }
    }

    /// Whether byte `o` lies in the other HDMI socket's blocks.
    fn others_(&self, o: u32) -> bool { self.other[..self.others].iter().any(|&(a, b)| (a..b).contains(&o)) }

    /// A write to `reg`: the firmware's blocks first, then the other lit pipe's, then the other socket's, then the route's.
    pub fn check(&self, reg: Reg) -> Result<Writable, Refused> {
        let o = reg.offset();
        if self.deny[..self.denies].iter().any(|&(a, b)| (a..b).contains(&o)) { return Err(Refused::Firmware(o)); }
        if self.lit[..self.lits].iter().any(|&(a, b)| (a..b).contains(&o)) { return Err(Refused::Lit(o)); }
        if self.others_(o) { return Err(Refused::OffRoute(o)); }
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

/// Step 1's batches 2 and 7 for a timing generator: its state, clocks, source and pixel rate.
const BY_PIPE: [R; 21] = {
    use R::*;
    [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax, OtgVTotalControl,
     OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgClockControl, OtgVerticalInterrupt1Control,
     OtgVerticalInterrupt1Position, OtgVerticalInterrupt2Control, OtgVerticalInterrupt2Position, OptcInputGlobalControl,
     OptcDataSourceSelect, OtgPixelRateCntl, OtgPhypllPixelRateCntl]
};

/// The firmware's registers that step 1 read and step 3 watches.
#[derive(Clone, Copy, Debug)]
pub struct Watch { regs: [Option<(R, u8, Reg)>; WATCH], len: usize }

impl Watch {
    pub fn of(firmware: &Firmware) -> Watch {
        let mut w = Watch { regs: [None; WATCH], len: 0 };
        let mut push = |r: R, i: u8| if let (Some(reg), true) = (r.at(i), w.len < WATCH) { w.regs[w.len] = Some((r, i, reg)); w.len += 1; };
        let p = firmware.protected();
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
    /// The pipe display has lit beside this one (213-DRV-0113): its timing generator's state, clocks, source and pixel
    /// rate, its DIG front and back end, its PHY's PLL resync and its HUBP's surface and control, as the firmware's are.
    pub fn pipe(lit: &Choice) -> Watch {
        let mut w = Watch { regs: [None; WATCH], len: 0 };
        let mut push = |r: R, i: u8| if let (Some(reg), true) = (r.at(i), w.len < WATCH) { w.regs[w.len] = Some((r, i, reg)); w.len += 1; };
        for r in BY_PIPE { push(r, lit.pipe.otg); }
        push(R::DigFeCntl, lit.pipe.dig_fe);
        push(R::DigBeCntl, lit.output.phy);
        push(R::DigBeEnCntl, lit.output.phy);
        push(R::PhypllPixclkResyncCntl, lit.output.phy);
        for r in [R::HubpreqPrimarySurfaceAddress, R::HubpreqPrimarySurfaceAddressHigh, R::HubpSurfaceConfig, R::HubpreqSurfacePitch, R::HubpCntl] { push(r, lit.pipe.otg); }
        w
    }
    /// No register: a run with no other pipe lit beside it.
    pub fn none() -> Watch { Watch { regs: [None; WATCH], len: 0 } }
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

/// Every write step 3 made, with the value before it and its stage, for the undo; and for an unplug's taking back
/// (213-DRV-0096) the mark with its value before, and the firmware's pipe as step 3 began.
pub struct Journal {
    items: [Option<(Writable, u32, u8)>; JOURNAL], len: usize, mark: Option<(Writable, u32)>, baseline: Option<[u32; WATCH]>,
    /// The two pipes' HUBP and ODM statuses as stage 9 found them, for the standing check (213-DRV-0103).
    statuses: Option<[u32; 4]>,
    /// The pipe lit beside this run as step 3 began (213-DRV-0113).
    beside: Option<[u32; WATCH]>,
}
impl Default for Journal { fn default() -> Journal { Journal { items: [None; JOURNAL], len: 0, mark: None, baseline: None, statuses: None, beside: None } } }
impl Journal {
    /// Nothing journaled, for a run of the stages again.
    pub fn clear(&mut self) {
        for item in self.items[..self.len].iter_mut() { *item = None; }
        (self.len, self.mark, self.baseline, self.statuses, self.beside) = (0, None, None, None, None);
    }
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

// The lit pipe's state at the end of a line, when a pipe is lit beside the run (213-DRV-0113); nothing otherwise.
struct LitToo(Option<(&'static str, Option<bool>)>);
impl fmt::Display for LitToo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Some((n, Some(true))) => write!(f, "; {}'S PIPE UNCHANGED", n),
            Some((n, Some(false))) => write!(f, "; {}'S PIPE CHANGED", n),
            Some((n, None)) => write!(f, "; {}'S PIPE UNREADABLE", n),
            None => Ok(()),
        }
    }
}

// A table's writes left unmade, at the end of its line; nothing when none.
struct Left(u32);
impl fmt::Display for Left {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { if self.0 > 0 { write!(f, ", {} LEFT AS IT IS", self.0) } else { Ok(()) } }
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
    pub firmware: Firmware, pub choice: Choice, pub timing: Timing, pub signal: Signal,
    /// Step 4's surface for the mode (first light's 1920x1080, or the mode's with its pitch rounded to 64 pixels).
    pub surface: picture::Size, plan: Plan, starts: [usize; STAGES as usize + 1], guard: Guard,
    watch: Watch, attached: Watch,
    /// Step 4's reference, when step 1 read it (stage 9 refuses without it), and the build stage 10's picture names.
    pub reference: Option<Reference>, pub build: &'static str,
    /// Stage 15's watch, in frames: `WATCH_FRAMES`, or `SHORT_WATCH_FRAMES` with the standing check (213-DRV-0103).
    pub watch_frames: u32,
    /// The pipe display has lit beside this run, if any (213-DRV-0113): read after each stage as the firmware's is.
    pub lit: Option<Choice>, beside: Watch,
}

impl Light {
    pub fn new(firmware: Firmware, choice: Choice, timing: Timing) -> Result<Light, Refusal> { Light::new_with(firmware, choice, timing, Signal::Dvi) }
    /// As `new`, in DVI or HDMI signalling (213-DRV-0095).
    pub fn new_with(firmware: Firmware, choice: Choice, timing: Timing, signal: Signal) -> Result<Light, Refusal> {
        let guard = Guard::new(&firmware, &choice)?;
        let mut plan = first_light(choice.pipe, choice.output, &timing, signal, Pattern::ColorSquares, firmware.protected()).map_err(Refusal::Plan)?;
        dcn::second_pipe(&mut plan).map_err(Refusal::Plan)?;
        let mut starts = [plan.len(); STAGES as usize + 1];
        for (i, s) in plan.steps().enumerate() { if let Step::Begin(n) = s { if (1..=STAGES).contains(&n) { starts[n as usize - 1] = i + 1; } } }
        let surface = picture::Size::for_mode(timing.h_active as usize, timing.v_active as usize);
        if surface.bytes() > SURFACE_BYTES || surface.width > picture::MAX_WIDTH { return Err(Refusal::Surface { bytes: surface.bytes() }); }
        Ok(Light { firmware, choice, timing, signal, surface, plan, starts, guard, watch: Watch::of(&firmware), attached: Watch::attached(&choice), reference: None, build: "UNKNOWN", watch_frames: WATCH_FRAMES,
                   lit: None, beside: Watch::none() })
    }
    /// Beside `lit`, the pipe display has lit already (213-DRV-0113): its blocks refused to this run, its registers read after
    /// each stage.
    pub fn beside(mut self, lit: &Choice) -> Result<Light, Refusal> {
        self.guard = self.guard.beside(lit)?;
        (self.lit, self.beside) = (Some(*lit), Watch::pipe(lit));
        Ok(self)
    }
    /// This lit run's guard refusing the blocks of `other`, a run display lights beside it (213-DRV-0113).
    pub fn refuse(&mut self, other: &Choice) -> Result<(), Refusal> { self.guard = self.guard.beside(other)?; Ok(()) }
    /// The pipe lit beside this run, read after each stage.
    pub fn beside_watch(&self) -> &Watch { &self.beside }
    /// With step 4's reference, from step 1's values.
    pub fn with_reference(mut self, r: Option<Reference>) -> Light { self.reference = r; self }
    /// With stage 15's watch of `frames` (at most `WATCH_FRAMES`): the build gives `SHORT_WATCH_FRAMES` only together
    /// with the standing check (213-DRV-0103).
    pub fn with_watch(mut self, frames: u32) -> Light { self.watch_frames = frames.clamp(1, WATCH_FRAMES); self }
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

    // The pipes and the mode: logged in either order (213-DRV-0098).
    fn intro(&self, allowed: u8, expected: bool, out: &mut dyn Out) {
        let (f, c) = (self.firmware, self.choice);
        out.part();
        out.note(format_args!("STEP 3: FIRST LIGHT ON {} IN {} STAGES, STEP 4 IN {} MORE (MIND CORE'S PICTURE); THIS BUILD MAY RUN {}",
            name(&c.output), dcn::FIRST_LIGHT, STAGES - dcn::FIRST_LIGHT, allowed));
        out.note(format_args!("S0 THE FIRMWARE'S PIPE ({}): OTG{} OPP{} DIG {} FRONT END, PHY {}, HUBPS {}{}{}{}; NEVER WRITTEN, READ AFTER EACH STAGE ({} REGISTERS)",
            if expected { "AS THE SURVEY EXPECTS: STEP 1 NOT READ" } else { "AS STEP 1 FOUND" }, f.otg, f.opp, LETTERS[f.dig_fe as usize], LETTERS[f.phy as usize],
            f.hubps[0] as u8, f.hubps[1] as u8, f.hubps[2] as u8, f.hubps[3] as u8, self.watch.len()));
        out.note(format_args!("S0 STEP 3'S PIPE: OTG{} OPP{} DIG {} FRONT END, PHY {} ({}), HPD {}; MODE {}x{} AT {} MHZ IN {} SIGNALLING (CTA VIC {})",
            c.pipe.otg, c.pipe.opp, LETTERS[c.pipe.dig_fe as usize], LETTERS[c.output.phy as usize], name(&c.output), c.output.hpd + 1,
            self.timing.h_active, self.timing.v_active, self.timing.pixel_khz as f32 / 1000.0,
            match self.signal { Signal::Dvi => "DVI", Signal::Hdmi => "HDMI" }, crate::timing::vic_of(&self.timing).unwrap_or(0)));
        for n in (0..4u8).filter(|&n| c.attached[n as usize]) {
            out.line(format_args!("S0 HUBP{} FOLLOWS OTG{}, POWERED AND BLANKED: NEVER WRITTEN, ITS DCHUBP_CNTL READ AFTER EACH STAGE FROM {} ON", n, c.pipe.otg, ATTACHED_FROM));
        }
        if let Some(l) = self.lit {
            out.note(format_args!("S0 {}'S PIPE, LIT BY DISPLAY: OTG{} OPP{} DIG {} FRONT END, PHY {}, HUBP{}; NEVER WRITTEN, READ AFTER EACH STAGE ({} REGISTERS)",
                name(&l.output), l.pipe.otg, l.pipe.opp, LETTERS[l.pipe.dig_fe as usize], LETTERS[l.output.phy as usize], l.pipe.otg, self.beside.len()));
        }
    }
    /// The end of a line saying the firmware's pipe is unchanged: the same of the pipe lit beside, if any.
    fn lit_too(&self, same: Option<bool>) -> LitToo { LitToo(self.lit.map(|l| (name(&l.output), same))) }

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
                n, verb, self.surface.width, self.surface.height, self.surface.pitch, self.build)),
            Step::NoUnderflow => out.line(format_args!("S{} {} CHECK FOR A NEW UNDERFLOW ON BOTH PIPES", n, verb)),
            Step::ReadBack { reg, .. } => out.line(format_args!("S{} {} READ {} BACK", n, verb, Name(reg))),
            Step::Watch { frames } => out.line(format_args!("S{} {} WATCH {} FRAMES OF OTG{} WITH THE PATTERN OFF, READING WITHOUT SLEEP: AT EACH FRAME BOTH PIPES' HUBP AND ODM; THE FIRMWARE'S UNDERFLOWING OR TIMING OUT UNDOES AT ONCE; PIPE {}'S KEPT BY FRAME, CLEARED AND WATCHED ON, THEN FAIL THE STAGE",
                n, verb, frames, self.choice.pipe.otg, self.choice.pipe.otg)),
            Step::Offer => out.line(format_args!("S{} {} OFFER THE SURFACE TO THE COMPOSITOR AS THE SECOND SCREEN: {}x{}, PITCH {} BYTES, BGR (DISPLAY.WIT 1.3); UNDO: SCREEN_GONE FIRST",
                n, verb, self.surface.width, self.surface.height, self.surface.pitch * 4)),
            Step::Hold { us } => out.line(format_args!("S{} {} HOLD IT {} US", n, verb, us)),
            Step::Late | Step::Begin(_) => {}
        }
    }

    fn plan_lines(&self, n: u8, verb: &str, late: Option<&Plan>, out: &mut dyn Out) {
        let built = self.stage(n).any(|s| matches!(s, Step::Late));
        match (built, late) {
            (true, Some(p)) => {
                for step in dcn::steps_in(p, n) { self.step_line(n, verb, step, out); }
                if n == 13 {
                    out.line(format_args!("S{} PITCH FIELD {} ({} PIXELS LESS ONE, AS DISPLAY CORE WRITES IT); THE FIRMWARE'S HUBP{} HOLDS {} FOR ITS {} PIXELS",
                        n, self.surface.pitch - 1, self.surface.pitch, self.reference.map_or(0, |r| r.pipe), self.reference.map_or(0, |r| r.pitch_field),
                        self.reference.map_or(0, |r| r.width)));
                }
            }
            (true, None) => {
                let (q, o, f) = (self.choice.pipe.otg, self.choice.pipe.opp, self.reference.map_or(0, |r| r.pipe));
                match n {
                    11 => out.line(format_args!("S{} {} TURN ON DPP{}'S AND HUBP{}'S CLOCKS (ITS WRITES SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, q)),
                    12 => out.line(format_args!("S{} {} SET DPP{} AS DPP{}: FORMAT, COLOUR STAGES, SCALER 1:1, RECOUT {}x{} (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, f, self.timing.h_active, self.timing.v_active)),
                    13 => out.line(format_args!("S{} {} SET HUBP{}, BLANKED: THE SURFACE AT FB + 0x1000000, PITCH {}, ITS DLG AND TTU (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, self.surface.pitch - 1)),
                    14 => out.line(format_args!("S{} {} CONNECT MPCC{} FROM DPP{} TO OPP{} (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, q, o)),
                    _ => out.line(format_args!("S{} {} UNBLANK HUBP{}, THEN TURN OPP{}'S PATTERN OFF AND WATCH BOTH PIPES {} FRAMES (SHOWN WHEN STAGE 9 HAS RUN)", n, verb, q, o, self.watch_frames)),
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
    out.note(format_args!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", STAGES));
}

// --- running ---

/// Why a stage failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreadable(u32),
    Refused(Refused),
    /// display started again with the stages in place: step 3's OTG holds the mark of a start before.
    StartedAgain,
    /// Stage 9 without step 1's batches 9 and 10 (no reference), or its measure of the firmware's frames failed.
    NoReference,
    /// Pipe 1's MPCC belongs to another OPP, or is in the firmware's tree (its OPP's mux, or its MPCC's bottom).
    MpccInUse { mpcc: u8, opp_id: u32, firmware_mux: u32, firmware_bot: u32 },
    /// Pipe 1's OPP's output mux names another MPCC.
    OutputMuxInUse { opp: u8, mux: u32 },
    /// Stages 11 to 15 do not fit a plan.
    PlanFull,
    /// Stage 16: the compositor did not take the second screen.
    Screen(ScreenRefused),
    /// An underflow, new since stage 9, at this HUBP's or ODM's register.
    Underflow { offset: u32, value: u32 },
    /// A request timed out at this HUBP (its HUBP_TIMEOUT_STATUS new since stage 9).
    RequestTimeout { offset: u32, value: u32 },
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
    /// The pipe display has lit beside this run changed (213-DRV-0113).
    LitChanged { offset: u32, before: u32, after: u32 },
    JournalFull,
}

impl Failure {
    /// A failure of what step 4 found or prepared, not of the hardware: stage 9, 10 or 16 then stops without undoing.
    pub fn not_ready(&self) -> bool {
        matches!(self, Failure::NoReference | Failure::NoFrames(_) | Failure::MpccInUse { .. } | Failure::OutputMuxInUse { .. }
            | Failure::PlanFull | Failure::NoSurface | Failure::SurfaceDiffers { .. } | Failure::Screen(_))
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Failure::Unreadable(o) => write!(f, "{:05X} COULD NOT BE READ", o),
            Failure::StartedAgain => f.write_str("DISPLAY HAS STARTED STEP 3 ALREADY (THE MARK IN ITS OTG): THE STAGES RUN ONCE UNTIL AN UNPLUG TAKES THEM BACK"),
            Failure::NoReference => f.write_str("NO REFERENCE: STEP 1'S BATCHES 9 AND 10 NOT READ, OR THE FIRMWARE'S FRAMES NOT COUNTED"),
            Failure::NoSurface => f.write_str("SLOT 5'S SURFACE IS NOT MAPPED"),
            Failure::MpccInUse { mpcc, opp_id, firmware_mux, firmware_bot } => write!(f,
                "MPCC{} IS IN USE: ITS OPP_ID {:X} (ITS OWN OR 15 WANTED), THE FIRMWARE'S MUX {:X} AND MPCC BOTTOM {:X} (NOT {} WANTED)", mpcc, opp_id, firmware_mux, firmware_bot, mpcc),
            Failure::OutputMuxInUse { opp, mux } => write!(f, "OPP{}'S OUTPUT MUX NAMES MPCC {:X} (ITS OWN OR 15 WANTED)", opp, mux),
            Failure::PlanFull => f.write_str("STAGES 11 TO 15 DO NOT FIT A PLAN"),
            Failure::Underflow { offset, value } => write!(f, "AN UNDERFLOW AT {:05X} ({:08X})", offset, value),
            Failure::RequestTimeout { offset, value } => write!(f, "A REQUEST TIMEOUT AT {:05X} ({:08X})", offset, value),
            Failure::Screen(why) => write!(f, "THE COMPOSITOR DID NOT TAKE THE SECOND SCREEN: {}", why),
            Failure::SurfaceDiffers { x, y } => write!(f, "THE SURFACE READ BACK DIFFERS AT {},{}", x, y),
            Failure::Refused(Refused::Firmware(o)) => write!(f, "A WRITE TO THE FIRMWARE'S {:05X} WAS REFUSED", o),
            Failure::Refused(Refused::OffRoute(o)) => write!(f, "A WRITE OFF THE ROUTE, TO {:05X}, WAS REFUSED", o),
            Failure::Timeout(o) => write!(f, "{:05X} NEVER SHOWED ITS STATUS", o),
            Failure::NoFrames(n) => write!(f, "THE FRAME COUNT MOVED {} TIMES ONLY", n),
            Failure::Table(e) => write!(f, "THE TABLE STOPPED: {:?}", e),
            Failure::FirmwareChanged { offset, before, after } => write!(f, "THE FIRMWARE'S {:05X} CHANGED FROM {:08X} TO {:08X}", offset, before, after),
            Failure::AttachedChanged { offset, before, after } => write!(f, "THE ATTACHED HUBP'S {:05X} CHANGED FROM {:08X} TO {:08X}", offset, before, after),
            Failure::LitChanged { offset, before, after } => write!(f, "THE OTHER LIT PIPE'S {:05X} CHANGED FROM {:08X} TO {:08X}", offset, before, after),
            Failure::Refused(Refused::Lit(o)) => write!(f, "A WRITE TO THE OTHER LIT PIPE'S {:05X} WAS REFUSED", o),
            Failure::JournalFull => f.write_str("MORE WRITES THAN THE UNDO CAN HOLD"),
        }
    }
}

/// A table's run, for its stage's line (213-DRV-0098): its index, steps and writes, and its time without the log's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableRun { pub index: u8, pub steps: u32, pub writes: u32, pub us: u64 }

/// What a stage did and measured, for its one line and step 3's times (213-DRV-0098).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// The stage's own writes (its tables' are theirs), its status and frame waits and their time.
    pub writes: u32, pub waits: u32, pub wait_us: u64,
    pub tables: [Option<TableRun>; 2],
    /// Table 12's path: the short one (PHY A set up already), or the long one with its writes through PHY A's bus.
    pub path: Option<(bool, u32)>,
    /// Stage 9's count of the firmware's frames; stage 13's pitch read back as written or not.
    pub count_us: u64, pub read_back: Option<bool>,
    /// Stage 15's watch: its frames, its time, the longest gap between two reads and the frame it came before.
    pub watch: Option<(u32, u64, u64, u32)>,
    pub offered: bool,
}

/// Step 3's times (213-DRV-0098), from its run's start: each stage's own time (its writes, waits, tables and the checks
/// after it), the tables' and the waits' shares, stage 9's count and stage 15's watch, and when the offer was taken.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Times {
    /// The clock when the run began.
    pub start_us: u64,
    pub stages_us: [u64; STAGES as usize],
    pub tables_us: u64, pub waits_us: u64, pub count_us: u64, pub watch_us: u64,
    pub offered_us: Option<u64>, pub total_us: u64,
}

/// A stage's line in either order: done, the firmware's pipe unchanged, its time, and what it did.
struct Done<'a> { light: &'a Light, n: u8, us: u64, t: &'a Tally, d: Option<&'a Derived> }
impl fmt::Display for Done<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (l, n, t) = (self.light, self.n, self.t);
        write!(f, "S{} {} DONE; THE FIRMWARE'S PIPE UNCHANGED ({} REGISTERS)", n, STAGE_NAMES[n as usize - 1], l.watch.len())?;
        if let Some(lit) = l.lit { write!(f, ", {}'S TOO ({})", name(&lit.output), l.beside.len())?; }
        write!(f, "; {}.{} MS", self.us / 1000, self.us % 1000 / 100)?;
        if t.writes > 0 { write!(f, ", {} WRITES", t.writes)?; }
        if t.waits > 0 { write!(f, ", {} WAITS {} US", t.waits, t.wait_us)?; }
        for r in t.tables.iter().flatten() { write!(f, ", TABLE {} {} US ({} STEPS, {} WRITES)", r.index, r.us, r.steps, r.writes)?; }
        match t.path {
            Some((true, _)) => f.write_str(", THE PHY SET UP ALREADY: THE SHORT PATH")?,
            Some((false, bus)) => write!(f, ", {}{} WRITES THROUGH THE PHY'S BUS", if n == 2 { "THE LONG PATH: " } else { "" }, bus)?,
            None => {}
        }
        if let (9, Some(d)) = (n, self.d) {
            write!(f, ", OTG{} AT {} KHZ, THE REFERENCE {} KHZ", l.firmware.otg, d.firmware_pixel_khz, d.reference_khz)?;
        }
        if n == 10 { write!(f, ", {}x{} DRAWN AND READ BACK", l.surface.width, l.surface.height)?; }
        match t.read_back { Some(true) => f.write_str(", THE PITCH READ BACK AS WRITTEN")?, Some(false) => f.write_str(", THE PITCH READ BACK NOT AS WRITTEN")?, None => {} }
        if let Some((frames, us, gap, at)) = t.watch { write!(f, ", WATCHED {} FRAMES IN {} MS CLEAN, LONGEST GAP {} US AT FRAME {}", frames, us / 1000, gap, at)?; }
        if t.offered { write!(f, ", OFFERED {}x{} PITCH {}", l.surface.width, l.surface.height, l.surface.pitch * 4)?; }
        Ok(())
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
    /// Stage `stage` (9 or 10, which write no register) was not ready: stages 1 to `done` stay as they are.
    NotReady { done: u8, stage: u8, why: Failure },
}

/// The interpreter's machine for step 3: reads, writes through the guard into the journal, each write logged; the time
/// the log takes is not counted against the table.
struct Adapter<'a, E: Engine> {
    engine: &'a mut E, guard: &'a Guard, journal: &'a mut Journal, out: &'a mut dyn Out,
    tag: (char, u8, u8), record: bool, writes: u32, refused: Option<Refused>, unreadable: Option<u32>, full: bool, log_us: u64,
    /// Writes left unmade: table 12's of a register off the route that held their value already (213-DRV-0112).
    left: u32,
}

impl<E: Engine> atom::Machine for Adapter<'_, E> {
    fn read(&mut self, reg: Reg) -> Option<u32> { self.engine.read(reg) }
    fn write(&mut self, reg: Reg, value: u32) -> bool {
        let w = match self.guard.check(reg) {
            Ok(w) => w,
            Err(Refused::OffRoute(o) | Refused::Lit(o)) if o == TABLE12_SETS_FOR_EVERY_PHY && self.tag.2 == dcn::TABLE_SETPIXELCLOCK => return self.leave(reg, value),
            Err(r) => { self.refused = Some(r); return false; }
        };
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

impl<E: Engine> Adapter<'_, E> {
    // Table 12's write of PHY A's FUSE0 on another output's route: not made when the register holds the value already,
    // so that PHY A is not written; refused when it would change it.
    fn leave(&mut self, reg: Reg, value: u32) -> bool {
        let Some(old) = self.engine.read(reg) else { self.unreadable = Some(reg.offset()); return false };
        let t0 = self.engine.now_us();
        let (tag, o) = (self.tag, reg.offset());
        let same = old == value;
        if same {
            self.left += 1;
            self.out.line(format_args!("{}{} T{} LEFT PHY A'S RDPCSTX_PHY_FUSE0 AT {:05X} AS IT IS: TABLE 12 SETS IT FOR EVERY PHY, AND IT HOLDS THAT VALUE ({:08X})",
                tag.0, tag.1, tag.2, o, old));
        } else {
            self.refused = Some(if self.guard.check(reg) == Err(Refused::Lit(o)) { Refused::Lit(o) } else { Refused::OffRoute(o) });
            self.out.line(format_args!("{}{} T{} REFUSED PHY A'S RDPCSTX_PHY_FUSE0 AT {:05X}: TABLE 12 SETS IT FOR EVERY PHY, AND THIS WRITE WOULD CHANGE IT FROM {:08X} TO {:08X}",
                tag.0, tag.1, tag.2, o, old, value));
        }
        self.log_us += self.engine.now_us().saturating_sub(t0);
        same
    }
}

/// Runs table `t` for stage `n`; `record` journals its writes for the undo.
fn call<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, t: Table, prefix: char, n: u8, record: bool, out: &mut dyn Out) -> Result<TableRun, Failure> {
    let (p, len) = t.parameters();
    out.line(format_args!("{}{} RUNNING TABLE {} ({}) NOW", prefix, n, t.index(), table_name(t.index())));
    let begin = e.now_us();
    let mut a = Adapter { engine: e, guard: &light.guard, journal, out, tag: (prefix, n, t.index()), record, writes: 0, refused: None, unreadable: None, full: false, log_us: 0, left: 0 };
    interp.set_time_limit(TABLE_TIME_LIMIT_US);
    let result = interp.run(t.index(), &p[..len], &mut a);
    let (writes, refused, unreadable, full, log_us, left) = (a.writes, a.refused, a.unreadable, a.full, a.log_us, a.left);
    let us = e.now_us().saturating_sub(begin).saturating_sub(log_us);
    match result {
        Ok(o) => {
            out.line(format_args!("{}{} TABLE {} DONE: {} STEPS, {} CALLS, {} US OF DELAYS, {} WRITES{}", prefix, n, t.index(), o.steps, o.calls, o.delay_us, writes, Left(left)));
            Ok(TableRun { index: t.index(), steps: o.steps, writes, us })
        }
        Err(_) if refused.is_some() => Err(Failure::Refused(refused.unwrap_or(Refused::OffRoute(0)))),
        Err(_) if full => Err(Failure::JournalFull),
        Err(_) if unreadable.is_some() => Err(Failure::Unreadable(unreadable.unwrap_or(0))),
        Err(atom::Error::Unreadable { offset, .. }) => Err(Failure::Unreadable(offset)),
        Err(err) => Err(Failure::Table(err)),
    }
}

fn read(e: &mut impl Bus, reg: Reg) -> Result<u32, Failure> { e.read(reg).ok_or(Failure::Unreadable(reg.offset())) }

fn stage<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, derived: &mut Option<Derived>,
                    late: Option<&Plan>, out: &mut dyn Out) -> Result<Tally, Failure> {
    let built = light.stage(n).any(|s| matches!(s, Step::Late));
    let steps = match (built, late) { (true, Some(p)) => dcn::steps_in(p, n), (true, None) => return Err(Failure::NoReference), _ => dcn::steps_in(&light.plan, n) };
    let mut last: Option<(Reg, u32)> = None;
    let mut tally = Tally::default();
    for step in steps {
        match step {
            Step::Update { reg, mask, value } => {
                let w = light.guard.check_at(reg, n).map_err(Failure::Refused)?;
                let old = read(e, reg)?;
                let new = (old & !mask) | value;
                if !journal.push(w, old, n) { return Err(Failure::JournalFull); }
                e.write(w, new);
                last = Some((reg, new));
                tally.writes += 1;
                out.line(format_args!("S{} WROTE {} = {:08X} (WAS {:08X})", n, Name(reg), new, old));
            }
            Step::ReadBack { reg, field } => {
                let v = read(e, reg)?;
                match last.filter(|l| l.0 == reg) {
                    Some((_, w)) if w == v => out.line(format_args!("S{} READ BACK {} = {:08X}, AS WRITTEN: FIELD {}", n, Name(reg), v, field.get(v))),
                    Some((_, w)) => out.line(format_args!("S{} READ BACK {} = {:08X}, NOT {:08X} AS WRITTEN: FIELD {}, NOT {}", n, Name(reg), v, w, field.get(v), field.get(w))),
                    None => out.line(format_args!("S{} READ BACK {} = {:08X}: FIELD {}", n, Name(reg), v, field.get(v))),
                }
                tally.read_back = Some(last.is_some_and(|l| l == (reg, v)));
            }
            // Every wait is for its condition, polled, with its limit: in either order (213-DRV-0098).
            Step::Wait { reg, mask, value, timeout_us } => {
                let start = e.now_us();
                loop {
                    if read(e, reg)? & mask == value { break; }
                    if e.now_us().saturating_sub(start) > timeout_us as u64 { return Err(Failure::Timeout(reg.offset())); }
                    e.delay_us(POLL_US);
                }
                let us = e.now_us().saturating_sub(start);
                (tally.waits, tally.wait_us) = (tally.waits + 1, tally.wait_us + us);
                out.line(format_args!("S{} {} SHOWED ITS STATUS IN {} US", n, Name(reg), us));
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
                let us = e.now_us().saturating_sub(start);
                (tally.waits, tally.wait_us) = (tally.waits + 1, tally.wait_us + us);
                out.line(format_args!("S{} OTG{} COUNTED {} FRAMES IN {} MS", n, otg, moved, us / 1000));
            }
            Step::Call(t) => {
                let from = journal.len;
                let run = call(light, e, interp, journal, t, 'S', n, true, out)?;
                if let Some(slot) = tally.tables.iter_mut().find(|s| s.is_none()) { *slot = Some(run); }
                if let Some(path) = table_path(light, journal, from, t, n, out) { tally.path = Some(path); }
            }
            Step::Reference => {
                let d = reference_stage(light, e, n, out)?;
                tally.count_us = d.count_us;
                *derived = Some(d);
            }
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
                    if reg == R::HubpCntl && HUBP_TIMEOUT_STATUS.get(v) & !HUBP_TIMEOUT_STATUS.get(d.underflow[i]) != 0 { return Err(Failure::RequestTimeout { offset: x.offset(), value: v }); }
                }
                out.line(format_args!("S{} NO NEW UNDERFLOW: HUBP{} AND ODM{} (THE FIRMWARE'S), HUBP{} AND ODM{}", n, r.pipe, r.otg, q, q));
            }
            Step::Offer => {
                let (w, h, pitch) = (light.surface.width as u16, light.surface.height as u16, (light.surface.pitch * 4) as u32);
                e.offer(w, h, pitch).map_err(Failure::Screen)?;
                tally.offered = true;
                out.line(format_args!("S{} OFFERED THE SECOND SCREEN: {}x{}, PITCH {} BYTES, BGR, ITS PIXELS SLOT 5'S SURFACE; THE COMPOSITOR MIRRORS THE FIRST INTO IT",
                    n, w, h, pitch));
            }
            Step::Watch { frames } => {
                let d = derived.as_ref().ok_or(Failure::NoReference)?;
                tally.watch = Some(watch(light, e, n, frames, d, out)?);
            }
            // No line: in the survey's order the write before it is followed by its own.
            Step::Hold { us } => e.delay_us(us),
            Step::Late | Step::Begin(_) => {}
        }
    }
    Ok(tally)
}

/// Stage 15's watch with the pattern off: OTG1's frame count read without sleeping, both pipes' HUBP and ODM statuses at
/// each new frame. The firmware's showing an underflow or a timeout fails at once; pipe 1's are kept by frame, cleared
/// (HUBP_UNDERFLOW_CLEAR, HUBP_TIMEOUT_STATUS_CLEAR, OPTC_UNDERFLOW_CLEAR: one, then zero) and watched on, and after the
/// watch fail the stage. Then the frames are logged, sixty a line.
fn watch<E: Engine>(light: &Light, e: &mut E, n: u8, frames: u32, d: &Derived, out: &mut dyn Out) -> Result<(u32, u64, u64, u32), Failure> {
    let r = light.reference.ok_or(Failure::NoReference)?;
    let q = light.choice.pipe.otg;
    let at = |reg: R, i: u8| reg.at(i).ok_or(Failure::NoReference);
    let (count, h0, o0, h1, o1) = (at(R::OtgStatusFrameCount, q)?, at(R::HubpCntl, r.pipe)?, at(R::OptcInputGlobalControl, r.otg)?,
                                   at(R::HubpCntl, q)?, at(R::OptcInputGlobalControl, q)?);
    let hubp_clear = light.guard.check_at(h1, n).map_err(Failure::Refused)?;
    let odm_clear = light.guard.check_at(o1, n).ok();
    let new = |field: dcn::Field, v: u32, was: u32| field.get(v) & !field.get(was) != 0;
    let clears = HUBP_UNDERFLOW_CLEAR.mask() | HUBP_TIMEOUT_STATUS_CLEAR.mask();
    // The clears are written over the registers' own bits, their read-only statuses left out (dcn_2_1_0_sh_mask.h).
    const HUBP_STATUSES: u32 = 0x70FF_000A; // NO_OUTSTANDING_REQ, IN_BLANK, XRQ_NO_OUTSTANDING_REQ, TIMEOUT_STATUS, UNDERFLOW_STATUS
    const ODM_STATUSES: u32 = 0x8000_2C00; // UNDERFLOW_OCCURRED_STATUS, _INT_STATUS, _OCCURRED_CURRENT, DOUBLE_BUFFER_PENDING
    let frames = frames.min(WATCH_FRAMES);
    let mut marks = [0u8; WATCH_FRAMES as usize];
    let first = read(e, count)? & 0xFF_FFFF;
    let start = e.now_us();
    let (mut seen, mut last, mut gap, mut gap_at, mut skipped, mut cleared) = (0u32, start, 0u64, 0u32, 0u32, 0u32);
    let mut found: Option<(u32, Failure)> = None;
    let limit = frames as u64 * 20_000 + 200_000;
    while seen < frames {
        let now = e.now_us();
        if now.saturating_sub(start) > limit { return Err(Failure::NoFrames(seen)); }
        if now.saturating_sub(last) > gap { (gap, gap_at) = (now - last, seen + 1); }
        last = now;
        let c = (read(e, count)?.wrapping_sub(first) & 0xFF_FFFF).min(frames);
        // No new frame: asleep until the next look. The statuses are kept until cleared, so nothing is lost by it; a wake
        // later than a frame shows in the frames passed between two reads and in the longest gap (213-DRV-0111).
        if c <= seen { e.delay_us(ASLEEP_POLL_US); continue; }
        let (v0, w0, v1, w1) = (read(e, h0)?, read(e, o0)?, read(e, h1)?, read(e, o1)?);
        // The firmware's pipe: at once.
        if new(HUBP_UNDERFLOW_STATUS, v0, d.underflow[0]) { return Err(Failure::Underflow { offset: h0.offset(), value: v0 }); }
        if new(HUBP_TIMEOUT_STATUS, v0, d.underflow[0]) { return Err(Failure::RequestTimeout { offset: h0.offset(), value: v0 }); }
        if new(OPTC_UNDERFLOW_OCCURRED_STATUS, w0, d.underflow[1]) { return Err(Failure::Underflow { offset: o0.offset(), value: w0 }); }
        let (u1, t1, od1) = (new(HUBP_UNDERFLOW_STATUS, v1, d.underflow[2]), new(HUBP_TIMEOUT_STATUS, v1, d.underflow[2]),
                             new(OPTC_UNDERFLOW_OCCURRED_STATUS, w1, d.underflow[3]));
        for f in seen..c - 1 { marks[f as usize] = SKIPPED; skipped += 1; }
        marks[c as usize - 1] = u1 as u8 | (t1 as u8) << 1 | (od1 as u8) << 2;
        if found.is_none() {
            if u1 { found = Some((c, Failure::Underflow { offset: h1.offset(), value: v1 })); }
            else if t1 { found = Some((c, Failure::RequestTimeout { offset: h1.offset(), value: v1 })); }
            else if od1 { found = Some((c, Failure::Underflow { offset: o1.offset(), value: w1 })); }
        }
        if u1 || t1 { let v = v1 & !HUBP_STATUSES & !clears; e.write(hubp_clear, v | clears); e.write(hubp_clear, v); cleared += 1; }
        if let (true, Some(w)) = (od1, odm_clear) {
            let v = w1 & !ODM_STATUSES & !OPTC_UNDERFLOW_CLEAR.mask();
            e.write(w, v | OPTC_UNDERFLOW_CLEAR.mask());
            e.write(w, v);
            cleared += 1;
        }
        seen = c;
    }
    let us = e.now_us().saturating_sub(start).max(1);
    let millihertz = frames as u64 * 1_000_000_000 / us;
    out.line(format_args!("S{} WATCHED {} FRAMES OF OTG{} IN {} MS ({}.{:03} HZ) WITH THE PATTERN OFF: {} PASSED BETWEEN TWO READS; THE LONGEST GAP BETWEEN READS {} US, BEFORE FRAME {}",
        n, frames, q, us / 1000, millihertz / 1000, millihertz % 1000, skipped, gap, gap_at));
    let marked = &marks[..frames as usize];
    let with = |bit: u8| marked.iter().filter(|&&m| m != SKIPPED && m & bit != 0).count();
    match &found {
        Some((f, why)) => out.line(format_args!("S{} HUBP{}: UNDERFLOW IN {} FRAMES, REQUEST TIMEOUT IN {}; ODM{} UNDERFLOW IN {}; THE FIRST AT FRAME {} (ABOUT {} MS AFTER THE PATTERN WENT OFF): {}; CLEARED {} TIMES",
            n, q, with(1), with(2), q, with(4), f, *f as u64 * us / frames as u64 / 1000, why, cleared)),
        None => out.line(format_args!("S{} NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN {} FRAMES", n, frames)),
    }
    for (i, chunk) in marked.chunks(60).enumerate() {
        let mut line = [b'.'; 60];
        for (c, &m) in line.iter_mut().zip(chunk) {
            *c = if m == SKIPPED { b'-' } else { match m & 7 { 0 => b'.', 1 => b'U', 2 => b'T', 3 => b'B', 4 => b'O', _ => b'X' } };
        }
        out.line(format_args!("S{} FRAMES {:3} TO {:3}: {}", n, i * 60 + 1, i * 60 + chunk.len(), core::str::from_utf8(&line[..chunk.len()]).unwrap_or("")));
    }
    match found { Some((_, why)) => Err(why), None => Ok((frames, us, gap, gap_at)) }
}

/// Which path a table took through PHY A's control register bus: for table 12, whether the PHY was set up already (its
/// `RDPCSTX_SCRATCH` at 0x300, the VBIOS's mark) and the short path ran, or the long one with its writes through the bus.
/// For the stage's line: whether the short path ran, and the writes through the bus.
fn table_path(light: &Light, journal: &Journal, from: usize, t: Table, n: u8, out: &mut dyn Out) -> Option<(bool, u32)> {
    let p = light.choice.output.phy as u32;
    let (cr_addr, cr_data, scratch) = (seg2(0x2934 + 0xD8 * p), seg2(0x2935 + 0xD8 * p), seg2(0x2937 + 0xD8 * p));
    let made = || journal.items[from..journal.len].iter().flatten();
    let bus = made().filter(|(w, _, _)| w.reg().offset() == cr_addr || w.reg().offset() == cr_data).count();
    let data = made().filter(|(w, _, _)| w.reg().offset() == cr_data).count();
    let mark = made().find(|(w, _, _)| w.reg().offset() == scratch).map(|(_, old, _)| *old);
    match (t, mark) {
        (Table::SetPixelClock { pixclk_100hz, .. }, _) if pixclk_100hz > 0 && bus == 0 => { out.line(format_args!(
            "S{} THE PHY WAS SET UP ALREADY (SCRATCH {:#X}): THE SHORT PATH, NO WRITE THROUGH ITS BUS", n, mark.unwrap_or(0))); Some((true, 0)) }
        (Table::SetPixelClock { pixclk_100hz, .. }, _) if pixclk_100hz > 0 => { out.line(format_args!(
            "S{} THE PHY WAS NOT SET UP (SCRATCH {:#X}): THE LONG PATH: {} WRITES THROUGH ITS BUS ({} INTO THE PHY)", n, mark.unwrap_or(0), bus, data)); Some((false, bus as u32)) }
        _ if bus > 0 => { out.line(format_args!("S{} TABLE {} MADE {} WRITES THROUGH PHY {}'S CONTROL REGISTER BUS ({} INTO THE PHY)",
            n, t.index(), bus, LETTERS[p as usize], data)); Some((false, bus as u32)) }
        _ => None,
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
    // Its frame rate: from one frame's edge to another, MEASURED_FRAMES frames later or more. Only the two edges are
    // waited for awake; the frames between are slept through, and a late wake only makes the count longer (213-DRV-0111).
    let count = R::OtgStatusFrameCount.at(r.otg).ok_or(Failure::NoReference)?;
    let frame = |e: &mut E| read(e, count).map(|v| v & 0xFF_FFFF);
    let begin = e.now_us();
    let first = frame(e)?;
    let mut edge = first;
    while edge == first {
        if e.now_us().saturating_sub(begin) > MEASURE_LIMIT_US { return Err(Failure::NoFrames(0)); }
        e.delay_us(EDGE_POLL_US);
        edge = frame(e)?;
    }
    let start = e.now_us();
    let mut frames = 0;
    while frames + 1 < MEASURED_FRAMES {
        if e.now_us().saturating_sub(begin) > MEASURE_LIMIT_US { return Err(Failure::NoFrames(frames)); }
        e.delay_us(ASLEEP_POLL_US);
        frames = frame(e)?.wrapping_sub(edge) & 0xFF_FFFF;
    }
    let before = frames;
    while frames == before {
        if e.now_us().saturating_sub(begin) > MEASURE_LIMIT_US { return Err(Failure::NoFrames(frames)); }
        e.delay_us(EDGE_POLL_US);
        frames = frame(e)?.wrapping_sub(edge) & 0xFF_FFFF;
    }
    let us = e.now_us().saturating_sub(start).max(1);
    let count_us = e.now_us().saturating_sub(begin);
    let (h0, v0) = ((r.h_total + 1) as u64, (r.v_total + 1) as u64);
    let pixel_khz = (h0 * v0 * frames as u64 * 1000 / us) as u32;
    // The reference clock, by PER_HTOTAL (Display Core's ref/pix x htotal x 2^8, which the firmware follows) and by
    // REF_FREQ_TO_PIX_FREQ (which on the Ryzen it does not: 0.1632 for 48 MHz over 268.5); a crystal's round MHz kept.
    let by_htotal = (r.per_htotal as u64 * pixel_khz as u64 / (h0 * 256)) as u32;
    let by_ratio = ((r.ref_to_pix as u64 * pixel_khz as u64) >> 19) as u32;
    let round_mhz = (by_htotal + 500) / 1000 * 1000;
    let reference_khz = if by_htotal.abs_diff(round_mhz) * 200 <= round_mhz { round_mhz } else { by_htotal };
    let millihertz = frames as u64 * 1_000_000_000 / us;
    out.line(format_args!("S{} OTG{} COUNTED {} FRAMES IN {} US: {}.{:03} HZ, ITS PIXEL CLOCK {} KHZ; THE REFERENCE CLOCK {} KHZ",
        n, r.otg, frames, us, millihertz / 1000, millihertz % 1000, pixel_khz, reference_khz));
    out.line(format_args!("S{} THE REFERENCE CLOCK: {} KHZ BY PER_HTOTAL, {} KHZ BY REF_FREQ_TO_PIX_FREQ; REFCLK_CNTL {:08X} ({}), GLOBAL TIMER {:08X} (REFDIV {})",
        n, by_htotal, by_ratio, r.refclk_cntl, if r.refclk_cntl & 1 == 0 { "THE CRYSTAL" } else { "ANOTHER SOURCE" }, r.timer_cntl, r.timer_cntl & 0xF));
    // The formulas (Display Core's dml21_rq_dlg_get_dlg_params, display_rq_dlg_calc_21.c, Linux v7.0) on the firmware's
    // own pipe, with that clock: they test what the firmware followed, not the formulas (it sets its own DLG).
    let ratio0 = ((reference_khz as u64) << 19) / pixel_khz.max(1) as u64;
    let at = |ratio: u64, x: u64| ((ratio * x) >> 19) as u32;
    let (per0, hbe0, line0) = (at(ratio0, h0 * 256), at(ratio0, r.h_blank_end as u64), at(ratio0, r.width as u64));
    out.line(format_args!("S{} PIPE {} BY FORMULA: REF/PIX {} (SET {}), PER HTOTAL {} (SET {}), H_BLANK_END {} (SET {}), LINE {} (SET {}), V_BLANK_END {} (DLG {})",
        n, r.pipe, crate::batches::Ratio19(Some(ratio0 as u32)), crate::batches::Ratio19(Some(r.ref_to_pix)), per0, r.per_htotal, hbe0,
        r.refcyc_h_blank_end, line0, r.line_delivery, r.v_blank_end, r.dlg_v_blank_end));
    // Pipe 1 for step 3's mode, as Display Core computes it: a linear 32-bit surface asks 256 bytes, 64 pixels, a request.
    let otg = light.timing.otg().map_err(|_| Failure::NoReference)?;
    let pix1 = light.timing.pixel_khz.max(1) as u64;
    let ratio1 = ((reference_khz as u64) << 19) / pix1;
    let h1 = otg.h_total as u64 + 1;
    // Each value from the clocks themselves, as Display Core's doubles give it (truncated last), not from the rounded ratio.
    let refcyc = |x: u64| (reference_khz as u64 * x / pix1) as u32;
    let line1 = refcyc(light.timing.h_active as u64);
    let requests1 = (light.timing.h_active as u64).div_ceil(64).max(1);
    let req1 = refcyc(light.timing.h_active as u64 * 1024 / requests1);
    // The prefetch's delivery in the same proportion to the line's as the firmware's.
    let scaled = |x: u32, num: u32, den: u32| if den == 0 { x } else { (x as u64 * num as u64 / den as u64) as u32 };
    // The underflow statuses now, so that stage 15 fails on a new one only.
    let q = light.choice.pipe.otg;
    let status = |e: &mut E, reg: R, i: u8| reg.at(i).ok_or(Failure::NoReference).and_then(|x| read(e, x));
    let underflow = [status(e, R::HubpCntl, r.pipe)?, status(e, R::OptcInputGlobalControl, r.otg)?, status(e, R::HubpCntl, q)?,
                     status(e, R::OptcInputGlobalControl, q)?];
    // Pipe 1's global sync for its plane, as dcn20_program_tg writes DML's before the plane goes live: VSTARTUP stage
    // 3's; VUPDATE's offset a quarter line (DML's VUpdateOffsetPix); its width and VREADY's offset the firmware's times
    // (DCFCLK deep sleep and DPPCLK cycles, clocks both pipes share) at pipe 1's pixel clock, rounded up.
    let (fw_width, fw_vready) = (VUPDATE_WIDTH.get(r.sync[1]), VREADY_OFFSET.get(r.sync[2]));
    let at_pix1 = |x: u32| (x as u64 * pix1).div_ceil(pixel_khz.max(1) as u64).max(1) as u32;
    let vstartup1 = dcn::vstartup(&otg);
    let (vupdate_offset, vupdate_width, vready_offset) = ((h1 as u32).div_ceil(4), at_pix1(fw_width), at_pix1(fw_vready));
    let vready_after_vsync = (vstartup1.saturating_sub((vready_offset + vupdate_width + vupdate_offset) / h1 as u32) <= otg.v_blank_end) as u32;
    let qos_high_wm = refcyc(4 * h1).min(0x3FFF);
    let d = Derived {
        vupdate_offset, vupdate_width, vready_offset, vready_after_vsync, qos_high_wm, count_us,
        firmware_pixel_khz: pixel_khz, reference_khz, ref_to_pix: ratio1 as u32, per_htotal: refcyc(h1 * 256),
        refcyc_h_blank_end: refcyc(otg.h_blank_end as u64), dlg_v_blank_end: otg.v_blank_end, line_delivery: line1,
        req_delivery: req1, min_dst_y_next_start: otg.v_blank_start * 4,
        // The prefetch's at the same rate as the line's (Display Core's for a vertical ratio of 1), or the firmware's proportion.
        line_delivery_pre: if r.line_delivery == 0 { line1 } else { scaled(line1, r.line_delivery_pre, r.line_delivery) },
        req_delivery_pre: if r.req_delivery == 0 { req1 } else { scaled(req1, r.req_delivery_pre, r.req_delivery) },
        underflow,
    };
    out.line(format_args!("S{} PIPE {} FOR {}x{} AT {} KHZ: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, LINE {}, REQ {} ({} REQUESTS A LINE)",
        n, light.choice.pipe.otg, light.timing.h_active, light.timing.v_active, pix1, crate::batches::Ratio19(Some(d.ref_to_pix)),
        d.per_htotal, d.refcyc_h_blank_end, d.dlg_v_blank_end, d.line_delivery, d.req_delivery, requests1));
    out.line(format_args!("S{} PIPE {}: NEXT START {} (V_BLANK_START {} x 4), PREFETCH LINE {} REQ {}; UNDERFLOW STATUSES {:08X} {:08X} {:08X} {:08X}",
        n, q, d.min_dst_y_next_start, otg.v_blank_start, d.line_delivery_pre, d.req_delivery_pre, underflow[0], underflow[1], underflow[2], underflow[3]));
    out.line(format_args!("S{} PIPE {}'S SYNC FOR ITS PLANE: VSTARTUP {} (STAGE 3'S), VUPDATE OFFSET {} WIDTH {}, VREADY {} (THE FIRMWARE'S OTG{}: VSTARTUP {}, VUPDATE {} WIDTH {}, VREADY {}); VREADY AT OR AFTER VSYNC {} (THE FIRMWARE'S HUBP {})",
        n, q, vstartup1, d.vupdate_offset, d.vupdate_width, d.vready_offset, r.otg, VSTARTUP_START.get(r.sync[0]), VUPDATE_OFFSET.get(r.sync[1]), fw_width, fw_vready,
        d.vready_after_vsync, r.vready));
    out.line(format_args!("S{} PIPE {}'S QOS: LEVEL {} (THE FIRMWARE'S {}), HIGH WATERMARK {} (4 LINES; THE FIRMWARE'S {}); HUBPREQ_DEBUG_DB {:08X} (THE FIRMWARE'S {:08X})",
        n, q, QOS_LEVEL, r.qos_level, d.qos_high_wm, QOS_LEVEL_HIGH_WM.get(copied(&r, R::HubpreqTtuQosWm)),
        DLG_TEST_MODE_OFF, r.debug_db));
    Ok(d)
}

/// Stages 11 to 15 for pipe 1, from the reference and what stage 9 derived, in Display Core's order (dcn20_program_pipe,
/// dcn20_update_dchubp_dpp, hubp21_setup, mpc1_insert_plane, hubp2_set_blank; Linux v7.0): its clocks; its DPP as the
/// firmware's with a 1920x1080 recout; its HUBP, still blanked, on the surface with the firmware's requestor and
/// prefetch and the derived DLG and TTU; its MPCC to its OPP; then unblanked and the pattern off.
pub fn pipe_plan(light: &Light, r: &Reference, d: &Derived) -> Result<Plan, Failure> {
    use R::*;
    let (q, o) = (light.choice.pipe.otg, light.choice.pipe.opp);
    // Pipe 1's MPCC: unused (15) or already its own OPP's, and outside the firmware's tree; its OPP's mux free or on it.
    let mine = |x: u32, own: u32| x == 0xF || x == own;
    if !mine(r.mpcc_opp, o as u32) || r.firmware_mux == q as u32 || (r.firmware_bot == q as u32 && r.pipe != q) {
        return Err(Failure::MpccInUse { mpcc: q, opp_id: r.mpcc_opp, firmware_mux: r.firmware_mux, firmware_bot: r.firmware_bot });
    }
    if !mine(r.mux, q as u32) { return Err(Failure::OutputMuxInUse { opp: o, mux: r.mux }); }
    let (w, h) = (light.timing.h_active as u32, light.timing.v_active as u32);
    let at = |reg: R, i: u8| reg.at(i).ok_or(Failure::PlanFull);
    let mut p = Plan::default();
    let put = |p: &mut Plan, reg: R, i: u8, fields: &[(dcn::Field, u32)]| -> Result<(), Failure> { p.update(at(reg, i)?, fields).map_err(|_| Failure::PlanFull) };
    let mark = |p: &mut Plan, n: u8| p.push(Step::Begin(n)).map_err(|_| Failure::PlanFull);
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
    // Its OTG's global sync for the plane first, as dcn20_program_tg; two frames for it to take.
    put(&mut p, OtgVupdateParam, q, &[(VUPDATE_OFFSET, d.vupdate_offset), (VUPDATE_WIDTH, d.vupdate_width)])?;
    put(&mut p, OtgVreadyParam, q, &[(VREADY_OFFSET, d.vready_offset)])?;
    p.push(Step::Frames { otg: q, frames: 2, timeout_us: 200_000 }).map_err(|_| Failure::PlanFull)?;
    for reg in [HubpSurfaceConfig, HubpTilingConfig, HubpReqSizeConfig, HubpReqSizeConfigC, HubpreqExpansionMode, HubpretControl,
                HubpreqSurfaceControl, HubpreqFlipControl, HubpreqVmidSettings0, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh,
                HubpreqVmMxL1TlbCntl, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0, HubpreqVblankParameters1,
                HubpreqVblankParameters2, HubpreqVblankParameters3, HubpreqVblankParameters4, HubpreqGlobalTtuCntl,
                HubpreqCur0TtuCntl1, HubpreqDstYDeltaDrqLimit] {
        put(&mut p, reg, q, &[(WHOLE, copied(r, reg))])?;
    }
    // The QoS as Display Core's DLG sets it for every plane (display_rq_dlg_calc_21.c): level 8 with its ramp, the high
    // watermark four lines.
    put(&mut p, HubpreqTtuQosWm, q, &[(QOS_LEVEL_LOW_WM, 0), (QOS_LEVEL_HIGH_WM, d.qos_high_wm)])?;
    put(&mut p, HubpreqCur0TtuCntl0, q, &[(REFCYC_PER_REQ_DELIVERY, REFCYC_PER_REQ_DELIVERY.get(copied(r, HubpreqCur0TtuCntl0))),
        (QOS_LEVEL_FIXED, QOS_LEVEL), (QOS_RAMP_DISABLE, 0)])?;
    let surface = r.fb_base + SURFACE_OFFSET;
    put(&mut p, HubpreqSurfacePitch, q, &[(WHOLE, light.surface.pitch as u32 - 1)])?;
    p.push(Step::ReadBack { reg: at(HubpreqSurfacePitch, q)?, field: PITCH }).map_err(|_| Failure::PlanFull)?;
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
    put(&mut p, HubpreqSurf0TtuCntl0, q, &[(REFCYC_PER_REQ_DELIVERY, d.req_delivery), (QOS_LEVEL_FIXED, QOS_LEVEL), (QOS_RAMP_DISABLE, 0)])?;
    put(&mut p, HubpreqSurf0TtuCntl1, q, &[(REFCYC_PER_REQ_DELIVERY, d.req_delivery_pre)])?;
    // hubp2_vready_at_or_After_vsync: the DLG's test mode off, then VREADY's place against the sync by its rule.
    put(&mut p, HubpreqDebugDb, q, &[(WHOLE, DLG_TEST_MODE_OFF)])?;
    put(&mut p, HubpCntl, q, &[(HUBP_VREADY_AT_OR_AFTER_VSYNC, d.vready_after_vsync)])?;
    mark(&mut p, 14)?;
    put(&mut p, MpccBotSel, q, &[(WHOLE, 0xF)])?;
    put(&mut p, MpccControl, q, &[(WHOLE, copied(r, MpccControl))])?;
    put(&mut p, MpccTopSel, q, &[(WHOLE, q as u32)])?;
    put(&mut p, MpccOppId, q, &[(WHOLE, o as u32)])?;
    put(&mut p, MpcOutMux, o, &[(MPCC_SEL, q as u32)])?;
    mark(&mut p, 15)?;
    put(&mut p, HubpCntl, q, &[(HUBP_BLANK_EN, 0), (HUBP_TTU_DISABLE, 0)])?;
    p.push(Step::Frames { otg: light.choice.pipe.otg, frames: 3, timeout_us: 200_000 }).map_err(|_| Failure::PlanFull)?;
    p.push(Step::NoUnderflow).map_err(|_| Failure::PlanFull)?;
    put(&mut p, DpgControl, o, &[(DPG_EN, 0)])?;
    p.push(Step::Watch { frames: light.watch_frames }).map_err(|_| Failure::PlanFull)?;
    Ok(p)
}

/// Stage 10: the picture drawn row by row into slot 5's memory, a few pixels read back. No register written.
fn draw_stage<E: Engine>(light: &Light, e: &mut E, n: u8, out: &mut dyn Out) -> Result<(), Failure> {
    let size = light.surface;
    let lines = picture::Lines::on(name(&light.choice.output), light.build, size);
    let mut row = [0u32; picture::MAX_WIDTH];
    for y in 0..size.height {
        picture::row(&lines, y, &mut row);
        if !e.row(y, size.pitch, &row[..size.width]) { return Err(Failure::NoSurface); }
    }
    let samples = picture::samples(size);
    for (x, y) in samples {
        if e.sample(x, y, size.pitch) != Some(picture::pixel(&lines, x, y)) { return Err(Failure::SurfaceDiffers { x: x as u32, y: y as u32 }); }
    }
    out.line(format_args!("S{} DRAWN: {}x{} AT SLOT 5'S START, PITCH {} PIXELS ({} BYTES); {} SAMPLES READ BACK AS DRAWN", n,
        size.width, size.height, size.pitch, size.pitch * 4, samples.len()));
    out.line(format_args!("S{} ITS TEXT: MIND CORE / DISPLAY / {} {}X{} / BUILD {}", n, name(&light.choice.output), size.width, size.height, light.build));
    Ok(())
}

/// Undoes stage `n`: its off call, if it has one, brings the output down in the VBIOS's own order; then every register
/// the stage wrote is put back, last first, and then every register only the off call wrote, so that all are as before
/// the stage (PHY A was off). The PHY's own control register bus (`RDPCS_TX_CR_ADDR`/`_DATA`) is left as it is: a
/// write to its data register is a write into the PHY, so putting it back would write stale values into the PHY. What
/// the stage set inside the PHY stays; the undo says how much. Whether something in it went wrong: the off call's table
/// stopped, or the compositor did not take screen_gone (every write is put back all the same).
fn undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, at_once: bool, out: &mut dyn Out) -> bool {
    let p = light.choice.output.phy as u32;
    let (cr_addr, cr_data) = (seg2(0x2934 + 0xD8 * p), seg2(0x2935 + 0xD8 * p));
    // At once (an underflow): the stage's own writes put back, last first, before any line is logged.
    let mut i = journal.len;
    while at_once && i > 0 {
        i -= 1;
        let Some((w, old, stage)) = journal.items[i] else { continue };
        if stage == n && w.reg().offset() != cr_addr && w.reg().offset() != cr_data { e.write(w, old); }
    }
    let (mut off, mut wrong) = (Journal::default(), false);
    if let Some(t) = light.off_call(n) {
        if let Err(why) = call(light, e, interp, &mut off, t, 'U', n, true, out) {
            out.line(format_args!("U{} TABLE {} STOPPED ({}); RESTORING STAGE {}'S WRITES ANYWAY", n, t.index(), why, n));
            wrong = true;
        }
    }
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
        restored += 1;
        if at_once { out.line(format_args!("U{} RESTORED {} = {:08X} AT ONCE", n, Name(w.reg()), old)); continue; }
        e.write(w, old);
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
    // Stage 16's undo, before stage 15's blanks the pipe: the compositor lets the surface go (screen_gone).
    if n == 16 {
        match e.gone() {
            Ok(()) => out.line(format_args!("U{} SCREEN_GONE: THE COMPOSITOR HAS LET THE SURFACE GO", n)),
            Err(why) => { out.line(format_args!("U{} SCREEN_GONE NOT TAKEN: {}; THE UNDO GOES ON", n, why)); wrong = true; }
        }
    }
    out.line(format_args!("U{} {} UNDONE: {} WRITES RESTORED", n, STAGE_NAMES[n as usize - 1], restored));
    wrong
}

/// Stages `last` to 1 undone, last first, for a take-back that is not a failure's (an unplug, the build's own undo): the
/// production order holds their detail, so a stage whose undo went wrong has it logged at once, and every line after it.
/// How many went wrong.
fn undo_all<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, last: u8, out: &mut dyn Out) -> u8 {
    let mut wrong = 0;
    for m in (1..=last).rev() { if undo(light, e, interp, journal, m, false, out) { out.failed(); wrong += 1; } }
    wrong
}

// The end of a take-back's last line: what went wrong in it, if anything, logged above it.
struct Wrong(u8);
impl fmt::Display for Wrong {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 > 0 { write!(f, "; {} STAGES' UNDOS WENT WRONG, THEIR DETAIL ABOVE", self.0) } else { Ok(()) }
    }
}

/// Step 3: stages 1 to `allowed`, each logged with its writes before any is made, the firmware's pipe read before and
/// after; a failure undoes the stage and every one before it.
pub fn run<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, out: &mut dyn Out) -> Outcome {
    run_timed(light, e, interp, journal, allowed, false, out).0
}

/// Step 3 as `run`, then, when no stage failed, the stages that ran undone, last first: the undo is seen on the hardware,
/// and nothing is left for the next boot.
pub fn run_then_undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, out: &mut dyn Out) -> Outcome {
    run_timed(light, e, interp, journal, allowed, true, out).0
}

/// A time in milliseconds, to a tenth.
struct Ms(u64);
impl fmt::Display for Ms { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}.{}", self.0 / 1000, self.0 % 1000 / 100) } }

/// Step 3 as `run` or `run_then_undo`, with its times (213-DRV-0098). Its order is `out`'s: the same reads, writes and
/// checks either way; the survey's pauses and the lines are `out`'s alone.
pub fn run_timed<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, allowed: u8, then_undo: bool, out: &mut dyn Out) -> (Outcome, Times) {
    let allowed = allowed.min(STAGES);
    let start = e.now_us();
    let mut times = Times { start_us: start, ..Times::default() };
    light.intro(allowed, false, out);
    if then_undo { out.note(format_args!("S0 THIS BUILD UNDOES THE STAGES IT RAN, LAST FIRST, BEFORE IT STOPS")); }
    let baseline = match light.watch.read(e) {
        Ok(v) => v,
        Err(why) => { out.note(format_args!("STEP 3 NOT RUN: THE FIRMWARE'S PIPE: {}", why)); return (Outcome::Failed { stage: 0, why }, times); }
    };
    journal.baseline = Some(baseline);
    let beside = match light.beside.read(e) {
        Ok(v) => v,
        Err(why) => { out.note(format_args!("STEP 3 NOT RUN: THE OTHER LIT PIPE: {}", why)); return (Outcome::Failed { stage: 0, why }, times); }
    };
    journal.beside = Some(beside);
    // Once while lit: a start of display again (init restarts a program that faults) finds the mark and runs no stage;
    // else the mark is left before stage 1, and only an unplug's taking back clears it.
    if allowed > 0 {
        let Some(mark) = R::OtgVerticalInterrupt2Position.at(light.choice.pipe.otg) else { return (Outcome::Failed { stage: 0, why: Failure::StartedAgain }, times) };
        let was = match read(e, mark) { Ok(v) => v, Err(why) => { out.note(format_args!("STEP 3 NOT RUN: ITS MARK: {}", why)); return (Outcome::Failed { stage: 0, why }, times); } };
        if was & 0x7FFF == RESTART_MARK {
            out.note(format_args!("STEP 3 NOT RUN: {} ({} HOLDS {:04X})", Failure::StartedAgain, Name(mark), was));
            return (Outcome::Failed { stage: 0, why: Failure::StartedAgain }, times);
        }
        let Ok(w) = light.guard.check(mark) else { return (Outcome::Failed { stage: 0, why: Failure::StartedAgain }, times) };
        e.write(w, RESTART_MARK);
        journal.mark = Some((w, was));
        out.line(format_args!("S0 MARKED {} = {:04X} (WAS {:08X}): A START OF DISPLAY AGAIN RUNS NO STAGE; A MACHINE BOOT OR AN UNPLUG CLEARS IT",
            Name(mark), RESTART_MARK, was));
    }
    let attached = match light.attached.read(e) {
        Ok(v) => v,
        Err(why) => { out.note(format_args!("STEP 3 NOT RUN: THE ATTACHED HUBPS: {}", why)); return (Outcome::Failed { stage: 0, why }, times); }
    };
    let mut derived: Option<Derived> = None;
    let mut late: Option<Plan> = None;
    for n in 1..=STAGES {
        let name = STAGE_NAMES[n as usize - 1];
        out.part();
        if n > allowed {
            out.line(format_args!("S{} {}: NOT RUN (THIS BUILD MAY RUN {}); WOULD:", n, name, allowed));
            light.plan_lines(n, "WOULD", late.as_ref(), out);
            continue;
        }
        out.line(format_args!("S{} {}: STAGE {} OF {} NEXT; ITS WRITES:", n, name, n, STAGES));
        light.plan_lines(n, "WILL", late.as_ref(), out);
        out.pause(PAUSE_BEFORE_STAGE_MS);
        let begin = e.now_us();
        let result = stage(light, e, interp, journal, n, &mut derived, late.as_ref(), out).and_then(|tally| {
            let now = light.watch.read(e)?;
            if let Some((reg, before, after)) = light.watch.changed(&baseline, &now) { return Err(Failure::FirmwareChanged { offset: reg.offset(), before, after }); }
            let now = light.beside.read(e)?;
            if let Some((reg, before, after)) = light.beside.changed(&beside, &now) { return Err(Failure::LitChanged { offset: reg.offset(), before, after }); }
            // Stage 9 done: stages 11 to 15 built from its reference.
            if let (9, Some(r), Some(d)) = (n, light.reference, derived) { late = Some(pipe_plan(light, &r, &d)?); journal.statuses = Some(d.underflow); }
            // The attached HUBP is guarded until step 4 takes it, from stage 11.
            if n < ATTACHED_FROM || n >= dcn::PIPE_WRITES_FROM || light.attached.is_empty() { return Ok(tally); }
            let now = light.attached.read(e)?;
            match light.attached.changed(&attached, &now) { Some((reg, before, after)) => Err(Failure::AttachedChanged { offset: reg.offset(), before, after }), None => Ok(tally) }
        });
        match result {
            Ok(tally) => {
                let us = e.now_us().saturating_sub(begin);
                times.stages_us[n as usize - 1] = us;
                times.tables_us += tally.tables.iter().flatten().map(|t| t.us).sum::<u64>();
                (times.waits_us, times.count_us) = (times.waits_us + tally.wait_us, times.count_us + tally.count_us);
                if let Some((_, watch_us, _, _)) = tally.watch { times.watch_us += watch_us; }
                if tally.offered { times.offered_us = Some(e.now_us().saturating_sub(start)); }
                if (ATTACHED_FROM..dcn::PIPE_WRITES_FROM).contains(&n) && !light.attached.is_empty() { out.line(format_args!("S{} THE ATTACHED HUBPS UNCHANGED, BLANKED ({} REGISTERS)", n, light.attached.len())); }
                out.note(format_args!("{}", Done { light, n, us, t: &tally, d: derived.as_ref() }));
            }
            // Stage 9 reads and derives, stage 10 draws into memory nothing scans, stage 16 offers: when one is not ready, no
            // register of its needs undoing, and the stages before it stay (after 16's, the picture on the second output on
            // its own). A firmware's pipe changed or a register unreadable still undoes.
            Err(why) if (n == 9 || n == 10 || n == 16) && why.not_ready() => {
                out.failed();
                out.note(format_args!("S{} {} NOT READY: {}; STAGES 1 TO {} STAY AS THEY ARE, STEP 4 STOPS HERE", n, name, why, n - 1));
                if then_undo {
                    out.line(format_args!("STEP 3 RAN {} OF {} STAGES ({} WRITES); UNDOING STAGES {} TO 1 AS THIS BUILD ASKS", n - 1, STAGES, journal.len, n - 1));
                    for m in (1..n).rev() { undo(light, e, interp, journal, m, false, out); }
                }
                times.total_us = e.now_us().saturating_sub(start);
                out.note(format_args!("STEP 3 STOPPED AFTER STAGE {} OF {}: STAGE {} NOT READY; {} WRITES MADE {}", n - 1, STAGES, n, journal.len,
                    if then_undo { "ARE UNDONE" } else { "ARE LEFT IN PLACE" }));
                return (Outcome::NotReady { done: n - 1, stage: n, why }, times);
            }
            Err(why) => {
                // An underflow: the stage taken back before the line that says so (the pattern on again, the HUBP blanked),
                // and before the detail held of it is logged.
                let at_once = matches!(why, Failure::Underflow { .. } | Failure::RequestTimeout { .. });
                if at_once { undo(light, e, interp, journal, n, true, out); }
                out.failed();
                out.note(format_args!("S{} {} FAILED: {}; {}UNDOING STAGES {} TO 1", n, name, why,
                    if at_once { "ITS WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; " } else { "" }, if at_once { n - 1 } else { n }));
                for m in (1..if at_once { n } else { n + 1 }).rev() { undo(light, e, interp, journal, m, false, out); }
                let after = light.watch.read(e).ok();
                let same = after.map(|a| light.watch.changed(&baseline, &a).is_none());
                let lit = light.beside.read(e).ok().map(|a| light.beside.changed(&beside, &a).is_none());
                times.total_us = e.now_us().saturating_sub(start);
                out.note(format_args!("STEP 3 FAILED AT STAGE {} AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE {}{}", n,
                    match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }, light.lit_too(lit)));
                return (Outcome::Failed { stage: n, why }, times);
            }
        }
        out.pause(PAUSE_AFTER_STAGE_MS);
    }
    out.part();
    times.total_us = e.now_us().saturating_sub(start);
    if allowed > 0 {
        let sum = |a: usize, b: usize| Ms(times.stages_us[a - 1..b].iter().sum());
        out.note(format_args!("STEP 3'S TIMES (MS): IN ALL {}; STAGES 1-8 {}, 9 {}, 10 {}, 11-14 {}, 15 {}, 16 {}; TABLES {}, WAITS {}, 9'S COUNT {}, 15'S WATCH {}",
            Ms(times.total_us), sum(1, 8), sum(9, 9), sum(10, 10), sum(11, 14), sum(15, 15), sum(16, 16), Ms(times.tables_us), Ms(times.waits_us),
            Ms(times.count_us), Ms(times.watch_us)));
    }
    if then_undo && allowed > 0 {
        out.line(format_args!("STEP 3 RAN {} OF {} STAGES ({} WRITES); UNDOING STAGES {} TO 1 AS THIS BUILD ASKS", allowed, STAGES, journal.len, allowed));
        let wrong = undo_all(light, e, interp, journal, allowed, out);
        let same = light.watch.read(e).ok().map(|a| light.watch.changed(&baseline, &a).is_none());
        let lit = light.beside.read(e).ok().map(|a| light.beside.changed(&beside, &a).is_none());
        if same != Some(true) || (light.lit.is_some() && lit != Some(true)) { out.failed(); }
        out.note(format_args!("STEP 3 UNDONE AFTER STAGE {} OF {} AS THIS BUILD ASKS; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE {}{}{}", allowed, STAGES,
            match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }, light.lit_too(lit), Wrong(wrong)));
        return (Outcome::Undone { done: allowed }, times);
    }
    out.note(format_args!("STEP 3 STOPPED AFTER STAGE {} OF {} AS THIS BUILD ALLOWS; {} WRITES MADE ARE LEFT IN PLACE", allowed, STAGES, journal.len));
    (Outcome::Stopped { done: allowed }, times)
}

/// Steps 3 and 4 taken back for an unplug (213-DRV-0096): stages `done` to 1 undone, last first, as a failure's undo
/// takes them back (stage 16's first lets the compositor go: screen_gone), then the mark put back as it was before step 3,
/// so that a monitor plugged in runs the stages again, and the journal emptied for that run. A stage whose undo went
/// wrong, or a firmware's pipe that does not read as before, has the detail logged before the last line. Whether the
/// firmware's pipe reads as it did when step 3 began.
pub fn take_back<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, done: u8, out: &mut dyn Out) -> bool {
    out.part();
    let (start, done) = (e.now_us(), done.min(STAGES));
    out.note(format_args!("STEP 3 TAKEN BACK: HPD {} SENSES NO MONITOR; UNDOING STAGES {} TO 1 ({} WRITES)", light.choice.output.hpd + 1, done, journal.len));
    let wrong = undo_all(light, e, interp, journal, done, out);
    if let Some((w, was)) = journal.mark.take() {
        e.write(w, was);
        out.line(format_args!("U0 RESTORED {} = {:08X}: THE MARK OF STAGES IN PLACE CLEARED", Name(w.reg()), was));
    }
    let same = journal.baseline.and_then(|b| light.watch.read(e).ok().map(|now| light.watch.changed(&b, &now).is_none()));
    let lit = journal.beside.and_then(|b| light.beside.read(e).ok().map(|now| light.beside.changed(&b, &now).is_none()));
    journal.clear();
    if same != Some(true) || (light.lit.is_some() && lit != Some(true)) { out.failed(); }
    out.note(format_args!("STEP 3 TAKEN BACK IN {} MS: THE SECOND PIPE IS OFF, ITS MARK CLEARED; THE FIRMWARE'S PIPE {}{}{}", Ms(e.now_us().saturating_sub(start)),
        match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }, light.lit_too(lit), Wrong(wrong)));
    same == Some(true) && (light.lit.is_none() || lit == Some(true))
}

/// The standing check's account (213-DRV-0103): the checks made since it last said so, the longest time between two
/// while the output was lit, how often and how long it was dark since (an unplug's take-back to the relight: no picture
/// to check), and when it said so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Standing { pub checks: u64, pub longest_us: u64, pub dark: u32, pub dark_us: u64, last_us: Option<u64>, dark_since: Option<u64>, said_us: u64 }

impl Standing {
    pub fn new(now_us: u64) -> Standing { Standing { said_us: now_us, ..Standing::default() } }
    /// The output went dark (an unplug's take-back): no time between two checks spans it; it is counted apart.
    pub fn dark(&mut self, now_us: u64) { (self.last_us, self.dark_since, self.dark) = (None, Some(now_us), self.dark + 1); }
}

/// The standing check after the offer (213-DRV-0103): the firmware's HUBP and ODM statuses, then pipe 1's, against what
/// stage 9 found. These statuses are latched: each has its clear bit (`HUBP_UNDERFLOW_CLEAR`, `HUBP_TIMEOUT_STATUS_CLEAR`,
/// `OPTC_UNDERFLOW_CLEAR`), the ODM's live one is apart (`OPTC_UNDERFLOW_OCCURRED_CURRENT`), Display Core reads and then
/// clears them (`dcn10_did_underflow_occur`), and only the watch clears pipe 1's. So an underflow since stage 9 shows at
/// the next check however short it was. A new one is the failure; none, nothing logged but, at most every
/// `STANDING_SAID_US`, how many checks were made and the longest time between two.
pub fn standing_check<E: Engine>(light: &Light, e: &mut E, journal: &Journal, account: &mut Standing, out: &mut dyn Out) -> Result<(), Failure> {
    let (Some(r), Some(was)) = (light.reference, journal.statuses) else { return Err(Failure::NoReference) };
    let q = light.choice.pipe.otg;
    let at = |reg: R, i: u8| reg.at(i).ok_or(Failure::NoReference);
    let regs = [at(R::HubpCntl, r.pipe)?, at(R::OptcInputGlobalControl, r.otg)?, at(R::HubpCntl, q)?, at(R::OptcInputGlobalControl, q)?];
    let new = |field: dcn::Field, v: u32, before: u32| field.get(v) & !field.get(before) != 0;
    for (i, reg) in regs.iter().enumerate() {
        let v = read(e, *reg)?;
        let (underflow, timeout) = if i % 2 == 0 { (new(HUBP_UNDERFLOW_STATUS, v, was[i]), new(HUBP_TIMEOUT_STATUS, v, was[i])) } else { (new(OPTC_UNDERFLOW_OCCURRED_STATUS, v, was[i]), false) };
        if underflow { return Err(Failure::Underflow { offset: reg.offset(), value: v }); }
        if timeout { return Err(Failure::RequestTimeout { offset: reg.offset(), value: v }); }
    }
    let now = e.now_us();
    if let Some(since) = account.dark_since.take() { account.dark_us += now.saturating_sub(since); }
    if let Some(last) = account.last_us { account.longest_us = account.longest_us.max(now.saturating_sub(last)); }
    (account.last_us, account.checks) = (Some(now), account.checks + 1);
    if now.saturating_sub(account.said_us) >= STANDING_SAID_US {
        let minutes = now.saturating_sub(account.said_us) / 60_000_000;
        match account.dark {
            0 => out.note(format_args!("S15 STANDING CHECK: {} CHECKS IN {} MIN, THE LONGEST {} MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE",
                account.checks, minutes, account.longest_us / 1000)),
            d => out.note(format_args!("S15 STANDING CHECK: {} CHECKS IN {} MIN, THE LONGEST {} MS BETWEEN TWO WHILE LIT; DARK {} TIMES, {} S IN ALL; NOTHING NEW ON EITHER PIPE",
                account.checks, minutes, account.longest_us / 1000, d, account.dark_us / 1_000_000)),
        }
        (account.checks, account.longest_us, account.dark, account.dark_us, account.said_us) = (0, 0, 0, 0, now);
    }
    Ok(())
}

/// What the standing check found undone as stage 15's watch undoes (213-DRV-0103): stage 15's writes put back at once
/// (the pattern on, HUBP1 blanked as stage 13 left it), then screen_gone when stage 16 ran, then stages 14 to 1; the mark
/// kept, so nothing is tried again this boot. Whether the firmware's pipe reads as it did when step 3 began.
pub fn standing_undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, done: u8, why: Failure, since_offer_us: u64, out: &mut dyn Out) -> bool {
    out.part();
    let done = done.min(STAGES);
    if done >= 15 { undo(light, e, interp, journal, 15, true, out); }
    out.failed();
    out.note(format_args!("S15 STANDING CHECK: {} {} S AFTER THE OFFER; STAGE 15'S WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; UNDOING {} AND STAGES 14 TO 1",
        why, since_offer_us / 1_000_000, if done >= 16 { "16 (SCREEN_GONE)" } else { "NO OFFER" }));
    if done >= 16 { undo(light, e, interp, journal, 16, false, out); }
    for m in (1..=done.min(14)).rev() { undo(light, e, interp, journal, m, false, out); }
    let same = journal.baseline.and_then(|b| light.watch.read(e).ok().map(|now| light.watch.changed(&b, &now).is_none()));
    out.note(format_args!("STEP 3 UNDONE AFTER THE STANDING CHECK; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE {}; THE MARK KEPT: NOT TRIED AGAIN THIS BOOT",
        match same { Some(true) => "UNCHANGED", Some(false) => "CHANGED", None => "UNREADABLE" }));
    same == Some(true)
}
