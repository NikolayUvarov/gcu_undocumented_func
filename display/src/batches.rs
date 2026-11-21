//! Step 1 of 213-DRV-0086: the firmware's display state read in seven batches, the registers Display Core itself reads
//! (hwdocs/cezanne-display.md, "a read proposed as safe"). Each batch is named in the log before its first read and
//! paused after, each value logged as it is read, so that a hang names its register; batch 4 reads only the HUBPs batch
//! 3 shows powered. `Bus` has no write: nothing here can change the device. `tests/display_batches_host.rs` checks it.

use crate::dcn::{field::*, Field, Reg, R};
use crate::timing::Otg;
use core::fmt;

pub const BATCHES: u8 = 7;
/// The most registers one batch reads (batch 2: 19 for each of the four timing generators).
pub const MAX_REGISTERS: usize = 80;
/// After a batch's name is logged and before its first read: long enough for the line to be kept.
pub const PAUSE_BEFORE_READ_MS: u32 = 1000;
/// After a batch's values, before the next batch's name.
pub const PAUSE_BETWEEN_BATCHES_MS: u32 = 3000;

/// The device's registers, read only.
pub trait Bus {
    /// The register's value; None when the grant does not map it.
    fn read(&mut self, reg: Reg) -> Option<u32>;
}

/// Where the lines go, and the pauses.
pub trait Out {
    fn line(&mut self, args: fmt::Arguments);
    fn pause(&mut self, ms: u32);
}

/// One register of a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named { pub r: R, pub inst: u8, pub reg: Reg }

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.r.block() {
            "" => write!(f, "{} AT {:05X}", self.r.name(), self.reg.offset()),
            block => write!(f, "{}{}_{} AT {:05X}", block, self.inst, self.r.name(), self.reg.offset()),
        }
    }
}

/// A batch's registers, in reading order.
#[derive(Clone, Copy, Debug)]
pub struct List { items: [Option<Named>; MAX_REGISTERS], len: usize }

