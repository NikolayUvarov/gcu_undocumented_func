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
/// What step 3 runs on: reads, guarded writes, time.
pub trait Engine: Bus + Poke + Clock {}
impl<T: Bus + Poke + Clock> Engine for T {}

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
    /// All seven of step 1's batches must have been read; this is the first that was not.
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
    Plan(PlanError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refusal::NotRead { batch } => write!(f, "STEP 1'S BATCH {} WAS NOT READ (STEP 3 NEEDS ALL SEVEN)", batch),
            Refusal::RunningOtgs(n) => write!(f, "{} TIMING GENERATORS RUN, NOT ONE", n),
            Refusal::BackEnds(n) => write!(f, "{} DIG BACK ENDS ARE ENABLED, NOT ONE", n),
            Refusal::HdmiDriven => f.write_str("PHY A (HDMI-A-1) IS ALREADY DRIVEN"),
            Refusal::FirmwareFrontEnd => f.write_str("THE FIRMWARE'S DIG FRONT END CANNOT BE TOLD"),
            Refusal::NoMonitor => f.write_str("HPD 1 SENSES NO MONITOR ON HDMI-A-1"),
            Refusal::NoFreePipe => f.write_str("NO FREE TIMING GENERATOR AND OPP"),
            Refusal::NoFreeFrontEnd => f.write_str("NO FREE DIG FRONT END"),
            Refusal::Overlap => f.write_str("THE CHOSEN PIPE SHARES A BLOCK WITH THE FIRMWARE'S"),
            Refusal::AttachedHubp(n) => write!(f, "HUBP{} FOLLOWS STEP 3'S TIMING GENERATOR AND IS NOT BLANKED", n),
            Refusal::Plan(e) => write!(f, "THE PLAN: {:?}", e),
        }
    }
}

const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

/// The firmware's pipe and the pipe step 3 takes, from step 1's values; nothing assumed. All seven batches: step 3
/// reads and writes no register step 1 has not read on the same boot (batch 7 holds DCCG's pixel rate and PLL resync).
pub fn assess(s: &Store) -> Result<(Firmware, Choice), Refusal> {
    if s.batches() < crate::batches::BATCHES { return Err(Refusal::NotRead { batch: s.batches() + 1 }); }
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
/// PHY's back end, UNIPHY, the PHY's DPCS/RDPCS, and in DCCG the PHY's PLL resync, the OTG's pixel rate and SYMCLK.
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
pub struct Guard { allow: [(u32, u32); 16], allows: usize, deny: [(u32, u32); 32], denies: usize }

impl Guard {
    pub fn new(firmware: &Firmware, choice: &Choice) -> Result<Guard, Refusal> {
        let mut g = Guard { allow: [(0, 0); 16], allows: 0, deny: [(0, 0); 32], denies: 0 };
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
        let overlap = g.allow[..g.allows].iter().any(|&(a, b)| g.deny[..g.denies].iter().any(|&(c, d)| a < d && c < b));
        if overlap { Err(Refusal::Overlap) } else { Ok(g) }
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
                    return match r.block() { "" => write!(f, "{} AT {:05X}", r.name(), self.0.offset()), b => write!(f, "{}{}_{} AT {:05X}", b, i, r.name(), self.0.offset()) };
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
pub struct Light { pub firmware: Firmware, pub choice: Choice, pub timing: Timing, plan: Plan, starts: [usize; STAGES as usize + 1], guard: Guard, watch: Watch, attached: Watch }

impl Light {
    pub fn new(firmware: Firmware, choice: Choice, timing: Timing) -> Result<Light, Refusal> {
        let guard = Guard::new(&firmware, &choice)?;
        let plan = first_light(choice.pipe, choice.output, &timing, Signal::Dvi, Pattern::ColorSquares, firmware.protected()).map_err(Refusal::Plan)?;
        let mut starts = [plan.len(); STAGES as usize + 1];
        for (i, s) in plan.steps().enumerate() { if let Step::Begin(n) = s { if (1..=STAGES).contains(&n) { starts[n as usize - 1] = i + 1; } } }
        Ok(Light { firmware, choice, timing, plan, starts, guard, watch: Watch::of(&firmware), attached: Watch::attached(&choice) })
    }
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
        out.line(format_args!("STEP 3: FIRST LIGHT ON HDMI-A-1 IN {} STAGES; THIS BUILD MAY RUN {}", STAGES, allowed));
        out.line(format_args!("S0 THE FIRMWARE'S PIPE ({}): OTG{} OPP{} DIG {} FRONT END, PHY {}, HUBPS {}{}{}{}; NEVER WRITTEN, READ AFTER EACH STAGE ({} REGISTERS)",
            if expected { "AS THE SURVEY EXPECTS: STEP 1 NOT READ" } else { "AS STEP 1 FOUND" }, f.otg, f.opp, LETTERS[f.dig_fe as usize], LETTERS[f.phy as usize],
            f.hubps[0] as u8, f.hubps[1] as u8, f.hubps[2] as u8, f.hubps[3] as u8, self.watch.len()));
        out.line(format_args!("S0 STEP 3'S PIPE: OTG{} OPP{} DIG {} FRONT END, PHY {} (HDMI-A-1), HPD {}; MODE {}x{} AT {} MHZ IN DVI SIGNALLING (CTA VIC 16)",
            c.pipe.otg, c.pipe.opp, LETTERS[c.pipe.dig_fe as usize], LETTERS[c.output.phy as usize], c.output.hpd + 1,
            self.timing.h_active, self.timing.v_active, self.timing.pixel_khz as f32 / 1000.0));
        for n in (0..4u8).filter(|&n| c.attached[n as usize]) {
            out.line(format_args!("S0 HUBP{} FOLLOWS OTG{}, POWERED AND BLANKED: NEVER WRITTEN, ITS DCHUBP_CNTL READ AFTER STAGES {} TO {}", n, c.pipe.otg, ATTACHED_FROM, STAGES));
        }
    }

    fn plan_lines(&self, n: u8, verb: &str, out: &mut dyn Out) {
        for step in self.stage(n) {
            match step {
                Step::Update { reg, mask, value } => out.line(format_args!("S{} {} WRITE {}: BITS {:08X} = {:08X}", n, verb, Name(reg), mask, value)),
                Step::Wait { reg, mask, value, timeout_us } => out.line(format_args!("S{} {} WAIT FOR {}: BITS {:08X} = {:08X}, AT MOST {} US", n, verb, Name(reg), mask, value, timeout_us)),
                Step::Call(t) => {
                    let (p, len) = t.parameters();
                    out.line(format_args!("S{} {} RUN TABLE {} ({}) WITH {} BYTES: {}", n, verb, t.index(), table_name(t.index()), len, Bytes(&p[..len])));
                }
                Step::Frames { otg, frames, timeout_us } => out.line(format_args!("S{} {} WATCH OTG{}'S FRAME COUNT MOVE {} TIMES WITHIN {} MS", n, verb, otg, frames, timeout_us / 1000)),
                Step::Begin(_) => {}
            }
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
        light.plan_lines(n, "WOULD", out);
    }
    out.line(format_args!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", STAGES));
}

// --- running ---

/// Why a stage failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreadable(u32),
    Refused(Refused),
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
    tag: (char, u8, u8), record: bool, writes: u32, refused: Option<Refused>, full: bool, log_us: u64,
}

impl<E: Engine> atom::Machine for Adapter<'_, E> {
    fn read(&mut self, reg: Reg) -> Option<u32> { self.engine.read(reg) }
    fn write(&mut self, reg: Reg, value: u32) -> bool {
        let w = match self.guard.check(reg) { Ok(w) => w, Err(r) => { self.refused = Some(r); return false; } };
        let Some(old) = self.engine.read(reg) else { return false };
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
    let mut a = Adapter { engine: e, guard: &light.guard, journal, out, tag: (prefix, n, t.index()), record, writes: 0, refused: None, full: false, log_us: 0 };
    interp.set_time_limit(TABLE_TIME_LIMIT_US);
    let result = interp.run(t.index(), &p[..len], &mut a);
    let (writes, refused, full) = (a.writes, a.refused, a.full);
    match result {
        Ok(o) => { out.line(format_args!("{}{} TABLE {} DONE: {} STEPS, {} CALLS, {} US OF DELAYS, {} WRITES", prefix, n, t.index(), o.steps, o.calls, o.delay_us, writes)); Ok(()) }
        Err(_) if refused.is_some() => Err(Failure::Refused(refused.unwrap_or(Refused::OffRoute(0)))),
        Err(_) if full => Err(Failure::JournalFull),
        Err(atom::Error::Unreadable { offset, .. }) => Err(Failure::Unreadable(offset)),
        Err(err) => Err(Failure::Table(err)),
    }
}

fn read(e: &mut impl Bus, reg: Reg) -> Result<u32, Failure> { e.read(reg).ok_or(Failure::Unreadable(reg.offset())) }

fn stage<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, out: &mut dyn Out) -> Result<(), Failure> {
    for step in light.stage(n) {
        match step {
            Step::Update { reg, mask, value } => {
                let w = light.guard.check(reg).map_err(Failure::Refused)?;
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
            Step::Call(t) => call(light, e, interp, journal, t, 'S', n, true, out)?,
            Step::Begin(_) => {}
        }
    }
    Ok(())
}

/// Undoes stage `n`: its off call, if it has one, then every register it wrote put back, last first, so the second
/// pipe's and PHY A's registers are as before step 3 (PHY A was off). What a table wrote through the PHY's own control
/// register bus (`RDPCS_TX_CR_ADDR`/`_DATA`) is not reached by a restore: the undo says how many such writes there were.
fn undo<E: Engine>(light: &Light, e: &mut E, interp: &mut Interpreter, journal: &mut Journal, n: u8, out: &mut dyn Out) {
    if let Some(t) = light.off_call(n) {
        let mut scratch = Journal::default();
        if let Err(why) = call(light, e, interp, &mut scratch, t, 'U', n, false, out) {
            out.line(format_args!("U{} TABLE {} STOPPED ({}); RESTORING STAGE {}'S WRITES ANYWAY", n, t.index(), why, n));
        }
    }
    let cr_bus = (seg2(0x2934 + 0xD8 * light.choice.output.phy as u32), seg2(0x2936 + 0xD8 * light.choice.output.phy as u32));
    let (mut restored, mut through_bus) = (0, 0);
    let mut i = journal.len;
    while i > 0 {
        i -= 1;
        let Some((w, old, stage)) = journal.items[i] else { continue };
        if stage != n { continue; }
        if (cr_bus.0..cr_bus.1).contains(&w.reg().offset()) { through_bus += 1; }
        e.write(w, old);
        restored += 1;
        out.line(format_args!("U{} RESTORED {} = {:08X}", n, Name(w.reg()), old));
    }
    if through_bus > 0 {
        out.line(format_args!("U{} {} WRITES WENT THROUGH PHY {}'S CONTROL REGISTER BUS: WHAT THEY SET INSIDE THE PHY IS NOT RESTORED", n, through_bus, LETTERS[light.choice.output.phy as usize]));
    }
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
    let attached = match light.attached.read(e) {
        Ok(v) => v,
        Err(why) => { out.line(format_args!("STEP 3 NOT RUN: THE ATTACHED HUBPS: {}", why)); return Outcome::Failed { stage: 0, why }; }
    };
    for n in 1..=STAGES {
        let name = STAGE_NAMES[n as usize - 1];
        if n > allowed {
            out.line(format_args!("S{} {}: NOT RUN (THIS BUILD MAY RUN {}); WOULD:", n, name, allowed));
            light.plan_lines(n, "WOULD", out);
            continue;
        }
        out.line(format_args!("S{} {}: STAGE {} OF {} NEXT; ITS WRITES:", n, name, n, STAGES));
        light.plan_lines(n, "WILL", out);
        out.pause(PAUSE_BEFORE_STAGE_MS);
        let result = stage(light, e, interp, journal, n, out).and_then(|()| {
            let now = light.watch.read(e)?;
            if let Some((reg, before, after)) = light.watch.changed(&baseline, &now) { return Err(Failure::FirmwareChanged { offset: reg.offset(), before, after }); }
            if n < ATTACHED_FROM || light.attached.is_empty() { return Ok(()); }
            let now = light.attached.read(e)?;
            match light.attached.changed(&attached, &now) { Some((reg, before, after)) => Err(Failure::AttachedChanged { offset: reg.offset(), before, after }), None => Ok(()) }
        });
        match result {
            Ok(()) => {
                out.line(format_args!("S{} {} DONE; THE FIRMWARE'S PIPE UNCHANGED ({} REGISTERS)", n, name, light.watch.len()));
                if n >= ATTACHED_FROM && !light.attached.is_empty() { out.line(format_args!("S{} THE ATTACHED HUBPS UNCHANGED, BLANKED ({} REGISTERS)", n, light.attached.len())); }
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