impl List {
    const fn new() -> List { List { items: [None; MAX_REGISTERS], len: 0 } }
    fn push(&mut self, r: R, inst: u8) {
        if let (Some(reg), true) = (r.at(inst), self.len < MAX_REGISTERS) {
            self.items[self.len] = Some(Named { r, inst, reg });
            self.len += 1;
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = Named> + '_ { self.items[..self.len].iter().flatten().copied() }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

/// What earlier batches showed that later ones depend on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Known {
    /// Batch 3: whether HUBP n's power domain (2n) is on; None until batch 3 has read all four.
    pub hubp_powered: Option<[bool; 4]>,
}

/// What `optc1_read_otg_state` reads, and the clock and source of each timing generator.
const OTG_STATE: [R; 19] = {
    use R::*;
    [OtgControl, OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax,
     OtgVTotalControl, OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgVerticalInterrupt1Control,
     OtgVerticalInterrupt1Position, OtgVerticalInterrupt2Control, OtgVerticalInterrupt2Position, OptcInputGlobalControl,
     OtgClockControl, OptcDataSourceSelect]
};
const HUBP_STATE: [R; 5] = {
    use R::*;
    [HubpreqPrimarySurfaceAddress, HubpreqPrimarySurfaceAddressHigh, HubpSurfaceConfig, HubpreqSurfacePitch, HubpCntl]
};
const APERTURE: [R; 6] = {
    use R::*;
    [DcnVmFbLocationBase, DcnVmFbLocationTop, DcnVmFbOffset, DcnVmAgpBot, DcnVmAgpTop, DcnVmAgpBase]
};
/// The DIG back ends of the board's outputs: PHYs A, B, D and E (the VBIOS's paths; C has no connector).
pub const BOARD_PHYS: [u8; 4] = [0, 1, 3, 4];

pub fn title(batch: u8) -> &'static str {
    match batch {
        1 => "HPD", 2 => "OTG STATE", 3 => "POWER", 4 => "HUBP", 5 => "DIG", 6 => "APERTURE", 7 => "CLOCKS", _ => "?",
    }
}

/// The registers batch `batch` reads, given what the earlier ones showed.
pub fn plan(batch: u8, known: &Known) -> List {
    let mut list = List::new();
    match batch {
        1 => for n in 0..5 { list.push(R::HpdIntStatus, n); list.push(R::HpdControl, n); },
        2 => for n in 0..4 { for r in OTG_STATE { list.push(r, n); } },
        3 => for n in 0..8 { list.push(R::DomainPgStatus, n); },
        4 => if let Some(powered) = known.hubp_powered {
            for n in 0..4 { if powered[n as usize] { for r in HUBP_STATE { list.push(r, n); } } }
        },
        5 => {
            for n in 0..5 { list.push(R::DigFeCntl, n); }
            for n in BOARD_PHYS { list.push(R::DigBeCntl, n); list.push(R::DigBeEnCntl, n); }
        }
        6 => for r in APERTURE { list.push(r, 0); },
        7 => {
            for n in 0..4 { list.push(R::OtgPixelRateCntl, n); list.push(R::OtgPhypllPixelRateCntl, n); }
            for n in 0..5 { list.push(R::PhypllPixclkResyncCntl, n); }
            list.push(R::DentistDispclkCntl, 0);
        }
        _ => {}
    }
    list
}

/// Batch 4's registers for every HUBP, for naming before batch 3 has been read.
fn every_hubp() -> List {
    let mut list = List::new();
    for n in 0..4 { for r in HUBP_STATE { list.push(r, n); } }
    list
}

/// A batch's values as read.
struct Values { items: [Option<(Named, Option<u32>)>; MAX_REGISTERS], len: usize }

impl Values {
    fn new() -> Values { Values { items: [None; MAX_REGISTERS], len: 0 } }
    fn push(&mut self, n: Named, v: Option<u32>) { if self.len < MAX_REGISTERS { self.items[self.len] = Some((n, v)); self.len += 1; } }
    fn get(&self, r: R, inst: u8) -> Option<u32> {
        self.items[..self.len].iter().flatten().find(|(n, _)| n.r == r && n.inst == inst).and_then(|(_, v)| *v)
    }
    fn field(&self, r: R, inst: u8, f: Field) -> Option<u32> { self.get(r, inst).map(|v| f.get(v)) }
}

/// A value or a dash for one that was not read.
struct Opt(Option<u32>);
impl fmt::Display for Opt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(v) => write!(f, "{}", v), None => f.write_str("-") } }
}

/// What a run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report { pub batches_read: u8, pub registers_read: usize }

/// Every value a run read, for step 3 to choose its pipe from (all seven batches: 147 registers).
pub const STORE: usize = 160;
#[derive(Clone, Copy, Debug)]
pub struct Store { items: [Option<(R, u8, u32)>; STORE], len: usize, batches: u8 }

impl Default for Store { fn default() -> Store { Store { items: [None; STORE], len: 0, batches: 0 } } }

impl Store {
    fn push(&mut self, r: R, inst: u8, v: u32) { if self.len < STORE { self.items[self.len] = Some((r, inst, v)); self.len += 1; } }
    pub fn get(&self, r: R, inst: u8) -> Option<u32> {
        self.items[..self.len].iter().flatten().find(|(x, i, _)| *x == r && *i == inst).map(|(_, _, v)| *v)
    }
    pub fn field(&self, r: R, inst: u8, f: Field) -> Option<u32> { self.get(r, inst).map(|v| f.get(v)) }
    /// How many batches were read in full.
    pub fn batches(&self) -> u8 { self.batches }
    /// For host cases: a value as if step 1 had read it.
    pub fn set(&mut self, r: R, inst: u8, v: u32) {
        match self.items[..self.len].iter_mut().flatten().find(|(x, i, _)| *x == r && *i == inst) { Some(e) => e.2 = v, None => self.push(r, inst, v) }
    }
    pub fn set_batches(&mut self, n: u8) { self.batches = n; }
}

/// Step 1: batches 1 to `allowed` read, the others only named with the registers they would read.
pub fn run(bus: &mut impl Bus, allowed: u8, out: &mut impl Out) -> Report { run_into(bus, allowed, out, &mut Store::default()) }

/// Step 1, every value read kept in `store`.
pub fn run_into(bus: &mut impl Bus, allowed: u8, out: &mut impl Out, store: &mut Store) -> Report {
    let allowed = allowed.min(BATCHES);
    out.line(format_args!("STEP 1: THE ENGINE'S STATE IN {} BATCHES, READ ONLY; THIS BUILD MAY READ {}", BATCHES, allowed));
    let mut known = Known::default();
    let mut report = Report::default();
    for batch in 1..=BATCHES {
        let list = plan(batch, &known);
        if batch > allowed {
            name_only(batch, &list, &known, allowed, out);
            continue;
        }
        if list.is_empty() {
            out.line(format_args!("B{} {}: NOTHING TO READ: BATCH 3 SHOWED NO HUBP POWERED, OR WAS NOT READ IN FULL", batch, title(batch)));
            continue;
        }
        out.line(format_args!("B{} {}: READING {} REGISTERS NEXT", batch, title(batch), list.len()));
        out.pause(PAUSE_BEFORE_READ_MS);
        let mut values = Values::new();
        for n in list.iter() {
            let v = bus.read(n.reg);
            match v {
                Some(v) => { out.line(format_args!("B{} {} = {:08X}", batch, n, v)); report.registers_read += 1; store.push(n.r, n.inst, v); }
                None => out.line(format_args!("B{} {}: NOT MAPPED, NOT READ", batch, n)),
            }
            values.push(n, v);
        }
        summarize(batch, &values, &mut known, out);
        report.batches_read = batch;
        store.batches = batch;
        out.pause(PAUSE_BETWEEN_BATCHES_MS);
    }
    out.line(format_args!("STEP 1 DONE: {} OF {} BATCHES READ, {} REGISTERS, NOTHING WRITTEN", report.batches_read, BATCHES, report.registers_read));
    report
}

fn name_only(batch: u8, list: &List, known: &Known, allowed: u8, out: &mut impl Out) {
    if batch == 4 && known.hubp_powered.is_none() {
        let all = every_hubp();
        out.line(format_args!("B4 HUBP: NOT READ (THIS BUILD MAY READ {}); WOULD READ, ONLY FOR EACH HUBP n THAT B3 SHOWS POWERED (DOMAIN 2n):", allowed));
        for n in all.iter() { out.line(format_args!("B4 WOULD READ {}", n)); }
        return;
    }
    out.line(format_args!("B{} {}: NOT READ (THIS BUILD MAY READ {}); WOULD READ {} REGISTERS:", batch, title(batch), allowed, list.len()));
    for n in list.iter() { out.line(format_args!("B{} WOULD READ {}", batch, n)); }
}

const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

fn summarize(batch: u8, v: &Values, known: &mut Known, out: &mut impl Out) {
    use R::*;
    match batch {
        1 => {
            let s = |n| Opt(v.field(HpdIntStatus, n, DC_HPD_SENSE));
            let e = |n| Opt(v.field(HpdControl, n, DC_HPD_EN));
            out.line(format_args!("B1 HPD SENSE 1:{} 2:{} 3:{} 4:{} 5:{}; ENABLED 1:{} 2:{} 3:{} 4:{} 5:{}",
                s(0), s(1), s(2), s(3), s(4), e(0), e(1), e(2), e(3), e(4)));
        }
        2 => for n in 0..4 { summarize_otg(v, n, out); },
        3 => {
            let on = |d: u8| v.field(DomainPgStatus, d, DOMAIN_PGFSM_PWR_STATUS).map(|s| s == PGFSM_POWER_ON);
            let word = |d: u8| match on(d) { Some(true) => "ON", Some(false) => "OFF", None => "-" };
            for n in 0..4u8 {
                out.line(format_args!("B3 PIPE {}: HUBP{} {}, DPP{} {}", n, n, word(2 * n), n, word(2 * n + 1)));
            }
            if let (Some(a), Some(b), Some(c), Some(d)) = (on(0), on(2), on(4), on(6)) { known.hubp_powered = Some([a, b, c, d]); }
        }
        4 => for n in 0..4 {
            let Some(low) = v.get(HubpreqPrimarySurfaceAddress, n) else { continue };
            out.line(format_args!("B4 HUBP{}: SURFACE {:04X}_{:08X}, FORMAT {}, PITCH {}, ROTATION {}, BLANK {}, TTU_DISABLE {}",
                n, v.field(HubpreqPrimarySurfaceAddressHigh, n, PRIMARY_SURFACE_ADDRESS_HIGH).unwrap_or(0), low,
                Opt(v.field(HubpSurfaceConfig, n, SURFACE_PIXEL_FORMAT)), Opt(v.field(HubpreqSurfacePitch, n, PITCH).map(|p| p + 1)),
                Opt(v.field(HubpSurfaceConfig, n, ROTATION_ANGLE)), Opt(v.field(HubpCntl, n, HUBP_BLANK_EN)),
                Opt(v.field(HubpCntl, n, HUBP_TTU_DISABLE))));
        },
        5 => {
            for n in 0..5u8 {
                out.line(format_args!("B5 DIG{} FE: FROM OTG {}, SYMCLK_FE_ON {}", LETTERS[n as usize],
                    Opt(v.field(DigFeCntl, n, DIG_SOURCE_SELECT)), Opt(v.field(DigFeCntl, n, DIG_SYMCLK_FE_ON))));
            }
            for n in BOARD_PHYS {
                out.line(format_args!("B5 DIG{} BE: ENABLED {}, MODE {}, FROM FE MASK {}, SYMCLK_BE_ON {}", LETTERS[n as usize],
                    Opt(v.field(DigBeEnCntl, n, DIG_ENABLE)), Opt(v.field(DigBeCntl, n, DIG_MODE)),
                    Opt(v.field(DigBeCntl, n, DIG_FE_SOURCE_SELECT)), Opt(v.field(DigBeEnCntl, n, DIG_SYMCLK_BE_ON))));
            }
        }
        6 => {
            let at = |r: R| v.field(r, 0, VM_16MIB).map(|x| (x as u64) << 24);
            let hex = |a: Option<u64>| Hex(a);
            out.line(format_args!("B6 APERTURE: FB {}..{} (TOP'S LAST 16 MIB), FB OFFSET {}, AGP {}..{} BASE {}",
                hex(at(DcnVmFbLocationBase)), hex(at(DcnVmFbLocationTop)), hex(at(DcnVmFbOffset)),
                hex(at(DcnVmAgpBot)), hex(at(DcnVmAgpTop)), hex(at(DcnVmAgpBase))));
        }
        7 => {
            for n in 0..4u8 {
                out.line(format_args!("B7 OTG{} PIXEL RATE: SOURCE {}, DP_DTO {}, PHYPLL SOURCE {}, PLL SOURCE {}", n,
                    Opt(v.field(OtgPixelRateCntl, n, OTG_PIXEL_RATE_SOURCE)), Opt(v.field(OtgPixelRateCntl, n, DP_DTO_ENABLE)),
                    Opt(v.field(OtgPhypllPixelRateCntl, n, OTG_PHYPLL_PIXEL_RATE_SOURCE)),
                    Opt(v.field(OtgPhypllPixelRateCntl, n, OTG_PIXEL_RATE_PLL_SOURCE))));
            }
            for n in 0..5u8 {
                out.line(format_args!("B7 PHYPLL{}: PIXCLK_ENABLE {}, DEEP COLOR {}", LETTERS[n as usize],
                    Opt(v.field(PhypllPixclkResyncCntl, n, PHYPLL_PIXCLK_ENABLE)),
                    Opt(v.field(PhypllPixclkResyncCntl, n, PHYPLL_DEEP_COLOR_CNTL))));
            }
            out.line(format_args!("B7 DENTIST: DISPCLK WDIVIDER {}, RDIVIDER {}",
                Opt(v.field(DentistDispclkCntl, 0, DENTIST_DISPCLK_WDIVIDER)), Opt(v.field(DentistDispclkCntl, 0, DENTIST_DISPCLK_RDIVIDER))));
        }
        _ => {}
    }
}

/// A GPU address in hex, or a dash.
struct Hex(Option<u64>);
impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(a) => write!(f, "{:X}", a), None => f.write_str("-") } }
}

/// The timing generator's state as its registers give it, in the convention of `Timing::otg`.
pub fn otg_state(v: &[(R, u32)]) -> Option<Otg> {
    let get = |r: R| v.iter().find(|(x, _)| *x == r).map(|(_, value)| *value);
    let (low, high) = (COUNTER_LOW, COUNTER_HIGH);
    use R::*;
    Some(Otg {
        h_total: low.get(get(OtgHTotal)?),
        h_blank_start: low.get(get(OtgHBlankStartEnd)?),
        h_blank_end: high.get(get(OtgHBlankStartEnd)?),
        h_sync_end: high.get(get(OtgHSyncA)?),
        v_total: low.get(get(OtgVTotal)?),
        v_blank_start: low.get(get(OtgVBlankStartEnd)?),
        v_blank_end: high.get(get(OtgVBlankStartEnd)?),
        v_sync_end: high.get(get(OtgVSyncA)?),
        h_sync_negative: SYNC_POL.get(get(OtgHSyncACntl)?) == 1,
        v_sync_negative: SYNC_POL.get(get(OtgVSyncACntl)?) == 1,
        vcount_init: low.get(get(OtgVBlankStartEnd)?),
    })
}

fn summarize_otg(v: &Values, n: u8, out: &mut impl Out) {
    use R::*;
    let mut regs = [(OtgHTotal, 0u32); 8];
    let wanted = [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgVTotal, OtgVBlankStartEnd, OtgVSyncA, OtgHSyncACntl, OtgVSyncACntl];
    let mut complete = true;
    for (slot, r) in regs.iter_mut().zip(wanted) {
        match v.get(r, n) { Some(x) => *slot = (r, x), None => complete = false }
    }
    let state = Opt(v.field(OtgControl, n, OTG_CURRENT_MASTER_EN_STATE));
    let clock = Opt(v.field(OtgClockControl, n, OTG_CLOCK_ON));
    let opp = Opt(v.field(OptcDataSourceSelect, n, OPTC_SEG0_SRC_SEL));
    let sync_start = |r: R| v.field(r, n, COUNTER_LOW).unwrap_or(0);
    let timing = if complete { otg_state(&regs).and_then(|o| o.timing(0)) } else { None };
    match timing {
        Some(t) => out.line(format_args!("B2 OTG{}: RUNNING {}, CLOCK_ON {}, FROM OPP {}; {}x{} TOTAL {}x{} H{} V{}{}", n, state, clock, opp,
            t.h_active, t.v_active, t.h_total(), t.v_total(), if t.h_sync_positive { '+' } else { '-' },
            if t.v_sync_positive { '+' } else { '-' },
            if sync_start(OtgHSyncA) != 0 || sync_start(OtgVSyncA) != 0 { ", SYNC NOT FROM 0" } else { "" })),
        None => out.line(format_args!("B2 OTG{}: RUNNING {}, CLOCK_ON {}, FROM OPP {}; NO TIMING IN ITS REGISTERS", n, state, clock, opp)),
    }
}
