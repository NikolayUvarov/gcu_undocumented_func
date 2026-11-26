// Host cases of display's step 3 (213-DRV-0086): first light's eight stages on a model of DCN 2.1's blocks: the pipe
// chosen from step 1's values, the guard's refusals, the order of the writes against the route, each stage's undo, a
// status that never comes, the firmware's pipe changed under a stage, a build that may run nothing, and the mode's
// timing against the CTA table. The tables are synthetic VBIOS tables here; the Ryzen's own run when MIND_AMD_VBIOS
// names its image (the vendor's, not in the repository).
#![allow(dead_code)] // the driver uses parts these cases do not
extern crate alloc;
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;
#[path = "../display/src/hpd.rs"] mod hpd;
#[path = "../display/src/ddc.rs"] mod ddc;
#[path = "../display/src/setting.rs"] mod setting;
#[path = "../display/src/start.rs"] mod start;

use batches::{Bus, Out, Store};
use dcn::{Reg, Step, R};
// Step 3's stages (first light); step 4's come after them (213-DRV-0092).
const STAGES: u8 = dcn::FIRST_LIGHT;
use light::{assess, Canvas, Choice, Clock, Failure, Light, Outcome, Poke, Refusal, Refused, ScreenRefused, Screens, Writable, EXPECTED, HDMI_A_1};
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

fn seg(s: usize, dword: u32) -> u32 { (dcn::SEGMENT[s] + dword) * 4 }
fn reg(r: R, i: u8) -> u32 { r.at(i).unwrap().offset() }

// --- step 1's values ---

// What step 1 would find if the GOP drives DP-2 as the survey expects: OTG 0 from OPP 0, HUBP 0 on it, DIG front end E
// into PHY E's enabled back end, HPD 1 sensing HDMI-A-1's monitor, every pipe's domains on, and HUBPs 1 to 3 blanked on
// their own idle OTGs (DCHUBP_CNTL as the Ryzen's: 000F0002, 000F001B, 000F002B, 000F003B).
fn found() -> Store {
    let mut s = Store::default();
    for n in 0..5u8 { s.set(R::HpdIntStatus, n, if n == 0 || n == 4 { 0b10 } else { 0 }); s.set(R::HpdControl, n, 1 << 28); }
    for t in 0..4u8 {
        s.set(R::OtgControl, t, if t == 0 { 0x0001_0001 } else { 0 });
        s.set(R::OptcDataSourceSelect, t, 0);
        s.set(R::OtgClockControl, t, if t == 0 { 0x101 } else { 0 });
    }
    for d in 0..8u8 { s.set(R::DomainPgStatus, d, 0); }
    for n in 0..4u8 { s.set(R::HubpCntl, n, if n == 0 { 0x000F_0002 } else { 0x000F_000B | (n as u32) << 4 }); }
    for d in 0..5u8 { s.set(R::DigFeCntl, d, if d == 4 { 1 << 24 } else { 7 }); }
    for p in batches::BOARD_PHYS {
        s.set(R::DigBeCntl, p, if p == 4 { (1 << 4) << 8 } else { 0 });
        s.set(R::DigBeEnCntl, p, if p == 4 { 0x101 } else { 0 });
    }
    s.set(R::DmcubCntl, 0, 1 << 17); // the DMCUB held in reset
    s.set_batches(10);
    s
}

#[test]
fn the_pipe_is_chosen_from_what_step_1_found() {
    assert_eq!(assess(&found()), Ok(EXPECTED));
    // The GOP on OTG 1 from OPP 1: step 3 takes OTG 0 and OPP 0.
    let mut s = found();
    s.set(R::OtgControl, 0, 0); s.set(R::OtgControl, 1, 0x0001_0001); s.set(R::OptcDataSourceSelect, 1, 1 << 8);
    s.set(R::HubpCntl, 0, 0xF << 4); s.set(R::HubpCntl, 1, 1 << 4);
    let (f, c) = assess(&s).unwrap();
    assert_eq!((f.otg, f.opp, f.hubps), (1, 1, [false, true, false, false]));
    assert_eq!((c.pipe.otg, c.pipe.opp, c.pipe.dig_fe, c.output), (0, 0, 0, HDMI_A_1));
    // Front end A busy: step 3 takes B.
    let mut s = found();
    s.set(R::DigFeCntl, 0, 1 << 24);
    assert_eq!(assess(&s).unwrap().1.pipe.dig_fe, 1);
    // The firmware's front end from its source when the back end names none.
    let mut s = found();
    s.set(R::DigBeCntl, 4, 0); s.set(R::DigFeCntl, 2, 1 << 24); s.set(R::DigFeCntl, 4, 7);
    assert_eq!(assess(&s).unwrap().0.dig_fe, 2);
}

#[test]
fn step_3_refuses_what_it_cannot_tell() {
    let refuse = |change: &dyn Fn(&mut Store)| { let mut s = found(); change(&mut s); assess(&s).unwrap_err() };
    assert_eq!(refuse(&|s| s.set_batches(9)), Refusal::NotRead { batch: 10 });
    assert_eq!(refuse(&|s| s.set_batches(7)), Refusal::NotRead { batch: 8 });
    assert_eq!(refuse(&|s| s.set_batches(6)), Refusal::NotRead { batch: 7 });
    assert_eq!(refuse(&|s| s.set_batches(4)), Refusal::NotRead { batch: 5 });
    assert_eq!(refuse(&|s| s.set_batches(0)), Refusal::NotRead { batch: 1 });
    assert_eq!(refuse(&|s| s.set(R::OtgControl, 2, 0x0001_0001)), Refusal::RunningOtgs(2));
    assert_eq!(refuse(&|s| s.set(R::OtgControl, 0, 0)), Refusal::RunningOtgs(0));
    assert_eq!(refuse(&|s| s.set(R::DigBeEnCntl, 3, 1)), Refusal::BackEnds(2));
    assert_eq!(refuse(&|s| s.set(R::DigBeEnCntl, 4, 0)), Refusal::BackEnds(0));
    assert_eq!(refuse(&|s| { s.set(R::DigBeEnCntl, 4, 0); s.set(R::DigBeEnCntl, 0, 1); }), Refusal::HdmiDriven(HDMI_A_1));
    assert_eq!(refuse(&|s| s.set(R::HpdIntStatus, 0, 0)), Refusal::NoMonitor(HDMI_A_1));
    assert_eq!(refuse(&|s| { s.set(R::DigBeCntl, 4, 0); s.set(R::DigFeCntl, 4, 0); }), Refusal::FirmwareFrontEnd);
    assert_eq!(refuse(&|s| for d in 0..4 { s.set(R::DigFeCntl, d, 1 << 24); }), Refusal::NoFreeFrontEnd);
    // The DMCUB enabled and out of reset: it may own the PHY and the pixel clocks; enabled but held in reset is no bar.
    assert_eq!(refuse(&|s| s.set(R::DmcubCntl, 0, 1 << 16)), Refusal::DmcubRuns);
    let mut held = found(); held.set(R::DmcubCntl, 0, 1 << 16 | 1 << 17);
    assert_eq!(assess(&held), Ok(EXPECTED));
    // HUBP 1 on OTG 1, step 3's, not blanked: it would fetch once OTG 1 runs.
    assert_eq!(refuse(&|s| s.set(R::HubpCntl, 1, 1 << 4)), Refusal::AttachedHubp(1));
}

// --- the guard ---

#[test]
fn the_guard_refuses_every_block_of_the_firmwares_pipe_and_all_off_the_route() {
    let (f, c) = EXPECTED;
    let g = light::Guard::new(&f, &c).unwrap();
    let refused = |o: u32| g.check(Reg::new(o).unwrap()).err();
    // The firmware's: OTG 0, ODM 0, VTG 0, OPP 0 and its DPG, DIG E (front and back end), UNIPHYE, PHY E's DPCS, PHYPLLE,
    // OTG 0's pixel rate, SYMCLKE, and HUBP 0, DPP 0, MPCC 0, which follow OTG 0.
    let firmware = [reg(R::OtgHTotal, 0), reg(R::OtgControl, 0), reg(R::OtgVreadyParam, 0), reg(R::OptcDataSourceSelect, 0),
        reg(R::VtgControl, 0), reg(R::OppPipeControl, 0), reg(R::DpgControl, 0), reg(R::DigFeCntl, 4), reg(R::DigBeCntl, 4),
        reg(R::DigBeEnCntl, 4), seg(2, 0x2875), seg(2, 0x2928 + 4 * 0xD8 + 0x18), reg(R::PhypllPixclkResyncCntl, 4),
        reg(R::OtgPixelRateCntl, 0), reg(R::OtgPhypllPixelRateCntl, 0), seg(1, 0xA4), reg(R::HubpreqPrimarySurfaceAddress, 0),
        reg(R::HubpCntl, 0), seg(2, 0xCC5), seg(2, 0x1271)];
    for o in firmware { assert_eq!(refused(o), Some(Refused::Firmware(o)), "{:05X}", o); }
    // Off the route: OTG 2, another HUBP, HPD, DSC, DCHUBBUB's aperture, DISPCLK, PHY B's DPCS, the SMU mailbox.
    for o in [reg(R::OtgHTotal, 2), reg(R::HubpCntl, 1), reg(R::HpdControl, 0), seg(2, 0x3000), reg(R::DcnVmFbLocationBase, 0),
              reg(R::DentistDispclkCntl, 0), seg(2, 0x2928 + 0xD8), dcn::SMU_MAILBOX[0]] {
        assert_eq!(refused(o), Some(Refused::OffRoute(o)), "{:05X}", o);
    }
    // Every write of the plan, and the writes tables 12, 4 and 76 made on the model (hwdocs), are allowed.
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for n in 1..=STAGES {
        for s in l.stage(n) { if let Step::Update { reg, .. } = s { assert!(g.check(reg).is_ok(), "{:05X}", reg.offset()); } }
    }
    for o in [seg(2, 0x294F), seg(2, 0x2934), seg(2, 0x2928), seg(1, 0xA0), seg(1, 0x40), seg(1, 0x84), seg(1, 0x87), seg(2, 0x20DA),
              seg(2, 0x206B), seg(2, 0x20B0), seg(2, 0x20E1), seg(2, 0x286D), seg(2, 0x286E), seg(2, 0x2929)] {
        assert!(g.check(Reg::new(o).unwrap()).is_ok(), "{:05X}", o);
    }
    // A choice that shares the firmware's DIG block is refused before anything runs.
    let shared = Choice { pipe: dcn::Pipe { otg: 1, opp: 1, dig_fe: 4 }, output: HDMI_A_1, attached: [false; 4] };
    assert_eq!(light::Guard::new(&f, &shared).err(), Some(Refusal::Overlap));
}

// --- the model ---

#[derive(Clone, Debug, PartialEq)]
enum Ev { Line(String), Write(u32, u32), Read(u32), Offer(u16, u16, u32), Gone }

#[derive(Default)]
struct Engine {
    regs: HashMap<u32, u32>, clock: u64, ev: Vec<Ev>, frames: u32, frames_move: bool,
    // Every read made, mapped or not (213-DRV-0099: the setting `off` makes none).
    reads: u64,
    // After a write to the first offset, the model sets the second to the third: the firmware's pipe changed by the hardware.
    hook: Option<(u32, u32, u32)>,
    // After a write to the first offset that clears the bits of the second, the model sets the third to the fourth.
    clear_hook: Option<(u32, u32, u32, u32)>,
    // After a write to the first offset that clears the bits of the second: at the third, the fourth's bits read as set
    // from the fifth microseconds on, until a write there sets a clear bit (31, 26 or 12) (a one-off event, cleared).
    arm: Vec<(u32, u32, u32, u32, u64)>, timed: Vec<(u32, u32, u64)>,
    // After a write to the first offset that clears the bits of the second, the fifth read after it finds the clock this
    // many microseconds on (the task kept from running).
    stall: Option<(u32, u32, u64)>, stalled: Option<(u32, u64)>,
    // Statuses the hardware sets: bits that read as set, and acknowledge bits that read as their request bit (one lower).
    set: Vec<(u32, u32)>, follow: Vec<(u32, u32)>,
    // Status bits that read as set while bits of a register (the same or another) are set: (at, bits, source, mask).
    derived: Vec<(u32, u32, u32, u32)>,
    // After a write to the first offset, the statuses at the second are no longer answered (a status that never comes).
    cut: Vec<(u32, u32)>,
    // After the first write to the offset, the n-th read cannot be read, once (a table or a stage failing midway).
    fail: Option<(u32, usize)>, armed: Option<usize>,
    // The timing generators' frame counts follow the clock at this period (0: one more at each read).
    frame_us: u64,
    // Step 4's surface, made at its first row; none when `no_surface`.
    surface: Vec<u32>, no_surface: bool, pitch: usize,
    // The compositor's answers to stage 16's offer and to screen_gone (display.wit 1.3); none: it takes them.
    refuse_offer: Option<ScreenRefused>, refuse_gone: Option<ScreenRefused>,
    // Time the log spent (the survey's pacing, `Paced`), added to the clock at the next access (213-DRV-0098).
    pace: Option<Rc<Cell<u64>>>,
    // Live bits, not latched: at the first offset, the second's bits read as set from the third microsecond to the fourth
    // (OPTC_UNDERFLOW_OCCURRED_CURRENT beside the latched OCCURRED_STATUS, 213-DRV-0103).
    window: Vec<(u32, u32, u64, u64)>,
    // The time waited awake (delays under a millisecond) and asleep (a millisecond or more), as the service's clock
    // takes them (213-DRV-0111).
    awake_us: u64, slept_us: u64,
    // A late wake, once: the first sleep that ends at this clock or later ends the second later. `late_after` sets it
    // at a write to its first offset that clears the bits of its second, the third microseconds on.
    late: Option<(u64, u64)>, late_after: Option<(u32, u32, u64, u64)>,
}
impl Engine { fn catch_up(&mut self) { if let Some(p) = &self.pace { self.clock += p.replace(0); } } }
impl Screens for Engine {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> {
        self.ev.push(Ev::Offer(width, height, pitch));
        self.refuse_offer.map_or(Ok(()), Err)
    }
    fn gone(&mut self) -> Result<(), ScreenRefused> {
        self.ev.push(Ev::Gone);
        self.refuse_gone.map_or(Ok(()), Err)
    }
}
impl Canvas for Engine {
    // Slot 5's 20 MiB, made at the first row.
    fn row(&mut self, y: usize, pitch: usize, pixels: &[u32]) -> bool {
        if self.no_surface || pixels.len() > pitch || (y + 1) * pitch * 4 > light::SURFACE_BYTES { return false; }
        if self.surface.is_empty() { self.surface = vec![0; light::SURFACE_BYTES / 4]; }
        self.surface[y * pitch..][..pixels.len()].copy_from_slice(pixels);
        self.pitch = pitch;
        true
    }
    fn sample(&mut self, x: usize, y: usize, pitch: usize) -> Option<u32> { self.surface.get(y * pitch + x).copied() }
}
impl Bus for Engine {
    fn read(&mut self, r: Reg) -> Option<u32> {
        self.reads += 1;
        self.catch_up();
        self.clock += 10;
        if let Some((left, len)) = self.stalled { if left == 0 { self.clock += len; self.stalled = None; } else { self.stalled = Some((left - 1, len)); } }
        if let Some(n) = self.armed { if n == 0 { self.armed = None; return None; } self.armed = Some(n - 1); }
        let o = r.offset();
        if (0..4).any(|t| reg(R::OtgStatusFrameCount, t) == o) && self.frames_move {
            if self.frame_us > 0 { return Some((self.clock / self.frame_us) as u32); }
            self.frames += 1;
            return Some(self.frames);
        }
        let mut v = *self.regs.get(&o).unwrap_or(&0);
        for &(at, bits) in &self.set { if at == o { v |= bits; } }
        for &(at, ack) in &self.follow { if at == o { v = (v & !ack) | ((v & (ack >> 1)) << 1); } }
        for &(at, bits, from) in &self.timed { if at == o && self.clock >= from { v |= bits; } }
        for &(at, bits, from, to) in &self.window { if at == o && (from..to).contains(&self.clock) { v |= bits; } }
        for &(at, bits, src, mask) in &self.derived {
            if at == o { let s = if src == o { *self.regs.get(&o).unwrap_or(&0) } else { *self.regs.get(&src).unwrap_or(&0) }; v = if s & mask != 0 { v | bits } else { v & !bits }; }
        }
        Some(v)
    }
}
impl Poke for Engine {
    fn write(&mut self, w: Writable, v: u32) {
        self.catch_up();
        let o = w.reg().offset();
        self.ev.push(Ev::Write(o, v));
        // The status bits that follow their requests: clocks on, the generator running.
        let v = if (0..4).any(|t| reg(R::OptcInputClockControl, t) == o) { (v & !4) | ((v >> 1) & 1) << 2 }
            else if (0..4).any(|t| reg(R::OtgClockControl, t) == o) { (v & !0x100) | (v & 1) << 8 }
            else if (0..4).any(|t| reg(R::OtgControl, t) == o) { (v & !0x1_0000) | (v & 1) << 16 } else { v };
        self.regs.insert(o, v);
        if let Some((at, target, value)) = self.hook { if at == o { self.regs.insert(target, value); } }
        if let Some((at, mask, target, value)) = self.clear_hook { if at == o && v & mask == 0 { self.regs.insert(target, value); } }
        for (at, mask, target, bits, delay) in self.arm.clone() { if at == o && v & mask == 0 { self.timed.push((target, bits, self.clock + delay)); } }
        let clock = self.clock;
        if v & (1 << 31 | 1 << 26 | 1 << 12) != 0 { self.timed.retain(|&(at, _, from)| at != o || from > clock); }
        if let Some((at, mask, len)) = self.stall { if at == o && v & mask == 0 { self.stalled = Some((4, len)); self.stall = None; } }
        for (at, status) in self.cut.clone() { if at == o { self.set.retain(|s| s.0 != status); self.follow.retain(|s| s.0 != status); self.derived.retain(|s| s.0 != status); } }
        if let Some((at, n)) = self.fail { if at == o { self.armed = Some(n); self.fail = None; } }
        if let Some((at, mask, delay, extra)) = self.late_after { if at == o && v & mask == 0 { self.late = Some((self.clock + delay, extra)); self.late_after = None; } }
    }
}
impl Clock for Engine {
    fn now_us(&mut self) -> u64 { self.catch_up(); self.clock }
    fn delay_us(&mut self, us: u32) {
        self.catch_up();
        self.clock += us as u64;
        if us < 1000 { self.awake_us += us as u64; return; }
        self.slept_us += us as u64;
        if let Some((from, extra)) = self.late { if self.clock >= from { self.clock += extra; self.late = None; } }
    }
}

struct Log(Vec<String>);
impl Out for Log {
    fn line(&mut self, a: fmt::Arguments) { self.0.push(a.to_string()); }
    fn pause(&mut self, _: u32) {}
}

// The firmware's pipe as the model holds it: values in every watched register, so that a change shows.
fn engine() -> Engine {
    let mut e = Engine { frames_move: true, ..Default::default() };
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for (i, o) in watched(&l).into_iter().enumerate() { e.regs.insert(o, 0x1000 + i as u32); }
    e.regs.insert(reg(R::OtgControl, 0), 0x0001_0001);
    e
}
fn watched(l: &Light) -> Vec<u32> {
    // The watch's registers, by the guard: every one is the firmware's.
    let mut v = vec![];
    for r in dcn::ALL { for i in 0..r.instances() { let o = reg(r, i); if l.guard().check(Reg::new(o).unwrap()) == Err(Refused::Firmware(o)) { v.push(o); } } }
    v
}

// --- synthetic VBIOS tables ---

fn image(tables: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut b = vec![0u8; 0x400];
    b[0] = 0x55; b[1] = 0xAA;
    b[0x48..0x4A].copy_from_slice(&0x100u16.to_le_bytes());
    b[0x104..0x108].copy_from_slice(b"ATOM");
    b[0x11E..0x120].copy_from_slice(&0x200u16.to_le_bytes());
    b[0x120..0x122].copy_from_slice(&0x300u16.to_le_bytes());
    b[0x200..0x202].copy_from_slice(&(4u16 + 2 * 81).to_le_bytes());
    b[0x300..0x302].copy_from_slice(&(4u16 + 2 * 34).to_le_bytes());
    for (index, code) in tables {
        let at = b.len();
        b.extend_from_slice(&((6 + code.len()) as u16).to_le_bytes());
        b.extend_from_slice(&[1, 1, 0, 0]);
        b.extend_from_slice(code);
        let e = 0x204 + 2 * *index as usize;
        b[e..e + 2].copy_from_slice(&(at as u16).to_le_bytes());
    }
    b
}
// MOVE_REG of a dword immediate (opcode 1, attr 5 = IMM DWORD) to a dword index; EOT is 91.
fn mov(byte: u32, v: u32) -> Vec<u8> { let mut c = vec![1, 5]; c.extend_from_slice(&((byte / 4) as u16).to_le_bytes()); c.extend_from_slice(&v.to_le_bytes()); c }
const EOT: u8 = 91;
// TEST_REG (74) of an immediate bit, then JUMP_EQUAL (68) back to that test while the bit is clear; `at` is the test's
// offset in the table's code (the 6-byte header comes before it).
fn wait_bit(byte: u32, bit: u32, at: u16) -> Vec<u8> {
    let mut c = vec![74, 5]; c.extend_from_slice(&((byte / 4) as u16).to_le_bytes()); c.extend_from_slice(&(1u32 << bit).to_le_bytes());
    c.push(68); c.extend_from_slice(&(6 + at).to_le_bytes()); c
}
const PHY_A_CNTL0: u32 = (0x34C0 + 0x2940) * 4; // RDPCSTX0_RDPCSTX_PHY_CNTL0
const PHY_E_CNTL0: u32 = (0x34C0 + 0x2940 + 4 * 0xD8) * 4; // RDPCSTX4_RDPCSTX_PHY_CNTL0
const PHY_A_CR_ADDR: u32 = (0x34C0 + 0x2934) * 4; // RDPCSTX0_RDPCS_TX_CR_ADDR
const PHY_A_CR_DATA: u32 = (0x34C0 + 0x2935) * 4; // RDPCSTX0_RDPCS_TX_CR_DATA

fn tables(t12: Vec<u8>, t76: Vec<u8>) -> Vec<u8> {
    image(&[(12, t12), (4, [mov(seg(2, 0x206B), 0x1F), vec![EOT]].concat()), (76, t76)])
}
fn good() -> Vec<u8> {
    tables([mov(seg(1, 0xA0), 1), mov(PHY_A_CNTL0, 0x0020_0001), vec![EOT]].concat(), [mov(seg(2, 0x20B0), 1), vec![EOT]].concat())
}

fn run(img: &[u8], e: &mut Engine, allowed: u8) -> (Outcome, Vec<String>) {
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    let mut interp = atom::Interpreter::new(atom::Image::new(img).unwrap());
    let mut journal = light::Journal::default();
    let mut log = Log(vec![]);
    let outcome = light::run(&l, e, &mut interp, &mut journal, allowed, &mut log);
    (outcome, log.0)
}

// A line that says stage `n` wrote something or started a table.
fn made(x: &str, n: u8) -> bool {
    x.starts_with(&format!("S{} WROTE ", n)) || x.starts_with(&format!("S{} RUNNING ", n)) || (x.starts_with(&format!("S{} T", n)) && x.contains(" WROTE "))
}

fn writes(e: &Engine) -> Vec<(u32, u32)> { e.ev.iter().filter_map(|x| if let Ev::Write(o, v) = x { Some((*o, *v)) } else { None }).collect() }

#[test]
fn all_eight_stages_write_in_the_routes_order_and_never_the_firmwares_pipe() {
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, log) = run(&good(), &mut e, STAGES);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", log);
    // The route's order, by the first write of each block.
    let ws = writes(&e);
    let first = |o: u32| ws.iter().position(|w| w.0 == o).unwrap_or_else(|| panic!("{:05X} never written", o));
    let order = [reg(R::OptcInputClockControl, 1), seg(1, 0xA0), reg(R::PhypllPixclkResyncCntl, 0), reg(R::OtgHTotal, 1),
        reg(R::OppPipeControl, 1), reg(R::DpgControl, 1), reg(R::OptcDataSourceSelect, 1), seg(2, 0x206B), reg(R::DigFeCntl, 0),
        reg(R::DigBeCntl, 0), seg(2, 0x20B0)];
    for pair in order.windows(2) { assert!(first(pair[0]) < first(pair[1]), "{:05X} before {:05X}", pair[0], pair[1]); }
    // The timing written is 1080p60's, as the CTA table has it.
    let o = timing::cta(16).unwrap().otg().unwrap();
    assert_eq!(e.regs[&reg(R::OtgHTotal, 1)], o.h_total);
    assert_eq!(e.regs[&reg(R::OtgVBlankStartEnd, 1)], o.v_blank_start | o.v_blank_end << 16);
    // The firmware's pipe: never written, every watched register as it was.
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for (w, _) in &ws { assert!(l.guard().check(Reg::new(*w).unwrap()).is_ok(), "{:05X}", w); }
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    // Each stage logs every write it will make before its first write; each is said done with the firmware's pipe unchanged.
    for n in 1..=STAGES {
        let will = log.iter().position(|x| x.starts_with(&format!("S{} ", n)) && x.contains("STAGE") && x.contains("NEXT")).unwrap();
        let first = log.iter().position(|x| made(x, n)).unwrap();
        assert!(will < first && log[will + 1..first].iter().any(|x| x.contains(" WILL ")), "stage {}", n);
        assert!(log.iter().any(|x| x.starts_with(&format!("S{} ", n)) && x.contains("DONE; THE FIRMWARE'S PIPE UNCHANGED")));
    }
    assert!(log.iter().any(|x| x.starts_with("S8 T76 WROTE DIG0_DIG_BE_EN_CNTL AT 155C0 = 00000001 (WAS 00000000)")));
    assert!(log.last().unwrap().starts_with(&format!("STEP 3 STOPPED AFTER STAGE 8 OF {}", dcn::STAGES)));
    for x in &log { assert!(x.len() <= 180, "{} bytes: {}", x.len(), x); }
}

#[test]
fn a_build_stops_after_its_allowed_stage_and_leaves_the_rest_unwritten() {
    for allowed in 0..=STAGES {
        let mut e = engine();
        let (outcome, log) = run(&good(), &mut e, allowed);
        assert_eq!(outcome, Outcome::Stopped { done: allowed });
        for n in allowed + 1..=STAGES {
            assert!(log.iter().any(|x| x.starts_with(&format!("S{} ", n)) && x.contains("NOT RUN (THIS BUILD MAY RUN")));
            assert!(!log.iter().any(|x| made(x, n)), "stage {} ran", n);
        }
        if allowed == 0 { assert!(writes(&e).is_empty()); }
    }
}

// Every register written is back to its value before step 3, but for the status bits the model itself sets.
fn restored_but(e: &Engine, before: &HashMap<u32, u32>, except: &[u32]) {
    for (o, _) in writes(e) {
        if except.contains(&o) || o == reg(R::OtgVerticalInterrupt2Position, 1) { continue; } // the once-a-boot mark stays
        let status = e.set.iter().chain(e.follow.iter()).filter(|(at, _)| *at == o).fold(0, |m, (_, bits)| m | bits);
        let value = |m: &HashMap<u32, u32>| m.get(&o).copied().unwrap_or(0) & !status;
        assert_eq!(value(&e.regs), value(before), "{:05X} not restored", o);
    }
}

#[test]
fn a_stage_that_fails_is_undone_with_every_stage_before_it() {
    // The frame count never moves: stage 5 fails, stages 5 to 1 are undone, last first.
    let mut e = engine();
    e.frames_move = false;
    let before = e.regs.clone();
    let (outcome, log) = run(&good(), &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 5, why: Failure::NoFrames(0) });
    let undone: Vec<u8> = log.iter().filter_map(|x| x.strip_prefix('U').and_then(|r| r.split(' ').next()).filter(|_| x.contains(" UNDONE: ")).and_then(|n| n.parse().ok())).collect();
    assert_eq!(undone, vec![5, 4, 3, 2, 1]);
    // Stage 2's off call (table 12 with pixel clock 0) ran before its writes were put back; every register is as before.
    assert!(log.iter().any(|x| x.starts_with("U2 RUNNING TABLE 12")));
    restored_but(&e, &before, &[]);
    assert!(log.iter().any(|x| x.starts_with("STEP 3 FAILED AT STAGE 5 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED")));
    for n in 6..=STAGES { assert!(!log.iter().any(|x| made(x, n)), "stage {} ran after stage 5 failed", n); }
}

#[test]
fn the_undo_leaves_the_phys_control_register_bus_as_it_is() {
    // Table 12 writes PHY A's internal register 0x1234 through the bus; stage 5 then fails. Putting the bus registers
    // back would write a stale value into the PHY: the undo leaves them, says so, and restores everything else.
    let img = tables([mov(seg(1, 0xA0), 1), mov(PHY_A_CR_ADDR, 0x1234), mov(PHY_A_CR_DATA, 0x5678), mov(PHY_A_CNTL0, 0x0020_0001), vec![EOT]].concat(),
        [mov(seg(2, 0x20B0), 1), vec![EOT]].concat());
    let mut e = engine();
    e.frames_move = false;
    let before = e.regs.clone();
    let (outcome, log) = run(&img, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 5, why: Failure::NoFrames(0) });
    // The only writes to the bus are the tables' own (the stage's, and the synthetic off call's that repeats them).
    let by_tables = log.iter().filter(|x| x.contains(" T12 WROTE AT 177D0") || x.contains(" T12 WROTE AT 177D4")).count();
    assert_eq!(writes(&e).iter().filter(|w| w.0 == PHY_A_CR_DATA || w.0 == PHY_A_CR_ADDR).count(), by_tables, "a restore wrote the bus");
    assert!(log.iter().any(|x| x == "U2 PHY A'S CONTROL REGISTER BUS LEFT AS IT IS: 1 ADDRESS AND 1 DATA WRITES; WHAT THEY SET INSIDE THE PHY STAYS"), "{:#?}", log);
    assert!(log.iter().any(|x| x.starts_with("U2 PIXEL CLOCK UNDONE: 3 WRITES RESTORED")), "{:#?}", log);
    restored_but(&e, &before, &[PHY_A_CR_ADDR, PHY_A_CR_DATA]);
}

#[test]
fn a_table_that_writes_the_firmwares_phy_is_refused_and_everything_undone() {
    let img = tables([mov(seg(1, 0xA0), 1), vec![EOT]].concat(), [mov(seg(2, 0x20B0), 1), mov(PHY_E_CNTL0, 0xDEAD), vec![EOT]].concat());
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, log) = run(&img, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 8, why: Failure::Refused(Refused::Firmware(PHY_E_CNTL0)) });
    assert!(!writes(&e).iter().any(|w| w.0 == PHY_E_CNTL0), "the refused write was made");
    // Stage 8's off call is table 76 DISABLE: the synthetic one writes DIG0_DIG_BE_EN_CNTL again before it stops on PHY E,
    // so every write of stage 8 is restored; the others are too.
    assert!(log.iter().any(|x| x.starts_with("U8 TABLE 76 STOPPED")));
    restored_but(&e, &before, &[]);
}

#[test]
fn a_status_the_hardware_never_shows_ends_by_the_limit_and_is_undone() {
    // Table 12 waits for RDPCS_SRAM_INIT_DONE on PHY A with no bound of its own, as the Ryzen's does at 0xB864.
    let img = tables([mov(seg(1, 0xA0), 1), wait_bit(PHY_A_CNTL0, 28, 8), vec![EOT]].concat(), vec![EOT]);
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, log) = run(&img, &mut e, STAGES);
    match outcome {
        Outcome::Failed { stage: 2, why: Failure::Table(atom::Error::TimeLimit { table: 12, .. }) } => {}
        other => panic!("{:?}\n{:#?}", other, log),
    }
    assert!(e.clock >= light::TABLE_TIME_LIMIT_US, "stopped before its time limit");
    // Its off call waits too and ends by the limit; every write of stage 2 is restored anyway, then stage 1's.
    assert!(log.iter().any(|x| x.starts_with("U2 TABLE 12 STOPPED")));
    restored_but(&e, &before, &[]);
    assert!(log.iter().any(|x| x.starts_with("U1 OTG CLOCKS UNDONE")));
}

#[test]
fn more_writes_than_the_undo_holds_fail_the_stage_and_are_undone() {
    // A table that writes on and on (its loop takes its write back in): the journal fills before the step limit.
    let img = tables([mov(seg(1, 0xA0), 1), vec![67, 6, 0], vec![EOT]].concat(), vec![EOT]);
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, _) = run(&img, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 2, why: Failure::JournalFull });
    restored_but(&e, &before, &[]);
}

#[test]
fn a_change_of_the_firmwares_pipe_fails_the_stage() {
    // When stage 3 writes OTG 1's total, the model changes OTG 0's (a pipe the stage must not move).
    let mut e = engine();
    e.hook = Some((reg(R::OtgHTotal, 1), reg(R::OtgHTotal, 0), 0x777));
    let (outcome, log) = run(&good(), &mut e, STAGES);
    match outcome { Outcome::Failed { stage: 3, why: Failure::FirmwareChanged { offset, after: 0x777, .. } } if offset == reg(R::OtgHTotal, 0) => {}, o => panic!("{:?} {:#?}", o, log) }
    assert!(log.iter().any(|x| x.starts_with("STEP 3 FAILED AT STAGE 3") && x.ends_with("CHANGED")));
}

// The firmware's pipe as runs 3 to 7 read OTG0 (2560x1600 in 2720x1646) and HUBP0, with DLG values for a 100 MHz
// reference clock by Display Core's formulas (the hardware's own are batch 9's).
fn ryzen_reference() -> light::Reference {
    // Pipe 0's registers pipe 1 copies: each its index plus a mark, so that a copy shows where it came from; MPCC_CONTROL
    // as a single layer's (top layer only, alpha 255).
    let mut copies = [0u32; light::COPIED.len()];
    for (i, c) in copies.iter_mut().enumerate() { *c = 0x00C0_0000 | i as u32; }
    copies[light::COPIED.len() - 1] = 0x00FF_0002;
    light::Reference { pipe: 0, otg: 0, h_total: 2719, v_total: 1645, h_blank_end: 112, v_blank_end: 43, width: 2560,
        ref_to_pix: 195_264, per_htotal: 259_335, refcyc_h_blank_end: 41, dlg_v_blank_end: 43, line_delivery: 953,
        req_delivery: 24_396, pitch_field: 0xA00, line_delivery_pre: 953, req_delivery_pre: 24_396, fb_base: 0xF4_0000_0000,
        mpcc_top: 1, mpcc_opp: 1, mux: 1, firmware_mux: 0, firmware_bot: 0, refclk_cntl: 0, timer_cntl: 0x1001, vready: 1, copies,
        // OTG0's global sync and HUBP0's QoS as boot 70 read them.
        sync: [0xD, 0x0140_02A8, 0x12C], debug_db: 0, qos_level: 8 }
}
// Pipe 0 as boot 70 read it on the Ryzen (2026-11-22, release 202611220537): the GOP's own DLG, not Display Core's
// (REF_FREQ_TO_PIX_FREQ 0x14E5E, line and request delivery 0); PER_HTOTAL by Display Core's formula at 48 MHz; every
// MPCC n wired from DPP n to OPP n, mode 1.
fn boot70_reference() -> light::Reference {
    light::Reference { ref_to_pix: 0x14E5E, per_htotal: 0x1E641, refcyc_h_blank_end: 0x35, dlg_v_blank_end: 0x13, line_delivery: 0,
        req_delivery: 0, line_delivery_pre: 0, req_delivery_pre: 0, pitch_field: 2560, vready: 0, ..ryzen_reference() }
}
fn step4(reference: Option<light::Reference>) -> Light {
    let (f, c) = EXPECTED;
    Light::new(f, c, timing::CEA_1080P60).unwrap().with_reference(reference).with_build("TESTBUILD")
}
fn run_light(l: &Light, img: &[u8], e: &mut Engine, allowed: u8, undo: bool) -> (Outcome, Vec<String>) {
    let mut interp = atom::Interpreter::new(atom::Image::new(img).unwrap());
    let mut journal = light::Journal::default();
    let mut log = Log(vec![]);
    let outcome = if undo { light::run_then_undo(l, e, &mut interp, &mut journal, allowed, &mut log) } else { light::run(l, e, &mut interp, &mut journal, allowed, &mut log) };
    (outcome, log.0)
}
fn number_after(log: &[String], start: &str, key: &str) -> u64 {
    let l = log.iter().find(|x| x.starts_with(start)).unwrap_or_else(|| panic!("no {:?}", start));
    let rest = &l[l.find(key).unwrap_or_else(|| panic!("no {:?} in {:?}", key, l)) + key.len()..];
    rest.split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

#[test]
fn stages_9_and_10_measure_the_firmwares_clock_derive_pipe_1_and_draw_the_picture() {
    let l = step4(Some(ryzen_reference()));
    let mut e = engine();
    e.frame_us = 16_675; // DP-2 at 59.97 Hz
    let (outcome, log) = run_light(&l, &good(), &mut e, 10, false);
    assert_eq!(outcome, Outcome::Stopped { done: 10 }, "{:#?}", &log[log.len().saturating_sub(16)..]);
    // Stage 9: the firmware's pixel clock from 30 of its frames, the reference clock from REF_FREQ_TO_PIX_FREQ.
    let pixel = number_after(&log, "S9 OTG0 COUNTED 30 FRAMES", "ITS PIXEL CLOCK ");
    let reference = number_after(&log, "S9 OTG0 COUNTED 30 FRAMES", "THE REFERENCE CLOCK ");
    assert!((268_000..=269_000).contains(&pixel) && (99_500..=100_500).contains(&reference), "{} {}", pixel, reference);
    assert!(log.iter().any(|x| x.starts_with("S9 PIPE 0 BY FORMULA: REF/PIX 0.37") && x.ends_with("V_BLANK_END 43 (DLG 43)")), "{:#?}", log.iter().filter(|x| x.starts_with("S9")).collect::<Vec<_>>());
    // Pipe 1's values for 1080p60 at the reference clock found.
    let otg = timing::CEA_1080P60.otg().unwrap();
    let ratio1 = (reference << 19) / 148_500;
    let line = format!("S9 PIPE 1 FOR 1920x1080 AT 148500 KHZ: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, LINE {}, REQ {} (30 REQUESTS A LINE)",
        batches::Ratio19(Some(ratio1 as u32)), reference * (otg.h_total as u64 + 1) * 256 / 148_500, reference * otg.h_blank_end as u64 / 148_500,
        otg.v_blank_end, reference * 1920 / 148_500, reference * (1920 * 1024 / 30) / 148_500);
    assert!(log.iter().any(|x| x.starts_with(&line)), "{}\n{:#?}", line, log.iter().filter(|x| x.starts_with("S9")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x.ends_with("(30 REQUESTS A LINE)")));
    // Stages 9 and 10 write no register: the writes are first light's.
    let mut first_light = engine();
    first_light.frame_us = 16_675;
    run_light(&l, &good(), &mut first_light, 8, false);
    assert_eq!(writes(&e), writes(&first_light));
    for n in [9u8, 10] {
        assert!(log.iter().any(|x| x.starts_with(&format!("S{} {} DONE; THE FIRMWARE'S PIPE UNCHANGED", n, dcn::STAGE_NAMES[n as usize - 1]))));
        assert!(log.iter().any(|x| x.starts_with(&format!("S{} THE ATTACHED HUBPS UNCHANGED, BLANKED", n))));
    }
    // Stage 10: the picture in the surface, read back where stage 10 reads it and elsewhere.
    let lines = picture::Lines::new("TESTBUILD", picture::Size::FIRST);
    for y in (0..picture::HEIGHT).step_by(7) {
        for x in (0..picture::WIDTH).step_by(5) { assert_eq!(e.sample(x, y, picture::PITCH), Some(picture::pixel(&lines, x, y)), "{},{}", x, y); }
    }
    assert!(log.iter().any(|x| x == "S10 DRAWN: 1920x1080 AT SLOT 5'S START, PITCH 1920 PIXELS (7680 BYTES); 9 SAMPLES READ BACK AS DRAWN"));
    assert!(log.iter().any(|x| x == "S10 ITS TEXT: MIND CORE / DISPLAY / HDMI-A-1 1920X1080 / BUILD TESTBUILD"));
    assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 10 OF"));
    // With the undo: step 4's stages first, then step 3's, every register as before.
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 10, true);
    assert_eq!(outcome, Outcome::Undone { done: 10 });
    assert!(log.iter().any(|x| x == "U10 THE SURFACE IS LEFT AS IT IS: NOTHING SCANS IT YET"));
    restored_but(&e, &before, &[]);
}

#[test]
fn stage_9_on_what_boot_70_read_takes_its_reference_clock_from_per_htotal() {
    // Boot 70's pipe 0: its REF_FREQ_TO_PIX_FREQ gives 43.8 MHz, its PER_HTOTAL Display Core's formula at 48 MHz, the
    // crystal through REFCLK_CNTL 0 and the timer's divider 1. Pipe 1 then gets what Display Core would program.
    // Boot 71 (release 202611220554) read the same values and ran stages 9 and 10 on them.
    let l = step4(Some(boot70_reference()));
    let mut e = engine();
    e.frame_us = 16_674; // DP-2 at 59.973 Hz, as boot 70 counted
    e.regs.insert(reg(R::HubpCntl, 1), 0x000F_001B); // HUBP1 blanked, as step 1 read it
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert_eq!(outcome, Outcome::Stopped { done: 15 }, "{:#?}", log.iter().filter(|x| x.starts_with("S9") || x.starts_with("STEP")).collect::<Vec<_>>());
    let s9: Vec<&String> = log.iter().filter(|x| x.starts_with("S9 ")).collect();
    let has = |p: &str| assert!(s9.iter().any(|x| x.contains(p)), "no {:?} in {:#?}", p, s9);
    has("ITS PIXEL CLOCK 268");
    has("THE REFERENCE CLOCK 48000 KHZ");
    has("KHZ BY PER_HTOTAL, 438");
    has("REFCLK_CNTL 00000000 (THE CRYSTAL), GLOBAL TIMER 00001001 (REFDIV 1)");
    has("S9 PIPE 0 BY FORMULA: REF/PIX 0.1787 (SET 0.1632), PER HTOTAL 124");
    has("S9 PIPE 1 FOR 1920x1080 AT 148500 KHZ: REF/PIX 0.3232, PER HTOTAL 182044, H_BLANK_END 62, V_BLANK_END 41, LINE 620, REQ 21183 (30 REQUESTS A LINE)");
    has("S9 PIPE 1: NEXT START 4484 (V_BLANK_START 1121 x 4), PREFETCH LINE 620 REQ 21183");
    // Those values in HUBP1, and the MPC tree as the firmware wired it but MPCC1's bottom (15, none).
    let g = |r: R, i: u8| e.regs.get(&reg(r, i)).copied().unwrap_or(0);
    assert_eq!((g(R::HubpreqRefFreqToPixFreq, 1), g(R::HubpreqDstDimensions, 1), g(R::HubpreqPerLineDelivery, 1)), (169_466, 182_044, 620));
    assert_eq!(g(R::HubpreqSurf0TtuCntl0, 1) & 0x7F_FFFF, 21_183);
    assert_eq!((g(R::MpccTopSel, 1), g(R::MpccBotSel, 1), g(R::MpccOppId, 1), g(R::MpcOutMux, 1)), (1, 0xF, 1, 1));
    // Pipe 1's sync for its plane: the firmware's VUPDATE width and VREADY times at 148.5 MHz, a quarter line's offset,
    // VREADY after the sync by hubp2_vready_at_or_After_vsync's rule; its QoS as Display Core's.
    has("S9 PIPE 1'S SYNC FOR ITS PLANE: VSTARTUP 36 (STAGE 3'S), VUPDATE OFFSET 550 WIDTH 177, VREADY 166 (THE FIRMWARE'S OTG0: VSTARTUP 13, VUPDATE 680 WIDTH 320, VREADY 300); VREADY AT OR AFTER VSYNC 1 (THE FIRMWARE'S HUBP 0)");
    has("S9 PIPE 1'S QOS: LEVEL 8 (THE FIRMWARE'S 8), HIGH WATERMARK 2844 (4 LINES;");
    assert!(log.iter().any(|x| x == "S13 WROTE OTG1_OTG_VUPDATE_PARAM AT 14320 = 00B10226 (WAS 00010000)"), "{:#?}", log.iter().filter(|x| x.starts_with("S13 WROTE OTG1")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x == "S13 WROTE OTG1_OTG_VREADY_PARAM AT 14324 = 000000A6 (WAS 00000001)"));
    assert!(log.iter().any(|x| x.starts_with("S13 WROTE HUBP1_HUBPREQ_DEBUG_DB AT 0EE48 = 00000100")));
    assert_eq!(g(R::HubpreqSurf0TtuCntl0, 1), 8 << 24 | 21_183);
    assert_eq!(g(R::HubpreqTtuQosWm, 1), 2844 << 16);
    // HUBP1's CNTL: stage 13 keeps it blanked with VREADY after the sync, stage 15 unblanks it; the pattern then off.
    let cntl: Vec<u32> = writes(&e).into_iter().filter(|x| x.0 == reg(R::HubpCntl, 1)).map(|x| x.1).collect();
    assert_eq!(cntl, [0x000F_011B, 0x000F_011A]);
    assert!(log.iter().any(|x| x == "S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES"));
    assert!(log.iter().any(|x| x == "S15 WROTE DPG1_DPG_CONTROL AT 135B8 = 00661000 (WAS 00661001)"));
    assert!(log.iter().any(|x| x == "S13 READ BACK HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 0000077F, AS WRITTEN: FIELD 1919"));
    assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 15 OF 16 AS THIS BUILD ALLOWS"));
}

#[test]
fn stages_11_to_15_put_the_surface_on_pipe_1_and_nothing_else_on_the_firmwares() {
    let r = ryzen_reference();
    let l = step4(Some(r));
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert_eq!(outcome, Outcome::Stopped { done: 15 }, "{:#?}", &log[log.len().saturating_sub(16)..]);
    let g = |reg: R, i: u8| e.regs.get(&self::reg(reg, i)).copied().unwrap_or(0);
    // Pipe 1's HUBP on the surface: GPU 0xF4_0100_0000, pitch field 1919, 1920x1080 from 0,0, unblanked.
    assert_eq!((g(R::HubpreqPrimarySurfaceAddressHigh, 1), g(R::HubpreqPrimarySurfaceAddress, 1)), (0xF4, 0x0100_0000));
    assert_eq!(g(R::HubpreqSurfacePitch, 1), 1919);
    assert_eq!((g(R::HubpPriViewportStart, 1), g(R::HubpPriViewportDimension, 1)), (0, 1080 << 16 | 1920));
    assert_eq!(g(R::HubpCntl, 1) & 0x1101, 0x100, "unblanked, TTU on, VREADY after the sync");
    // Its DLG and TTU as stage 9 derived them; the rest copied from pipe 0.
    assert_eq!(g(R::HubpreqRefFreqToPixFreq, 1), ((number_after(&log, "S9 OTG0 COUNTED", "THE REFERENCE CLOCK ") << 19) / 148_500) as u32);
    let own = [R::MpccControl, R::HubpreqTtuQosWm, R::HubpreqCur0TtuCntl0];
    for (i, reg) in light::COPIED.iter().enumerate().filter(|(_, r)| !own.contains(r)) {
        assert_eq!(g(*reg, 1), r.copies[i], "{}", reg.name());
    }
    // The QoS Display Core gives every plane: level 8 for the surface and the cursor, the high watermark four lines.
    let reference = number_after(&log, "S9 OTG0 COUNTED", "THE REFERENCE CLOCK ");
    assert_eq!(g(R::HubpreqTtuQosWm, 1), ((reference * 4 * 2200 / 148_500) as u32) << 16);
    assert_eq!(g(R::HubpreqSurf0TtuCntl0, 1) >> 24, 8);
    let cur = light::COPIED.iter().position(|&c| c == R::HubpreqCur0TtuCntl0).unwrap();
    assert_eq!(g(R::HubpreqCur0TtuCntl0, 1), r.copies[cur] & 0x7F_FFFF | 8 << 24);
    // The sync for the plane, and the DLG's test mode off.
    let pix0 = number_after(&log, "S9 OTG0 COUNTED", "ITS PIXEL CLOCK ");
    let at1 = |x: u64| (x * 148_500).div_ceil(pix0) as u32;
    assert_eq!((g(R::OtgVupdateParam, 1), g(R::OtgVreadyParam, 1)), (at1(320) << 16 | 550, at1(300)));
    assert_eq!(g(R::HubpreqDebugDb, 1), 0x100);
    // DPP1 clocked with a 1920x1080 recout, HUBP1 clocked; MPCC1 from DPP1 to OPP1, its output mux on MPCC1; the
    // pattern off.
    assert_eq!((g(R::DppControl, 1) & 0x10, g(R::HubpClkCntl, 1) & 1), (0x10, 1));
    assert_eq!((g(R::DsclRecoutSize, 1), g(R::DsclMpcSize, 1)), (1080 << 16 | 1920, 1080 << 16 | 1920));
    assert_eq!((g(R::MpccTopSel, 1), g(R::MpccBotSel, 1), g(R::MpccOppId, 1), g(R::MpccControl, 1), g(R::MpcOutMux, 1)), (1, 0xF, 1, 0x00FF_0002, 1));
    assert_eq!(g(R::DpgControl, 1) & 1, 0);
    // Every write of stages 11 to 15 to pipe 1's blocks or OPP1's pattern generator; nothing of pipe 0's.
    let first_late = log.iter().position(|x| x.starts_with("S11 WROTE ")).unwrap();
    for x in log[first_late..].iter().filter(|x| x.contains(" WROTE ")) {
        let at = u32::from_str_radix(x.split(" AT ").nth(1).unwrap().split(' ').next().unwrap(), 16).unwrap();
        assert!(l.guard().check(Reg::new(at).unwrap()) != Err(Refused::Firmware(at)), "{}", x);
        assert!(l.guard().check_at(Reg::new(at).unwrap(), 11).is_ok(), "{}", x);
    }
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    // The log: the pitch, and no new underflow twice.
    assert!(log.iter().any(|x| x == "S13 PITCH FIELD 1919 (1920 PIXELS LESS ONE, AS DISPLAY CORE WRITES IT); THE FIRMWARE'S HUBP0 HOLDS 2560 FOR ITS 2560 PIXELS"));
    // The pitch read back just after it is written, and logged with its field.
    let pitch = log.iter().position(|x| x == "S13 READ BACK HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 0000077F, AS WRITTEN: FIELD 1919").expect("the read-back");
    assert!(log[pitch - 1].starts_with("S13 WROTE HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 0000077F"));
    assert!(log.iter().any(|x| x == "S13 WILL READ HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C BACK"));
    assert_eq!(log.iter().filter(|x| x.starts_with("S15 NO NEW UNDERFLOW: HUBP0 AND ODM0 (THE FIRMWARE'S), HUBP1 AND ODM1")).count(), 1);
    // The watch with the pattern off: 300 frames read one by one, all clean, five lines of them.
    assert!(log.iter().any(|x| x.starts_with("S15 WATCHED 300 FRAMES OF OTG1 IN ") && x.contains(" WITH THE PATTERN OFF: 0 PASSED BETWEEN TWO READS")), "{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x == "S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES"));
    let frames: Vec<&String> = log.iter().filter(|x| x.starts_with("S15 FRAMES ")).collect();
    assert_eq!(frames.len(), 5);
    assert_eq!(frames[4].as_str(), &format!("S15 FRAMES 241 TO 300: {}", ".".repeat(60)));
    // The attached HUBP is watched until stage 10, then is step 4's own.
    for n in 5..=15u8 { assert_eq!(log.iter().any(|x| x.starts_with(&format!("S{} THE ATTACHED HUBPS UNCHANGED", n))), n <= 10, "stage {}", n); }
    // The guard keeps pipe 1's blocks shut to first light's stages, and pipe 0's to all.
    for (reg, i) in [(R::HubpCntl, 1u8), (R::DppControl, 1), (R::MpccTopSel, 1), (R::MpcOutMux, 1)] {
        let x = Reg::new(self::reg(reg, i)).unwrap();
        assert_eq!(l.guard().check_at(x, 8), Err(Refused::OffRoute(x.offset())), "{}", reg.name());
        assert!(l.guard().check_at(x, 11).is_ok(), "{}", reg.name());
    }
    for (reg, i) in [(R::HubpCntl, 0u8), (R::DppControl, 0), (R::MpccTopSel, 0)] {
        let x = Reg::new(self::reg(reg, i)).unwrap();
        assert_eq!(l.guard().check_at(x, 15), Err(Refused::Firmware(x.offset())), "{}", reg.name());
    }
    // With the undo, everything back as before, pipe 1 included.
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    assert_eq!(run_light(&l, &good(), &mut e, 15, true).0, Outcome::Undone { done: 15 });
    restored_but(&e, &before, &[]);
    // A register that does not keep what was written (as if it rounded the pitch): said, with both fields; not a failure.
    let mut e = engine();
    e.frame_us = 16_675;
    let at = reg(R::HubpreqSurfacePitch, 1);
    e.hook = Some((at, at, 0x780));
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert_eq!(outcome, Outcome::Stopped { done: 15 });
    assert!(log.iter().any(|x| x == "S13 READ BACK HUBPREQ1_DCSURF_SURFACE_PITCH AT 0EE8C = 00000780, NOT 0000077F AS WRITTEN: FIELD 1920, NOT 1919"));
}

#[test]
fn an_underflow_at_stage_15_fails_and_is_undone_whole_and_a_busy_mpcc_stops_step_4() {
    // HUBP1's underflow status set once it is set up: stage 15 sees it new, fails, and undoes 15 to 1.
    let l = step4(Some(ryzen_reference()));
    let mut e = engine();
    e.frame_us = 16_675;
    e.hook = Some((reg(R::HubpCntl, 1), reg(R::HubpCntl, 1), 1 << 28));
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    match outcome { Outcome::Failed { stage: 15, why: Failure::Underflow { offset, .. } } if offset == reg(R::HubpCntl, 1) => {}, o => panic!("{:?} {:#?}", o, &log[log.len().saturating_sub(12)..]) }
    assert!(log.iter().any(|x| x == "STEP 3 FAILED AT STAGE 15 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED"));
    // At once: stage 15's writes put back, last first, before any line, then named; then stages 14 to 1.
    let failed = log.iter().position(|x| x.starts_with("S15 UNBLANK FAILED: AN UNDERFLOW AT 0EE3C")).unwrap();
    assert!(log[failed].ends_with("; ITS WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; UNDOING STAGES 14 TO 1"), "{}", log[failed]);
    let found = log[..failed].iter().rposition(|x| x.starts_with("S15 ")).unwrap();
    assert!(log[found + 1..failed].iter().all(|x| x.starts_with("U15 ")), "{:#?}", &log[found..failed]);
    assert!(log[found + 1].starts_with("U15 RESTORED HUBP1_DCHUBP_CNTL AT 0EE3C = ") && log[found + 1].ends_with(" AT ONCE"), "{:#?}", &log[found..failed]);
    assert_eq!(log.iter().filter(|x| x.starts_with("U15 RESTORED")).count(), 1, "the pattern was still on: one write to put back");
    assert!(log[failed + 1..].iter().any(|x| x.starts_with("U14 RESTORED")));
    // The writes in order, with HUBP1's status read as underflowing once its surface is set (stage 13), its CNTL kept:
    // the unblank, then at once HUBP1's CNTL as stage 13 left it.
    let cntl = reg(R::HubpCntl, 1);
    let mut e = engine();
    e.frame_us = 16_675;
    e.derived.push((cntl, 1 << 28, reg(R::HubpreqSurfacePitch, 1), !0));
    assert!(matches!(run_light(&l, &good(), &mut e, 15, false).0, Outcome::Failed { stage: 15, why: Failure::Underflow { .. } }));
    let pipe_1 = |e: &Engine| -> Vec<(u32, u32)> {
        let w: Vec<(u32, u32)> = writes(e).into_iter().filter(|x| x.0 == cntl || x.0 == reg(R::DpgControl, 1)).collect();
        w[w.iter().position(|x| x.0 == cntl).unwrap()..].to_vec() // from stage 13's
    };
    let w = pipe_1(&e);
    assert_eq!((w[1].0, w[1].1, w[2]), (cntl, w[0].1 & !0x1001, (cntl, w[0].1)), "unblanked, then put back at once: {:X?}", w);
    // The underflow after the pattern is off: both writes put back at once, the pattern first, then HUBP1's CNTL.
    let flag = 0xF_FFF0;
    let mut e = engine();
    e.frame_us = 16_675;
    e.clear_hook = Some((reg(R::DpgControl, 1), 1, flag, 1));
    e.derived.push((cntl, 1 << 28, flag, 1));
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert!(matches!(outcome, Outcome::Failed { stage: 15, why: Failure::Underflow { .. } }), "{:?}", outcome);
    let failed = log.iter().position(|x| x.starts_with("S15 UNBLANK FAILED")).unwrap();
    let names: Vec<&String> = log[..failed].iter().filter(|x| x.starts_with("U15 RESTORED")).collect();
    assert_eq!(names.len(), 2, "{:#?}", names);
    assert!(names[0].starts_with("U15 RESTORED DPG1_DPG_CONTROL") && names[1].starts_with("U15 RESTORED HUBP1_DCHUBP_CNTL"), "{:#?}", names);
    // The watch ran its 300 frames, clearing HUBP1's status at each (this underflow comes back every frame), then the
    // stage failed: the pattern on, then HUBP1's CNTL as stage 13 left it.
    assert!(log.iter().any(|x| x.starts_with("S15 HUBP1: UNDERFLOW IN 300 FRAMES, REQUEST TIMEOUT IN 0; ODM1 UNDERFLOW IN 0; THE FIRST AT FRAME 1 ")), "{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x == &format!("S15 FRAMES 241 TO 300: {}", "U".repeat(60))));
    let w = pipe_1(&e);
    let off = w.iter().position(|x| x.0 == reg(R::DpgControl, 1) && x.1 & 1 == 0).unwrap();
    let on = off + w[off..].iter().position(|x| x.0 == reg(R::DpgControl, 1) && x.1 & 1 == 1).unwrap();
    assert!(w[off + 1..on].iter().all(|x| x.0 == cntl), "only the clears between: {:X?}", &w[off..on]);
    assert_eq!(w[off + 1..on].iter().filter(|x| x.1 & 1 << 31 != 0).count(), 300);
    let stage_13 = w.iter().find(|x| x.0 == cntl).unwrap().1;
    assert_eq!(w[on + 1], (cntl, stage_13), "the pattern on, then HUBP1's CNTL: {:X?}", &w[on..]);
    restored_but(&e, &before, &[flag]);
    restored_but(&e, &before, &[reg(R::HubpCntl, 1)]);
    assert_eq!(e.regs[&reg(R::DpgControl, 1)] & 1, before.get(&reg(R::DpgControl, 1)).copied().unwrap_or(0) & 1);
    // Pipe 1's MPCC or its OPP's mux in use: stage 9 is not ready, says which and with what values, and stops with first
    // light left in place, nothing of pipe 1 written.
    for (r, why) in [
        (light::Reference { mpcc_opp: 0, ..ryzen_reference() }, Failure::MpccInUse { mpcc: 1, opp_id: 0, firmware_mux: 0, firmware_bot: 0 }),
        (light::Reference { firmware_mux: 1, ..ryzen_reference() }, Failure::MpccInUse { mpcc: 1, opp_id: 1, firmware_mux: 1, firmware_bot: 0 }),
        (light::Reference { firmware_bot: 1, ..ryzen_reference() }, Failure::MpccInUse { mpcc: 1, opp_id: 1, firmware_mux: 0, firmware_bot: 1 }),
        (light::Reference { mux: 0, ..ryzen_reference() }, Failure::OutputMuxInUse { opp: 1, mux: 0 }),
    ] {
        let l = step4(Some(r));
        let mut e = engine();
        e.frame_us = 16_675;
        let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
        assert_eq!(outcome, Outcome::NotReady { done: 8, stage: 9, why });
        assert!(!writes(&e).iter().any(|w| w.0 == reg(R::HubpCntl, 1) || w.0 == reg(R::MpccTopSel, 1)));
        assert!(log.iter().any(|x| x.starts_with(&format!("S9 REFERENCE NOT READY: {}; STAGES 1 TO 8 STAY AS THEY ARE, STEP 4 STOPS HERE", why))), "{:#?}", &log[log.len() - 4..]);
        assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 8 OF 16: STAGE 9 NOT READY;") && log.last().unwrap().ends_with("ARE LEFT IN PLACE"));
        assert_eq!(e.regs[&reg(R::DigBeEnCntl, 0)] & 1, 1, "first light's back end left enabled");
    }
    // A build of 0 names stages 11 to 15 with what they will do.
    let mut log = Log(vec![]);
    light::describe(&l, false, &mut log);
    assert!(log.0.iter().any(|x| x.starts_with("S13 WOULD SET HUBP1, BLANKED: THE SURFACE AT FB + 0x1000000, PITCH 1919")));
}

#[test]
fn stage_9_without_a_reference_and_stage_10_without_a_surface_stop_with_first_light_in_place() {
    let mut e = engine();
    e.frame_us = 16_675;
    let (outcome, log) = run_light(&step4(None), &good(), &mut e, 10, false);
    assert_eq!(outcome, Outcome::NotReady { done: 8, stage: 9, why: Failure::NoReference });
    assert!(log.iter().any(|x| x.starts_with("S9 REFERENCE NOT READY: NO REFERENCE")));
    assert_eq!(e.regs[&reg(R::DigBeEnCntl, 0)] & 1, 1);
    let mut e = engine();
    e.frame_us = 16_675;
    e.no_surface = true;
    let (outcome, log) = run_light(&step4(Some(ryzen_reference())), &good(), &mut e, 10, false);
    assert_eq!(outcome, Outcome::NotReady { done: 9, stage: 10, why: Failure::NoSurface });
    assert!(log.iter().any(|x| x.starts_with("S10 DRAW NOT READY: SLOT 5'S SURFACE IS NOT MAPPED; STAGES 1 TO 9 STAY AS THEY ARE")));
    // With the undo the build asks for, first light is undone all the same, every register as before.
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&step4(None), &good(), &mut e, 10, true);
    assert_eq!(outcome, Outcome::NotReady { done: 8, stage: 9, why: Failure::NoReference });
    assert!(log.last().unwrap().ends_with("ARE UNDONE"));
    restored_but(&e, &before, &[]);
    // A firmware's pipe changed in stage 9 (its own reads only) still undoes everything.
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    e.hook = Some((reg(R::DigBeCntl, 0), reg(R::OtgHTotal, 0), 0x777)); // set once stage 7 writes; seen after stage 7
    let (outcome, _) = run_light(&step4(Some(ryzen_reference())), &good(), &mut e, 10, false);
    assert!(matches!(outcome, Outcome::Failed { why: Failure::FirmwareChanged { .. }, .. }), "{:?}", outcome);
    restored_but(&e, &before, &[reg(R::OtgHTotal, 0)]);
    // Step 1's values give the reference only when all its batches were read and they name the firmware's pipe.
    let (f, c) = EXPECTED;
    let mut s = found();
    assert_eq!(light::reference(&s, &f, &c), None, "found() holds no batch 9 values");
    let want = ryzen_reference();
    for (r, v) in [(R::OtgHTotal, 2719), (R::OtgVTotal, 1645), (R::OtgHBlankStartEnd, 112 << 16 | 2672), (R::OtgVBlankStartEnd, 43 << 16 | 1643),
        (R::HubpPriViewportDimension, 1600 << 16 | 2560), (R::HubpreqRefFreqToPixFreq, 195_264), (R::HubpreqDstDimensions, 259_335),
        (R::HubpreqBlankOffset0, 43 << 16 | 41), (R::HubpreqPerLineDelivery, 953), (R::HubpreqSurf0TtuCntl0, 8 << 24 | 24_396), (R::HubpreqSurfacePitch, 0xA00),
        (R::HubpreqPerLineDeliveryPre, 953), (R::HubpreqSurf0TtuCntl1, 24_396), (R::DcnVmFbLocationBase, 0xF400),
        (R::OtgVstartupParam, 0xD), (R::OtgVupdateParam, 0x0140_02A8), (R::OtgVreadyParam, 0x12C), (R::HubpreqDebugDb, 0)] {
        s.set(r, 0, v);
    }
    for (r, v) in light::COPIED.iter().zip(want.copies) { s.set(*r, 0, v); }
    s.set(R::HubpCntl, 0, 0x000F_0102); // HUBP_VREADY_AT_OR_AFTER_VSYNC 1
    // The MPC tree and the clocks' route as boot 70 read them.
    for n in 0..4u8 { s.set(R::MpccTopSel, n, n as u32); s.set(R::MpccBotSel, n, n as u32); s.set(R::MpccOppId, n, n as u32); s.set(R::MpcOutMux, n, n as u32); }
    s.set(R::RefclkCntl, 0, 0); s.set(R::DchubbubGlobalTimerCntl, 0, 0x1001);
    assert_eq!(light::reference(&s, &f, &c), Some(want));
    s.set_batches(9);
    assert_eq!(light::reference(&s, &f, &c), None, "batch 10 not read");
}

#[test]
fn stage_15s_watch_keeps_the_picture_up_and_names_the_frame_of_pipe_1s_first_underflow_but_not_the_firmwares() {
    let l = step4(Some(boot70_reference()));
    let (cntl1, cntl0, dpg) = (reg(R::HubpCntl, 1), reg(R::HubpCntl, 0), reg(R::DpgControl, 1));
    // A one-off underflow on HUBP1 some 17 frames after the pattern goes off, cleared by the watch: kept, the watch
    // goes on to its end with the picture up, then the stage fails and is put back at once.
    let mut e = engine();
    e.frame_us = 16_674;
    e.arm.push((dpg, 1, cntl1, 1 << 28, 17 * 16_674 + 8_000));
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert!(matches!(outcome, Outcome::Failed { stage: 15, why: Failure::Underflow { offset, .. } } if offset == cntl1), "{:?}", outcome);
    let s15: Vec<&String> = log.iter().filter(|x| x.starts_with("S15 ")).collect();
    let first = s15.iter().find(|x| x.starts_with("S15 HUBP1: UNDERFLOW IN 1 FRAMES, REQUEST TIMEOUT IN 0; ODM1 UNDERFLOW IN 0; THE FIRST AT FRAME ")).unwrap_or_else(|| panic!("{:#?}", s15));
    let frame: u32 = first.split("AT FRAME ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!((17..=19).contains(&frame), "{}", first);
    assert!(first.ends_with("; CLEARED 1 TIMES"));
    let marks: String = s15.iter().filter(|x| x.starts_with("S15 FRAMES ")).map(|x| x.split(": ").nth(1).unwrap()).collect();
    assert_eq!((marks.len(), marks.matches('U').count(), marks.find('U')), (300, 1, Some(frame as usize - 1)));
    assert!(log.iter().any(|x| x.starts_with("S15 UNBLANK FAILED: AN UNDERFLOW AT 0EE3C") && x.ends_with("ITS WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; UNDOING STAGES 14 TO 1")));
    // The firmware's HUBP underflowing during the watch: at once, the watch cut short.
    let mut e = engine();
    e.frame_us = 16_674;
    e.arm.push((dpg, 1, cntl0, 1 << 28, 5 * 16_674));
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert!(matches!(outcome, Outcome::Failed { stage: 15, why: Failure::Underflow { offset, .. } } if offset == cntl0), "{:?}", outcome);
    assert!(!log.iter().any(|x| x.starts_with("S15 WATCHED ")));
    assert!(log.iter().any(|x| x.starts_with("S15 UNBLANK FAILED: AN UNDERFLOW AT 0EB1C") || x.starts_with(&format!("S15 UNBLANK FAILED: AN UNDERFLOW AT {:05X}", cntl0))), "{:#?}", &log[log.len().saturating_sub(4)..]);
    restored_but(&e, &before, &[]);
    // A request timing out at HUBP1: kept as T, the stage failed after the watch.
    let mut e = engine();
    e.frame_us = 16_674;
    e.arm.push((dpg, 1, cntl1, 2 << 20, 40 * 16_674));
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert!(matches!(outcome, Outcome::Failed { stage: 15, why: Failure::RequestTimeout { offset, .. } } if offset == cntl1), "{:?}", outcome);
    assert!(log.iter().any(|x| x.starts_with("S15 HUBP1: UNDERFLOW IN 0 FRAMES, REQUEST TIMEOUT IN 1;")), "{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x.starts_with("S15 UNBLANK FAILED: A REQUEST TIMEOUT AT 0EE3C")));
    // Display kept from running for 676 ms just after the pattern goes off, as in boot 74: the frames it missed are
    // marked, and the gap named.
    let mut e = engine();
    e.frame_us = 16_674;
    e.stall = Some((dpg, 1, 676_000));
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    assert_eq!(outcome, Outcome::Stopped { done: 15 });
    let watched = log.iter().find(|x| x.starts_with("S15 WATCHED 300 FRAMES")).unwrap();
    let passed: u32 = watched.split("WITH THE PATTERN OFF: ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    let gap = number_after(&log, "S15 WATCHED 300 FRAMES", "THE LONGEST GAP BETWEEN READS ");
    assert!((39..=41).contains(&passed) && (676_000..677_000).contains(&gap), "{}", watched);
    assert!(log.iter().any(|x| x.starts_with("S15 FRAMES   1 TO  60: ---------")), "{:#?}", log.iter().filter(|x| x.starts_with("S15 FRAMES")).collect::<Vec<_>>());
}

#[test]
fn stage_16_offers_the_surface_as_display_wit_1_3_has_it_and_its_undo_lets_it_go_before_stage_15_blanks() {
    let l = step4(Some(ryzen_reference()));
    let mut e = engine();
    e.frame_us = 16_675;
    let (outcome, log) = run_light(&l, &good(), &mut e, 16, false);
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log[log.len().saturating_sub(12)..]);
    // One offer, after stage 15's last write: 1920x1080, a row 7680 bytes apart, pixels slot 5's; nothing let go.
    let offers: Vec<usize> = e.ev.iter().enumerate().filter(|(_, x)| matches!(x, Ev::Offer(..))).map(|(i, _)| i).collect();
    assert_eq!(offers.len(), 1);
    assert_eq!(e.ev[offers[0]], Ev::Offer(1920, 1080, 7680));
    assert!(!e.ev[offers[0]..].iter().any(|x| matches!(x, Ev::Write(..) | Ev::Gone)), "no write after the offer");
    assert!(log.iter().any(|x| x == "S16 OFFERED THE SECOND SCREEN: 1920x1080, PITCH 7680 BYTES, BGR, ITS PIXELS SLOT 5'S SURFACE; THE COMPOSITOR MIRRORS THE FIRST INTO IT"));
    assert!(log.iter().any(|x| x.starts_with("S16 OFFER DONE; THE FIRMWARE'S PIPE UNCHANGED")));
    assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 16 OF 16 AS THIS BUILD ALLOWS"));
    // Stage 16 writes no register: the writes are stage 15's.
    let mut fifteen = engine();
    fifteen.frame_us = 16_675;
    run_light(&l, &good(), &mut fifteen, 15, false);
    assert_eq!(writes(&e), writes(&fifteen));
    // The undo: screen_gone first, then stage 15's writes restored; every register as before.
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 16, true);
    assert_eq!(outcome, Outcome::Undone { done: 16 });
    let gone = e.ev.iter().position(|x| *x == Ev::Gone).expect("screen_gone");
    assert_eq!(e.ev.iter().filter(|x| **x == Ev::Gone).count(), 1);
    let offer = e.ev.iter().position(|x| matches!(x, Ev::Offer(..))).unwrap();
    let last_15 = writes(&fifteen).len();
    let writes_before_gone = e.ev[..gone].iter().filter(|x| matches!(x, Ev::Write(..))).count();
    assert!(offer < gone && writes_before_gone == last_15, "screen_gone before any undo write: {} writes before it, {} by stage 15", writes_before_gone, last_15);
    let u16 = log.iter().position(|x| x == "U16 SCREEN_GONE: THE COMPOSITOR HAS LET THE SURFACE GO").unwrap();
    let u15 = log.iter().position(|x| x.starts_with("U15 ")).unwrap();
    assert!(u16 < u15);
    restored_but(&e, &before, &[]);
    // screen_gone refused: said so, and the undo goes on.
    let mut e = engine();
    e.frame_us = 16_675;
    e.refuse_gone = Some(ScreenRefused::NoScreen);
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 16, true);
    assert_eq!(outcome, Outcome::Undone { done: 16 });
    assert!(log.iter().any(|x| x == "U16 SCREEN_GONE NOT TAKEN: NO SCREEN; THE UNDO GOES ON"), "{:#?}", log.iter().filter(|x| x.starts_with("U16")).collect::<Vec<_>>());
    restored_but(&e, &before, &[]);
}

#[test]
fn stage_16_refused_or_without_a_client_stops_with_the_picture_on_the_second_output() {
    let l = step4(Some(ryzen_reference()));
    for refused in [ScreenRefused::Unavailable, ScreenRefused::NoScreen, ScreenRefused::NoMemory, ScreenRefused::Denied, ScreenRefused::BadScreen, ScreenRefused::Call] {
        let mut e = engine();
        e.frame_us = 16_675;
        e.refuse_offer = Some(refused);
        let (outcome, log) = run_light(&l, &good(), &mut e, 16, false);
        assert_eq!(outcome, Outcome::NotReady { done: 15, stage: 16, why: Failure::Screen(refused) }, "{:#?}", &log[log.len().saturating_sub(6)..]);
        assert!(log.iter().any(|x| x == &format!("S16 OFFER NOT READY: THE COMPOSITOR DID NOT TAKE THE SECOND SCREEN: {}; STAGES 1 TO 15 STAY AS THEY ARE, STEP 4 STOPS HERE", refused)),
            "{:#?}", &log[log.len().saturating_sub(4)..]);
        assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 15 OF 16: STAGE 16 NOT READY;") && log.last().unwrap().ends_with("ARE LEFT IN PLACE"));
        // Pipe 1 left unblanked on the surface, screen_gone not called.
        assert_eq!(e.regs[&reg(R::HubpCntl, 1)] & 1, 0);
        assert!(!e.ev.contains(&Ev::Gone));
    }
    // Refused with the undo the build asks for: stages 15 to 1 undone, screen_gone not called, every register as before.
    let mut e = engine();
    e.frame_us = 16_675;
    e.refuse_offer = Some(ScreenRefused::Unavailable);
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 16, true);
    assert_eq!(outcome, Outcome::NotReady { done: 15, stage: 16, why: Failure::Screen(ScreenRefused::Unavailable) });
    assert!(log.iter().any(|x| x.starts_with("S16 OFFER NOT READY: THE COMPOSITOR DID NOT TAKE THE SECOND SCREEN: NO CLIENT OF IT (SLOT_DISPLAY) WAS GRANTED")));
    assert!(log.last().unwrap().ends_with("ARE UNDONE"));
    assert!(!e.ev.contains(&Ev::Gone));
    restored_but(&e, &before, &[]);
    // A build of 15 names stage 16 with what it would do; a build of 0 too.
    let mut e = engine();
    e.frame_us = 16_675;
    let (_, log) = run_light(&l, &good(), &mut e, 15, false);
    assert!(log.iter().any(|x| x == "S16 OFFER: NOT RUN (THIS BUILD MAY RUN 15); WOULD:"));
    assert!(log.iter().any(|x| x == "S16 WOULD OFFER THE SURFACE TO THE COMPOSITOR AS THE SECOND SCREEN: 1920x1080, PITCH 7680 BYTES, BGR (DISPLAY.WIT 1.3); UNDO: SCREEN_GONE FIRST"));
    assert!(!e.ev.iter().any(|x| matches!(x, Ev::Offer(..))));
    let mut log = Log(vec![]);
    light::describe(&l, false, &mut log);
    assert!(log.0.iter().any(|x| x.starts_with("S16 WOULD OFFER THE SURFACE TO THE COMPOSITOR")), "{:#?}", log.0.iter().filter(|x| x.starts_with("S16")).collect::<Vec<_>>());
}

#[test]
fn step_3_runs_once_while_lit_by_the_mark_it_leaves_in_its_otg() {
    let mark = reg(R::OtgVerticalInterrupt2Position, 1);
    // The first start leaves the mark before stage 1 and keeps it through the undo.
    let mut e = engine();
    let (outcome, log) = run(&good(), &mut e, 1);
    assert_eq!(outcome, Outcome::Stopped { done: 1 });
    assert_eq!(writes(&e)[0], (mark, light::RESTART_MARK));
    assert!(log.iter().any(|x| x == "S0 MARKED OTG1_OTG_VERTICAL_INTERRUPT2_POSITION AT 14298 = 5A5A (WAS 00000000): A START OF DISPLAY AGAIN RUNS NO STAGE; A MACHINE BOOT OR AN UNPLUG CLEARS IT"), "{:#?}", log);
    let mut undone = engine();
    let img = good();
    let mut interp = atom::Interpreter::new(atom::Image::new(&img).unwrap());
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    light::run_then_undo(&l, &mut undone, &mut interp, &mut light::Journal::default(), 2, &mut Log(vec![]));
    assert_eq!(undone.regs[&mark], light::RESTART_MARK);
    // A start again in the same boot finds it and writes nothing.
    let mut again = engine();
    again.regs.insert(mark, light::RESTART_MARK);
    let (outcome, log) = run(&good(), &mut again, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 0, why: Failure::StartedAgain });
    assert!(writes(&again).is_empty());
    assert!(log.iter().any(|x| x == "STEP 3 NOT RUN: DISPLAY HAS STARTED STEP 3 ALREADY (THE MARK IN ITS OTG): THE STAGES RUN ONCE UNTIL AN UNPLUG TAKES THEM BACK (OTG1_OTG_VERTICAL_INTERRUPT2_POSITION AT 14298 HOLDS 5A5A)"), "{:#?}", log);
    // A build that may run no stage leaves no mark.
    let mut none = engine();
    run(&good(), &mut none, 0);
    assert!(writes(&none).is_empty());
}

#[test]
fn display_asks_for_the_stack_steps_3_and_4_need() {
    // linker.ld's PT_GNU_STACK size is the stack display runs on (262-KRN-0164); without it a task has 64 KiB.
    let ld = include_str!("../display/linker.ld");
    assert!(ld.contains("stack PT_GNU_STACK FLAGS(6);"));
    let at = ld.find(".stack (NOLOAD) : { . = . + 0x").expect("no .stack section") + ".stack (NOLOAD) : { . = . + 0x".len();
    let size = usize::from_str_radix(ld[at..].split(';').next().unwrap(), 16).unwrap();
    assert_eq!(size, light::STACK_BYTES);
    assert!(2 * light::STEP3_STACK_BYTES <= size, "{} needed of {}", light::STEP3_STACK_BYTES, size);
    // The 64 KiB that faulted on 2026-11-22 would not have passed this bound.
    assert!(2 * light::STEP3_STACK_BYTES > 64 * 1024, "{}", light::STEP3_STACK_BYTES);
}

#[test]
fn a_build_that_undoes_what_it_ran_leaves_every_register_as_before() {
    let run_undo = |e: &mut Engine, allowed: u8| {
        let (f, c) = EXPECTED;
        let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
        let img = good();
        let mut interp = atom::Interpreter::new(atom::Image::new(&img).unwrap());
        let mut journal = light::Journal::default();
        let mut log = Log(vec![]);
        let outcome = light::run_then_undo(&l, e, &mut interp, &mut journal, allowed, &mut log);
        (outcome, log.0)
    };
    for allowed in [1, 2, 8] {
        let mut e = engine();
        let before = e.regs.clone();
        let (outcome, log) = run_undo(&mut e, allowed);
        assert_eq!(outcome, Outcome::Undone { done: allowed }, "{:#?}", log);
        assert!(!writes(&e).is_empty());
        restored_but(&e, &before, &[]);
        let undone: Vec<u8> = log.iter().filter_map(|x| x.strip_prefix('U').and_then(|r| r.split(' ').next()).filter(|_| x.contains(" UNDONE: ")).and_then(|n| n.parse().ok())).collect();
        assert_eq!(undone, (1..=allowed).rev().collect::<Vec<u8>>());
        assert!(log[..6].iter().any(|x| x.starts_with("S0 THIS BUILD UNDOES THE STAGES IT RAN")));
        assert!(log.last().unwrap().starts_with(&format!("STEP 3 UNDONE AFTER STAGE {} OF {} AS THIS BUILD ASKS; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED", allowed, dcn::STAGES)));
        assert!(!log.iter().any(|x| x.contains("LEFT IN PLACE")));
    }
    // A build of 0 writes nothing and has nothing to undo; a stage that fails is undone as in `run`.
    let mut e = engine();
    assert_eq!(run_undo(&mut e, 0).0, Outcome::Stopped { done: 0 });
    assert!(writes(&e).is_empty());
    let mut e = engine();
    e.frames_move = false;
    assert_eq!(run_undo(&mut e, STAGES).0, Outcome::Failed { stage: 5, why: Failure::NoFrames(0) });
}

#[test]
fn the_hubp_that_follows_step_3s_otg_is_read_after_stages_5_to_8() {
    let hubp1 = reg(R::HubpCntl, 1);
    // Its value as on the Ryzen: blanked, in blank, following OTG 1.
    let engine1 = || { let mut e = engine(); e.regs.insert(hubp1, 0x000F_001B); e };
    let mut e = engine1();
    let (outcome, log) = run(&good(), &mut e, STAGES);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", log);
    assert!(log.iter().any(|x| x.starts_with("S0 HUBP1 FOLLOWS OTG1, POWERED AND BLANKED: NEVER WRITTEN, ITS DCHUBP_CNTL READ AFTER EACH STAGE FROM 5 ON")));
    for n in 1..=STAGES { assert_eq!(log.iter().any(|x| x.starts_with(&format!("S{} THE ATTACHED HUBPS UNCHANGED, BLANKED (1 REGISTERS)", n))), n >= 5, "stage {}", n); }
    assert!(!writes(&e).iter().any(|w| w.0 == hubp1), "HUBP1 was written");
    // Its statuses moving when OTG 1 starts (in blank, the XRQ's requests) fail no stage.
    let mut e = engine1();
    e.hook = Some((reg(R::OptcDataSourceSelect, 1), hubp1, 0x000F_001B ^ (0x8 | 0x000E_0000)));
    assert_eq!(run(&good(), &mut e, STAGES).0, Outcome::Stopped { done: 8 });
    // Leaving its blank in stage 5 fails stage 5, which is undone with every stage before it.
    let mut e = engine1();
    let before = e.regs.clone();
    e.hook = Some((reg(R::OptcDataSourceSelect, 1), hubp1, 0x000F_001A));
    let (outcome, log) = run(&good(), &mut e, STAGES);
    match outcome { Outcome::Failed { stage: 5, why: Failure::AttachedChanged { offset, before: 0x11, after: 0x10 } } if offset == hubp1 => {}, o => panic!("{:?} {:#?}", o, log) }
    assert!(log.iter().any(|x| x.starts_with("S5 START FAILED: THE ATTACHED HUBP'S")));
    restored_but(&e, &before, &[]);
    // A change before stage 5 is not looked for until stage 5, and is caught there.
    let mut e = engine1();
    e.hook = Some((reg(R::OtgHTotal, 1), hubp1, 0x000F_001A));
    match run(&good(), &mut e, STAGES).0 { Outcome::Failed { stage: 5, why: Failure::AttachedChanged { .. } } => {}, o => panic!("{:?}", o) }
}

#[test]
fn the_firmwares_status_bits_moving_fail_no_stage_but_a_setting_beside_them_does() {
    // Statuses that move by themselves, whole fields (the Ryzen's HUBP0 reads its XRQ field as F): DCHUBP_CNTL's
    // outstanding-request, in-blank, XRQ, timeout and underflow; OPTC_DOUBLE_BUFFER_PENDING.
    let hubp = reg(R::HubpCntl, 0);
    let odm = reg(R::OptcInputGlobalControl, 0);
    for (target, bits) in [(hubp, 0x2 | 0x8 | 0x000E_0000 | 0x00E0_0000 | 0x6000_0000), (odm, 0x8000_0000)] {
        let mut e = engine();
        let was = e.regs[&target];
        e.hook = Some((reg(R::OtgHTotal, 1), target, was ^ bits));
        let (outcome, log) = run(&good(), &mut e, STAGES);
        assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:05X} {:#?}", target, log);
    }
    // HUBP_VTG_SEL (bits 4 to 7) is a setting: its change fails the stage.
    let mut e = engine();
    let was = e.regs[&hubp];
    e.hook = Some((reg(R::OtgHTotal, 1), hubp, was ^ 1 << 4));
    let (outcome, log) = run(&good(), &mut e, STAGES);
    match outcome { Outcome::Failed { stage: 3, why: Failure::FirmwareChanged { offset, .. } } if offset == hubp => {}, o => panic!("{:?} {:#?}", o, log) }
}

#[test]
fn a_build_that_may_run_nothing_only_describes_and_has_no_device() {
    // describe takes the plan and the log, nothing that reads or writes.
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    let mut log = Log(vec![]);
    light::describe(&l, true, &mut log);
    let lines = log.0;
    assert!(lines[0].contains("THIS BUILD MAY RUN 0"));
    assert!(lines.iter().any(|x| x.contains("AS THE SURVEY EXPECTS: STEP 1 NOT READ")));
    for n in 1..=STAGES {
        assert!(lines.iter().any(|x| x.starts_with(&format!("S{} {}: NOT RUN", n, dcn::STAGE_NAMES[n as usize - 1]))));
        assert!(lines.iter().any(|x| x.starts_with(&format!("S{} WOULD ", n))));
        assert!(lines.iter().any(|x| x.starts_with(&format!("S{} UNDO: ", n))));
    }
    assert!(lines.iter().any(|x| x.starts_with("S2 WOULD RUN TABLE 12 (SETPIXELCLOCK 1.7) WITH 16 BYTES: C8 A8 16 00 14 1E 02 00 01")));
    assert!(lines.iter().any(|x| x.starts_with("S8 WOULD RUN TABLE 76 (DIG1TRANSMITTERCONTROL 1.6) WITH 32 BYTES: 00 01 02 04")));
    assert!(lines.iter().any(|x| x.starts_with("S2 UNDO: TABLE 12 WITH 16 BYTES 00 00 00 00 14 00 00 01 FF") && x.ends_with("THEN EVERY WRITE RESTORED")));
    assert!(lines.iter().any(|x| x.starts_with("S8 UNDO: TABLE 76 WITH 32 BYTES 00 00 02 00")));
    assert!(!lines.iter().any(|x| x.contains("WROTE") || x.contains("RUNNING")));
    assert_eq!(lines.last().unwrap(), &format!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", dcn::STAGES));
    for x in &lines { assert!(x.len() <= 180, "{} bytes: {}", x.len(), x); }
}

#[test]
fn the_mode_is_cta_1080p60_which_hdmi_a_1s_edid_lists() {
    let vics = [1u8, 2, 3, 4, 5, 16, 17, 18, 19, 20, 31, 63, 93, 94, 95, 96, 97];
    let t = timing::first_light_mode(&vics);
    assert_eq!(t, timing::cta(16).unwrap());
    assert_eq!(t, timing::CEA_1080P60);
    assert_eq!((t.h_total(), t.v_total(), t.pixel_khz, t.refresh_millihertz()), (2200, 1125, 148_500, 60_000));
    assert!(t.h_sync_positive && t.v_sync_positive && t.pixel_khz <= timing::DVI_SINGLE_LINK_KHZ);
    let check = |vic: u8, total: (u32, u32), mhz: u32| { let t = timing::cta(vic).unwrap(); assert_eq!(((t.h_total(), t.v_total()), t.refresh_millihertz()), (total, mhz), "VIC {}", vic); };
    check(1, (800, 525), 59_940);
    check(4, (1650, 750), 60_000);
    check(19, (1980, 750), 50_000);
    check(31, (2640, 1125), 50_000);
    assert_eq!(timing::cta(2), None);
    assert_eq!(timing::first_light_mode(&[4, 1]), timing::cta(4).unwrap());
    assert_eq!(timing::first_light_mode(&[]), timing::cta(1).unwrap());
}

#[test]
fn the_vfct_table_yields_the_radeons_image() {
    let mut t = vec![0u8; 0x60];
    t[..4].copy_from_slice(b"VFCT");
    t[52..56].copy_from_slice(&0x40u32.to_le_bytes());
    let image = [0x55u8, 0xAA, 1, 2, 3, 4];
    t.resize(0x40 + 28, 0);
    t[0x40 + 12..0x40 + 14].copy_from_slice(&0x1002u16.to_le_bytes());
    t[0x40 + 14..0x40 + 16].copy_from_slice(&0x1638u16.to_le_bytes());
    t[0x40 + 24..0x40 + 28].copy_from_slice(&(image.len() as u32).to_le_bytes());
    t.extend_from_slice(&image);
    let len = t.len() as u32;
    t[4..8].copy_from_slice(&len.to_le_bytes());
    assert_eq!(atom::vfct_image(&t), Some((0x1002, 0x1638, &image[..])));
    let mut short = t.clone(); short[4..8].copy_from_slice(&(len - 1).to_le_bytes());
    assert_eq!(atom::vfct_image(&short), None, "the image past the table's own length");
    let mut bad = t.clone(); bad[0x40 + 28] = 0;
    assert_eq!(atom::vfct_image(&bad), None, "no ROM signature");
    assert_eq!(atom::vfct_image(&t[..40]), None);
    assert_eq!(atom::vfct_image(b"XXXX"), None);
}

// The statuses the Ryzen's tables wait for, answered as the hardware answered them (hwdocs, "What first light's VBIOS
// tables write"; the watched run of 2026-11-22): SRAM init done; each lane's TX_ACK after its TX_REQ; the symbol clock on
// while its gate is off or it is enabled; the half-rate symbol clock after its enable; the PLL's state on while any
// lane's MPLL_EN in PHY_CNTL6 is (dpcs_2_1_0_sh_mask.h).
const fn phy(d: u32) -> u32 { (0x34C0 + d) * 4 }
const STATUSES: [u32; 5] = [phy(0x2940), phy(0x2943), phy(0x2928), phy(0x2931), phy(0x294C)];
fn answered() -> Engine {
    let mut e = engine();
    e.set.push((phy(0x2940), 1 << 28)); // RDPCS_SRAM_INIT_DONE
    e.follow.push((phy(0x2943), 0x2020_2020)); // RDPCS_PHY_DP_TXn_ACK after TXn_REQ
    e.derived.push((phy(0x2928), 1 << 2, phy(0x2928), 0b11)); // DPCS_SYMCLK_CLOCK_ON: GATE_DIS or EN
    e.follow.push((phy(0x2931), 1 << 10)); // RDPCS_SYMCLK_DIV2_CLOCK_ON after its EN
    e.derived.push((phy(0x294C), 1 << 7, phy(0x2946), 0x4444)); // RDPCS_PHY_DP_MPLLB_STATE: a lane's MPLL_EN
    e
}

// The Ryzen as the watched run found it (2026-11-22, boot 66, after a restart out of Linux): every register step 3
// wrote, at its value before step 3. PHY A's RDPCSTX_SCRATCH (0x177DC) holds 0x300, the VBIOS's mark of a PHY it set up.
const WARM: [(u32, u32); 56] = [(0x13E7C, 0x00000000), (0x14318, 0x00000000), (0x1783C, 0xB008003E), (0x17844, 0x3F00003E), (0x17840, 0x1410003E), (0x1782C, 0x00010B00), (0x17830, 0x00000105), (0x17834, 0x00000000), (0x17838, 0x11000000), (0x1781C, 0x22AA0003), (0x17820, 0x00001000), (0x17824, 0x01000AC1), (0x17828, 0x00000002), (0x17818, 0x001D3333), (0x1780C, 0x02020202), (0x00580, 0x00000000), (0x177A0, 0x00000005), (0x177C4, 0x001171F0), (0x177C0, 0x00D00000), (0x177DC, 0x00000300), (0x00400, 0x00000000), (0x0051C, 0x00000000), (0x00510, 0x0000C000), (0x14204, 0x80000300), (0x141A8, 0x00000000), (0x141B0, 0x00000000), (0x141AC, 0x00000000), (0x141B4, 0x00000000), (0x141BC, 0x00000000), (0x141C0, 0x00000000), (0x141C4, 0x00000000), (0x141DC, 0x00000000), (0x141D8, 0x00000000), (0x141E0, 0x00000000), (0x14210, 0x00000000), (0x0E7A4, 0x00000000), (0x1431C, 0x00000000), (0x14320, 0x00010000), (0x14324, 0x00000000), (0x13E70, 0x00000000), (0x141B8, 0x00000000), (0x13698, 0x00000000), (0x135C0, 0x00000000), (0x135D0, 0x00000000), (0x135B8, 0x00000000), (0x13E6C, 0x0000F100), (0x15668, 0x00000001), (0x154AC, 0x00000063), (0x154A0, 0x00000000), (0x155BC, 0x00010000), (0x174B8, 0x03020100), (0x174B4, 0x01000100), (0x17848, 0x7434003E), (0x155C0, 0x00000100), (0x15684, 0x00000000), (0x177A4, 0x0000C000)];
fn warm() -> Engine {
    let mut e = answered();
    for (o, v) in WARM { e.regs.insert(o, v); }
    e
}

// A stage that failed: every register written before it is as before (but PHY A's control register bus, left by
// design), the firmware's pipe reads as before, and the log says so.
fn undone_whole(e: &Engine, before: &HashMap<u32, u32>, log: &[String], stage: u8) {
    restored_but(e, before, &[PHY_A_CR_ADDR, PHY_A_CR_DATA]);
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X} after stage {} failed", o, stage); }
    let end = format!("STEP 3 FAILED AT STAGE {} AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED", stage);
    assert!(log.iter().any(|x| x == &end), "{:#?}", &log[log.len().saturating_sub(12)..]);
}

// Stage `stage` failing at each read from `after` (a write that comes before it) until the failures move past it: each
// one undoes it and every stage before, and its off call, if it has one, runs to its end. The number of its failures.
fn fail_at_each_read(img: &[u8], stage: u8, after: u32, answered_engine: &dyn Fn() -> Engine) -> usize {
    let mut seen = 0;
    for k in 0..4000 {
        let mut e = answered_engine();
        e.fail = Some((after, k));
        let before = e.regs.clone();
        let (outcome, log) = run(img, &mut e, STAGES);
        match outcome {
            Outcome::Failed { stage: s, why: Failure::Unreadable(_) } if s < stage => undone_whole(&e, &before, &log, s),
            Outcome::Failed { stage: s, why: Failure::Unreadable(_) | Failure::Table(atom::Error::Unreadable { .. }) } if s == stage => {
                undone_whole(&e, &before, &log, s);
                for (n, t) in [(2u8, 12u8), (8, 76)] {
                    if n <= stage { assert!(log.iter().any(|x| x.starts_with(&format!("U{} TABLE {} DONE", n, t))), "stage {} read {}: U{}'s call did not end", stage, k, n); }
                }
                seen += 1;
            }
            o => { eprintln!("stage {} read {}: {:?}", stage, k, o); return seen }
        }
    }
    panic!("stage {} kept failing", stage);
}

#[test]
fn a_stage_that_fails_at_any_read_is_undone_whole_with_its_off_calls_run_to_their_end() {
    // Synthetic tables: stages 2 (table 12), 6 (table 4) and 8 (table 76) failing at every read.
    for (stage, after) in [(2u8, reg(R::OtgClockControl, 1)), (6, reg(R::OptcDataSourceSelect, 1)), (8, reg(R::DigBeCntl, 0))] {
        assert!(fail_at_each_read(&good(), stage, after, &engine) > 0, "stage {}", stage);
    }
    // The Ryzen's own tables, when its image is given.
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { return };
    let bytes = std::fs::read(&path).unwrap();
    // From a cold PHY A (table 12's long path) and from the state the watched run found (its short path).
    for (state, start) in [("cold", &answered as &dyn Fn() -> Engine), ("warm", &warm)] {
        let mut counts = vec![];
        for (stage, after) in [(2u8, reg(R::OtgClockControl, 1)), (6, reg(R::OptcDataSourceSelect, 1)), (8, reg(R::DigBeCntl, 0))] {
            counts.push((stage, fail_at_each_read(&bytes, stage, after, start)));
        }
        eprintln!("the Ryzen's tables failing at each read, {}: {:?} (stage, failures at that stage)", state, counts);
        assert!(counts.iter().all(|&(_, n)| n > 0));
    }
}

#[test]
fn stage_8_failing_at_each_of_its_waits_is_undone_whole() {
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("stage_8_failing_at_each_of_its_waits_is_undone_whole SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    // Once stage 7 has written, each status in turn stops being answered: a wait of table 76's that ends by its limit.
    let mut waits = vec![];
    for (at, start) in STATUSES.iter().flat_map(|&at| [(at, answered as fn() -> Engine), (at, warm as fn() -> Engine)]) {
        let mut e = start();
        e.cut.push((reg(R::DigBeCntl, 0), at));
        let before = e.regs.clone();
        let (outcome, log) = run(&bytes, &mut e, STAGES);
        match outcome {
            Outcome::Failed { stage: 8, why: Failure::Table(atom::Error::TimeLimit { .. } | atom::Error::StepLimit { .. }) } => {
                undone_whole(&e, &before, &log, 8);
                for x in log.iter().filter(|x| x.starts_with("U8 ") && !x.contains(" RESTORED ") && !x.contains(" WROTE ")) { eprintln!("{:05X} cut: {}", at, x); }
                waits.push(at);
            }
            Outcome::Stopped { done: 8 } => {} // table 76 does not wait for this one
            o => panic!("{:05X}: {:?} {:#?}", at, o, &log[log.len().saturating_sub(12)..]),
        }
    }
    eprintln!("table 76 waits for: {:05X?}", waits);
    assert!(!waits.is_empty());
}

// Every write of the watched run on the Ryzen (2026-11-22, release 202611220438, boot 66) in its order: (stage, table
// (0 for the plan's own), byte offset, value), from its log.
const WATCHED_RUN: [(u8, u8, u32, u32); 95] = [(1, 0, 0x13E7C, 0x00000003), (1, 0, 0x14318, 0x00000003), (2, 12, 0x1783C, 0xB008003E), (2, 12, 0x1783C, 0xB038003E), (2, 12, 0x1783C, 0xB038003E), (2, 12, 0x17844, 0x3F00003E), (2, 12, 0x17840, 0x1E14003E), (2, 12, 0x1782C, 0x000105A0), (2, 12, 0x1782C, 0x000105A0), (2, 12, 0x17830, 0x00000021), (2, 12, 0x17834, 0x00000000), (2, 12, 0x17838, 0x11000000), (2, 12, 0x17838, 0x11000000), (2, 12, 0x1781C, 0xE0000001), (2, 12, 0x17820, 0x00000000), (2, 12, 0x17824, 0x00000000), (2, 12, 0x17828, 0x00000000), (2, 12, 0x17818, 0x001D3337), (2, 12, 0x17818, 0x001D3377), (2, 12, 0x17818, 0x001D3777), (2, 12, 0x17818, 0x001D7777), (2, 12, 0x1780C, 0x12121212), (2, 12, 0x1780C, 0x22222222), (2, 12, 0x00580, 0x00000001), (2, 12, 0x177A0, 0x00000007), (2, 12, 0x177C4, 0x001173F0), (2, 12, 0x177C0, 0x00D01000), (2, 12, 0x177C0, 0x00D03000), (2, 12, 0x177C0, 0x00D07000), (2, 12, 0x177C0, 0x00D0F000), (2, 12, 0x177DC, 0x00000300), (2, 12, 0x00400, 0x00000000), (2, 12, 0x0051C, 0x00000000), (2, 12, 0x00510, 0x0000C000), (2, 12, 0x00400, 0x00000101), (2, 0, 0x00400, 0x00000101), (3, 0, 0x14204, 0x80000300), (3, 0, 0x141A8, 0x00000897), (3, 0, 0x141B0, 0x002C0000), (3, 0, 0x141AC, 0x00C00840), (3, 0, 0x141B4, 0x00000000), (3, 0, 0x141BC, 0x00000464), (3, 0, 0x141C0, 0x00000464), (3, 0, 0x141C4, 0x00000464), (3, 0, 0x141DC, 0x00050000), (3, 0, 0x141D8, 0x00290461), (3, 0, 0x141E0, 0x00000000), (3, 0, 0x14210, 0x00000000), (3, 0, 0x0E7A4, 0x00000000), (3, 0, 0x14204, 0x80000300), (3, 0, 0x1431C, 0x00000024), (3, 0, 0x14320, 0x00010000), (3, 0, 0x14324, 0x00000001), (3, 0, 0x0E7A4, 0x04610000), (3, 0, 0x13E70, 0x00000000), (3, 0, 0x141B8, 0x00000000), (4, 0, 0x13698, 0x00000001), (4, 0, 0x135C0, 0x07800438), (4, 0, 0x135D0, 0x00000000), (4, 0, 0x135B8, 0x00661001), (5, 0, 0x13E6C, 0x0000F100), (5, 0, 0x0E7A4, 0x84610000), (5, 0, 0x14204, 0x80000301), (6, 4, 0x15668, 0x00000000), (6, 4, 0x154AC, 0x0000001F), (6, 4, 0x154A0, 0x00000000), (6, 4, 0x154A0, 0x00000400), (6, 4, 0x154A0, 0x00000000), (6, 0, 0x154A0, 0x00000001), (7, 0, 0x155BC, 0x00020100), (8, 76, 0x155BC, 0x00020100), (8, 76, 0x155BC, 0x00020100), (8, 76, 0x155BC, 0x00020100), (8, 76, 0x174B8, 0x01000302), (8, 76, 0x174B4, 0x00000100), (8, 76, 0x1780C, 0x03030303), (8, 76, 0x1780C, 0x22222222), (8, 76, 0x1783C, 0xB038A034), (8, 76, 0x17840, 0x1E14A034), (8, 76, 0x17844, 0x3F00A034), (8, 76, 0x17848, 0x7434A034), (8, 76, 0x155C0, 0x00000101), (8, 76, 0x15684, 0x0000010F), (8, 76, 0x174B8, 0x11000302), (8, 76, 0x177C0, 0x00D0F000), (8, 76, 0x177C0, 0x00D1F000), (8, 76, 0x177A4, 0x0001C000), (8, 76, 0x1780C, 0x00000000), (8, 76, 0x1780C, 0x04040404), (8, 76, 0x17818, 0x001D4444), (8, 76, 0x1780C, 0x14141414), (8, 76, 0x1780C, 0x24242424), (8, 76, 0x1780C, 0x0C0C0C0C), (8, 0, 0x154A0, 0x01000401), (8, 0, 0x154A0, 0x00000001)];

#[test]
fn hdmi_signalling_at_1080p_lights_hdmi_a_1_with_the_same_clock_and_is_undone_whole() {
    let (f, c) = EXPECTED;
    let l = Light::new_with(f, c, timing::CEA_1080P60, dcn::Signal::Hdmi).unwrap();
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 8, false);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log[log.len().saturating_sub(10)..]);
    assert!(log.iter().any(|x| x.ends_with("MODE 1920x1080 AT 148.5 MHZ IN HDMI SIGNALLING (CTA VIC 16)")), "{:#?}", &log[..6]);
    let g = |r: R| e.regs.get(&reg(r, 0)).copied().unwrap_or(0);
    assert_eq!((g(R::HdmiControl) & 0x3100_001F, g(R::HdmiVbiPacketControl) & 0x1031, g(R::HdmiGc) & 1), (0x19, 0x31, 0));
    assert_eq!((g(R::AfmtGenericHdr), g(R::AfmtGeneric0), g(R::AfmtGeneric1)), (0x000D_0282, 0x0828_101F, 16));
    assert_eq!((g(R::HdmiGenericPacketControl0) & 3, g(R::HdmiGenericPacketControl1) & 0xFFFF), (3, 2));
    // Every HDMI write logged in stage 6, after table 4 and before stage 7.
    let s6 = log.iter().position(|x| x.starts_with("S6 WROTE DIG0_HDMI_CONTROL")).unwrap();
    let s7 = log.iter().position(|x| x.starts_with("S7 BACK END: STAGE 7")).unwrap();
    assert!(log[..s6].iter().any(|x| x.starts_with("S6 TABLE 4 DONE")) && s6 < s7);
    assert_eq!(log[s6..s7].iter().filter(|x| x.starts_with("S6 WROTE ")).count(), 18);
    // The same writes as DVI's but for stage 6's and the tables' mode.
    let mut dvi = engine();
    run_light(&Light::new(f, c, timing::CEA_1080P60).unwrap(), &good(), &mut dvi, 8, false);
    let hdmi_only: Vec<(u32, u32)> = writes(&e).into_iter().filter(|w| !writes(&dvi).iter().any(|d| d.0 == w.0)).collect();
    assert_eq!(hdmi_only.len(), 18, "{:X?}", hdmi_only);
    // Undone whole.
    let mut e = engine();
    assert_eq!(run_light(&l, &good(), &mut e, 8, true).0, Outcome::Undone { done: 8 });
    restored_but(&e, &before, &[]);
    // The Ryzen's own tables on its warm state: tables 4 and 76 in HDMI mode, the PHY's short path, nothing through its bus.
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("hdmi_signalling_at_1080p... (the Ryzen's tables) SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    let mut e = warm();
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &bytes, &mut e, 8, false);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log[log.len().saturating_sub(10)..]);
    // In HDMI mode at the same clock: table 12 two steps more (310, DVI's 308), tables 4 and 76 their HDMI paths, the same
    // writes in number.
    for x in ["S2 TABLE 12 DONE: 310 STEPS, 4 CALLS, 10 US OF DELAYS, 33 WRITES", "S6 TABLE 4 DONE: 33 STEPS, 2 CALLS, 2 US OF DELAYS, 5 WRITES",
              "S8 TABLE 76 DONE: 289 STEPS, 9 CALLS, 420 US OF DELAYS, 23 WRITES", "S2 THE PHY WAS SET UP ALREADY (SCRATCH 0x300): THE SHORT PATH, NO WRITE THROUGH ITS BUS"] {
        assert!(log.iter().any(|l| l == x), "no {:?} in {:#?}", x, log.iter().filter(|l| l.contains("TABLE") && l.contains("DONE") || l.starts_with("S2 THE")).collect::<Vec<_>>());
    }
    assert!(!writes(&e).iter().any(|w| w.0 == PHY_A_CR_ADDR || w.0 == PHY_A_CR_DATA));
    let mut e = warm();
    assert_eq!(run_light(&l, &bytes, &mut e, 8, true).0, Outcome::Undone { done: 8 });
    restored_but(&e, &before, &[]);
}

// The Ryzen's monitor's own mode (213-DRV-0095, boot 78's EDID): 3440x1440 at 60 Hz, 324 MHz in 3600x1500.
const WIDE: timing::Timing = timing::Timing { pixel_khz: 324_000, h_active: 3440, h_front: 48, h_sync: 32, h_back: 80, v_active: 1440, v_front: 3,
    v_sync: 5, v_back: 52, h_sync_positive: true, v_sync_positive: false, interlaced: false };

#[test]
fn the_monitors_own_mode_in_hdmi_runs_all_sixteen_stages_at_3440x1440_and_is_undone_whole() {
    let (f, c) = EXPECTED;
    let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
    assert_eq!(l.surface, picture::Size { width: 3440, height: 1440, pitch: 3456 });
    let mut e = engine();
    e.frame_us = 16_674;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 16, false);
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log[log.len().saturating_sub(12)..]);
    let has = |p: &str| assert!(log.iter().any(|x| x.contains(p)), "no {:?} in {:#?}", p, log.iter().filter(|x| x.starts_with("S9 ") || x.starts_with("S0 ")).collect::<Vec<_>>());
    has("MODE 3440x1440 AT 324 MHZ IN HDMI SIGNALLING (CTA VIC 0)");
    // Stage 9 for 324 MHz at the 48 MHz reference, 54 requests a line; stage 13 at pitch 3456.
    has("S9 PIPE 1 FOR 3440x1440 AT 324000 KHZ: REF/PIX 0.1481, PER HTOTAL 136533, H_BLANK_END 16, V_BLANK_END 57, LINE 509, REQ 9664 (54 REQUESTS A LINE)");
    has("S9 PIPE 1'S SYNC FOR ITS PLANE: VSTARTUP 52 (STAGE 3'S), VUPDATE OFFSET 900 WIDTH ");
    has("HIGH WATERMARK 2133 (4 LINES;");
    let g = |r: R, i: u8| e.regs.get(&reg(r, i)).copied().unwrap_or(0);
    assert_eq!((g(R::HubpreqSurfacePitch, 1), g(R::HubpPriViewportDimension, 1), g(R::DsclRecoutSize, 1), g(R::DsclMpcSize, 1)),
               (3455, 1440 << 16 | 3440, 1440 << 16 | 3440, 1440 << 16 | 3440));
    assert_eq!(g(R::DpgDimensions, 1), 3440 << 16 | 1440);
    // Stage 10 at that size, read back; stage 16 offers its shape.
    assert!(log.iter().any(|x| x == "S10 DRAWN: 3440x1440 AT SLOT 5'S START, PITCH 3456 PIXELS (13824 BYTES); 9 SAMPLES READ BACK AS DRAWN"));
    assert!(log.iter().any(|x| x == "S10 ITS TEXT: MIND CORE / DISPLAY / HDMI-A-1 3440X1440 / BUILD TESTBUILD"));
    let lines = picture::Lines::new("TESTBUILD", l.surface);
    for (x, y) in [(0, 0), (3439, 1439), (1720, 100), (1000, 700)] { assert_eq!(e.surface.get(y * 3456 + x).copied(), Some(picture::pixel(&lines, x, y)), "{},{}", x, y); }
    assert!(e.ev.contains(&Ev::Offer(3440, 1440, 13_824)));
    // HDMI signalling: no VIC, no aspect code in the AVI infoframe; the watch clean.
    assert_eq!((g(R::AfmtGeneric0, 0), g(R::AfmtGeneric1, 0)), (0x0808_104F, 0));
    assert!(log.iter().any(|x| x == "S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES"));
    // Undone whole.
    let mut e = engine();
    e.frame_us = 16_674;
    assert_eq!(run_light(&l, &good(), &mut e, 16, true).0, Outcome::Undone { done: 16 });
    restored_but(&e, &before, &[]);
    // A mode whose surface does not fit slot 5 is refused before any stage.
    let big = timing::Timing { pixel_khz: 297_000, h_active: 3840, h_front: 176, h_sync: 88, h_back: 296, v_active: 2160, v_front: 8, v_sync: 10, v_back: 72,
        h_sync_positive: true, v_sync_positive: true, interlaced: false };
    assert_eq!(Light::new_with(f, c, big, dcn::Signal::Hdmi).err(), Some(light::Refusal::Surface { bytes: 3840 * 2160 * 4 }));
    // The Ryzen's tables at 324 MHz in HDMI, on its warm state: the PHY's short path, tables 4 and 76 in HDMI mode.
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("the_monitors_own_mode... (the Ryzen's tables) SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    let mut e = warm();
    let (outcome, log) = run_light(&l, &bytes, &mut e, 8, false);
    assert_eq!(outcome, Outcome::Stopped { done: 8 });
    for x in ["S2 TABLE 12 DONE: 308 STEPS, 4 CALLS, 10 US OF DELAYS, 33 WRITES", "S6 TABLE 4 DONE: 33 STEPS, 2 CALLS, 2 US OF DELAYS, 5 WRITES",
              "S8 TABLE 76 DONE: 300 STEPS, 9 CALLS, 420 US OF DELAYS, 23 WRITES", "S2 THE PHY WAS SET UP ALREADY (SCRATCH 0x300): THE SHORT PATH, NO WRITE THROUGH ITS BUS"] {
        assert!(log.iter().any(|l| l == x), "no {:?} in {:#?}", x, log.iter().filter(|l| l.contains("TABLE") && l.contains("DONE")).collect::<Vec<_>>());
    }
    assert!(!writes(&e).iter().any(|w| w.0 == PHY_A_CR_ADDR || w.0 == PHY_A_CR_DATA));
    // Its four clock-dependent RDPCS values for 324 MHz, as the probe of 2026-11-22 found them.
    let w = writes(&e);
    for (o, v) in [(0x1783C, 0xB028_003E), (0x1782C, 0x0001_0660), (0x17830, 0x0000_0011), (0x1781C, 0x8000_0001)] {
        assert!(w.contains(&(o, v)), "{:05X} = {:08X}", o, v);
    }
}

#[test]
fn the_ryzens_tables_on_its_warm_state_take_the_watched_runs_path() {
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("the_ryzens_tables_on_its_warm_state_take_the_watched_runs_path SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    let mut e = warm();
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log[log.len().saturating_sub(12)..]);
    // The tables' runs as the hardware logged them: table 12 found PHY A set up (RDPCSTX_SCRATCH 0x300) and skipped it.
    for x in ["S2 TABLE 12 DONE: 308 STEPS, 4 CALLS, 10 US OF DELAYS, 33 WRITES", "S6 TABLE 4 DONE: 31 STEPS, 2 CALLS, 2 US OF DELAYS, 5 WRITES",
              "S8 TABLE 76 DONE: 286 STEPS, 9 CALLS, 420 US OF DELAYS, 23 WRITES"] {
        assert!(log.iter().any(|l| l == x), "no {:?}", x);
    }
    // Every write in the hardware's order and value, but two status bits a read-modify-write carried: the lanes'
    // TX_ACK, still set on the hardware just after TX_REQ fell, and DIG_FE_CNTL's bit 24 at the DIG_START pulse.
    let ours: Vec<(u8, u8, u32, u32)> = log.iter().filter_map(|l| {
        let (stage, rest) = l.strip_prefix('S')?.split_once(' ')?;
        let (table, rest) = match rest.strip_prefix('T') { Some(r) => { let (t, r) = r.split_once(' ')?; (t.parse().ok()?, r) } None => (0u8, rest) };
        let rest = rest.strip_prefix("WROTE ")?;
        let at = rest.find("AT ")?;
        let (off, val) = rest[at + 3..].split_once(" = ")?;
        Some((stage.parse().ok()?, table, u32::from_str_radix(off, 16).ok()?, u32::from_str_radix(&val[..8], 16).ok()?))
    }).collect();
    assert_eq!(ours.len(), WATCHED_RUN.len());
    for (i, (a, b)) in ours.iter().zip(WATCHED_RUN.iter()).enumerate() {
        let status = if b.2 == phy(0x2943) { 0x2020_2020 } else if b.2 == reg(R::DigFeCntl, 0) { 1 << 24 } else { 0 };
        assert_eq!((a.0, a.1, a.2, a.3 & !status), (b.0, b.1, b.2, b.3 & !status), "write {}", i + 1);
    }
    assert!(!ours.iter().any(|w| w.2 == PHY_A_CR_ADDR || w.2 == PHY_A_CR_DATA), "a write through PHY A's control register bus");
    assert!(log.iter().any(|l| l == "S2 THE PHY WAS SET UP ALREADY (SCRATCH 0x300): THE SHORT PATH, NO WRITE THROUGH ITS BUS"));
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    // From a cold PHY A (its scratch 0), table 12 sets it up first: the long path, 37 writes through the bus.
    let mut e = answered();
    let (_, log) = run(&bytes, &mut e, 2);
    assert!(log.iter().any(|l| l == "S2 TABLE 12 DONE: 530 STEPS, 4 CALLS, 10 US OF DELAYS, 111 WRITES"), "{:#?}", log.iter().filter(|l| l.contains("TABLE 12")).collect::<Vec<_>>());
    assert_eq!(writes(&e).iter().filter(|w| w.0 == PHY_A_CR_ADDR || w.0 == PHY_A_CR_DATA).count(), 37);
    assert!(log.iter().any(|l| l == "S2 THE PHY WAS NOT SET UP (SCRATCH 0x0): THE LONG PATH: 37 WRITES THROUGH ITS BUS (17 INTO THE PHY)"), "{:#?}", log.iter().filter(|l| l.starts_with("S2 THE")).collect::<Vec<_>>());
}

#[test]
fn the_ryzens_own_tables_through_step_3() {
    let skip = |why: String| { use std::io::Write; let _ = writeln!(std::io::stderr(), "the_ryzens_own_tables_through_step_3 SKIPPED: {}", why); };
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { skip("MIND_AMD_VBIOS is not set (the Ryzen's VBIOS image, not in the repository)".into()); return };
    let Ok(bytes) = std::fs::read(&path) else { skip(format!("{} cannot be read", path)); return };
    // With no status answered, table 12's wait for SRAM_INIT_DONE ends by a limit and step 3 undoes stages 2 and 1.
    let mut e = engine();
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    match outcome {
        Outcome::Failed { stage: 2, why: Failure::Table(atom::Error::TimeLimit { table: 77, .. } | atom::Error::StepLimit { table: 77, .. }) } => {}
        o => panic!("{:?} {:#?}", o, &log[log.len().saturating_sub(20)..]),
    }
    for x in log.iter().filter(|x| x.starts_with("S2 TABLE") || x.starts_with("S2 FAILED") || (x.starts_with("U") && !x.contains(" WROTE ") && !x.contains(" RESTORED ")) || x.starts_with("STEP 3")) { eprintln!("{}", x); }
    // Table 12's power-down (pixel clock 0) runs to its end on the Ryzen's image, making no write on this model; every
    // write of stages 2 and 1 is restored (table 12 stops at the SRAM wait, before its control-register-bus writes).
    assert!(log.iter().any(|x| x.starts_with("U2 TABLE 12 DONE")), "the power-down did not end");
    restored_but(&e, &before, &[]);
    let (f, c) = EXPECTED;
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    for (w, _) in writes(&e) { assert!(l.guard().check(Reg::new(w).unwrap()).is_ok(), "{:05X}", w); }

    // With the four statuses answered (hwdocs, "What first light's VBIOS tables write"), all eight stages run on the
    // Ryzen's tables, every write on the route, the firmware's pipe unchanged.
    let mut e = answered();
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log[log.len().saturating_sub(20)..]);
    for (w, _) in writes(&e) { assert!(l.guard().check(Reg::new(w).unwrap()).is_ok(), "{:05X}", w); }
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    for t in [12, 4, 76] { assert!(log.iter().any(|x| x.contains(&format!("TABLE {} DONE", t))), "table {}", t); }
    // The frame count held still: stage 5 fails, and the undo restores every register stages 1 to 4 wrote but PHY A's
    // control register bus, which it leaves as it is (a write to its data register is a write into the PHY), saying how
    // many of table 12's writes went through it.
    let mut e = answered();
    e.frames_move = false;
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 5, why: Failure::NoFrames(0) });
    restored_but(&e, &before, &[PHY_A_CR_ADDR, PHY_A_CR_DATA]);
    assert!(log.iter().any(|x| x.starts_with("U2 PHY A'S CONTROL REGISTER BUS LEFT AS IT IS: 20 ADDRESS AND 17 DATA WRITES")), "{:#?}", log.iter().filter(|x| x.starts_with('U') && !x.contains(" RESTORED ")).collect::<Vec<_>>());
    let table_data = log.iter().filter(|x| x.starts_with("S2 T12 WROTE AT 177D4")).count();
    assert_eq!(writes(&e).iter().filter(|w| w.0 == PHY_A_CR_DATA).count(), table_data, "the undo wrote into PHY A");
    for x in log.iter().filter(|x| x.starts_with('U') && !x.contains(" RESTORED ") && !x.contains(" WROTE ")) { eprintln!("{}", x); }
}

// --- the production order (213-DRV-0098) ---

// The survey's log as display paces it on the hardware: 20 ms after each line (LINE_PACE_MS) and each pause its length,
// on the model's clock.
struct Paced { lines: Vec<String>, clock: Rc<Cell<u64>> }
impl Out for Paced {
    fn line(&mut self, a: fmt::Arguments) { self.lines.push(a.to_string()); self.clock.set(self.clock.get() + 20_000); }
    fn pause(&mut self, ms: u32) { self.clock.set(self.clock.get() + ms as u64 * 1000); }
}

// What the device saw, in order: every write, the offer and screen_gone.
fn device(e: &Engine) -> Vec<Ev> { e.ev.iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect() }

struct Ran { outcome: Outcome, times: light::Times, device: Vec<Ev>, log: Vec<String>, engine: Engine }

// Steps 3 and 4 on a fresh model in each order: the survey's through `Paced` (its lines and pauses on the clock), the
// production one through `batches::Held` over `Paced` (only what it logs is paced), as main.rs makes them.
fn both_orders(l: &Light, img: &[u8], make: &dyn Fn() -> Engine, allowed: u8, undo: bool) -> (Ran, Ran) {
    let go = |production: bool| {
        let mut e = make();
        let pace = Rc::new(Cell::new(0));
        e.pace = Some(pace.clone());
        let mut interp = atom::Interpreter::new(atom::Image::new(img).unwrap());
        let mut journal = light::Journal::default();
        let paced = Paced { lines: vec![], clock: pace };
        let (outcome, times, log) = if production {
            let mut buf = vec![0u8; batches::HELD_BYTES];
            let mut held = batches::Held::new(paced, &mut buf);
            let (o, t) = light::run_timed(l, &mut e, &mut interp, &mut journal, allowed, undo, &mut held);
            (o, t, held.inner.lines)
        } else {
            let mut paced = paced;
            let (o, t) = light::run_timed(l, &mut e, &mut interp, &mut journal, allowed, undo, &mut paced);
            (o, t, paced.lines)
        };
        Ran { outcome, times, device: device(&e), log, engine: e }
    };
    (go(false), go(true))
}

fn same_device(survey: &Ran, production: &Ran, case: &str) {
    assert_eq!(survey.outcome, production.outcome, "{}", case);
    assert_eq!(survey.device.len(), production.device.len(), "{}: {} and {} writes", case, survey.device.len(), production.device.len());
    for (i, (a, b)) in survey.device.iter().zip(&production.device).enumerate() { assert_eq!(a, b, "{}: event {}", case, i); }
}

// Both outputs' frames on the model, 60.2 Hz: a whole number of stage 9's polls (100 us and a 10 us read), so that its
// count of 30 frames, and with it pipe 1's DLG and sync, comes out the same whatever phase of a frame it starts in. On
// the hardware that count is a measure each run (268,495 kHz at boot 71, 268,522 at boot 81), so the values stages 13
// and 14 write from it can differ in their last digit between two boots in either order.
const DP2: u64 = 16_610;

// A line with its numbers masked: the times in a line differ between the orders, the words do not.
fn words(x: &str) -> String {
    let mut out = String::new();
    for ch in x.chars() { if ch.is_ascii_digit() { if !out.ends_with('#') { out.push('#'); } } else { out.push(ch); } }
    out
}

#[test]
fn the_production_order_makes_the_surveys_writes_in_the_same_order_at_1080p_and_in_the_edids_mode() {
    let (f, c) = EXPECTED;
    let modes = [("1920x1080 DVI", timing::CEA_1080P60, dcn::Signal::Dvi), ("1920x1080 HDMI", timing::CEA_1080P60, dcn::Signal::Hdmi),
                 ("the EDID's 3440x1440 HDMI", WIDE, dcn::Signal::Hdmi)];
    let paced = || { let mut e = engine(); e.frame_us = DP2; e };
    for (name, mode, signal) in modes {
        let l = Light::new_with(f, c, mode, signal).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
        // All sixteen stages, left in place and undone.
        for undo in [false, true] {
            let (s, p) = both_orders(&l, &good(), &paced, 16, undo);
            assert_eq!(s.outcome, if undo { Outcome::Undone { done: 16 } } else { Outcome::Stopped { done: 16 } }, "{} {:#?}", name, &s.log[s.log.len().saturating_sub(8)..]);
            same_device(&s, &p, &format!("{}, undo {}", name, undo));
            assert!(s.device.iter().filter(|x| matches!(x, Ev::Write(..))).count() > 100, "{}", s.device.len());
        }
        // Each build of fewer stages.
        for allowed in [1, 2, 5, 8, 9, 10, 13, 15] {
            let (s, p) = both_orders(&l, &good(), &paced, allowed, false);
            assert_eq!(s.outcome, Outcome::Stopped { done: allowed });
            same_device(&s, &p, &format!("{}, {} stages", name, allowed));
        }
    }
    // The survey's pauses and lines took the clock on by tens of seconds; the production order by under a second besides
    // stage 9's count and stage 15's watch.
    let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
    let (s, p) = both_orders(&l, &good(), &paced, 16, false);
    assert!(s.times.total_us > 60_000_000, "{:?}", s.times);
    assert!(p.times.total_us < p.times.count_us + p.times.watch_us + 1_000_000, "{:?}", p.times);
}

#[test]
fn the_production_order_fails_and_undoes_as_the_survey_does() {
    let (f, c) = EXPECTED;
    let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
    let cntl1 = reg(R::HubpCntl, 1);
    let cases: Vec<(&str, Box<dyn Fn() -> Engine>, Outcome)> = vec![
        ("stage 5's frames never move", Box::new(|| { let mut e = engine(); e.frames_move = false; e }), Outcome::Failed { stage: 5, why: Failure::NoFrames(0) }),
        ("a status that never clears: stage 6's generic packet conflict", Box::new(|| { let mut e = engine(); e.frame_us = DP2; e.set.push((reg(R::AfmtVbiPacketControl, 0), 1 << 16)); e }),
            Outcome::Failed { stage: 6, why: Failure::Timeout(0) }),
        ("HUBP1 underflowing once its surface is set", Box::new(move || { let mut e = engine(); e.frame_us = DP2; e.derived.push((cntl1, 1 << 28, reg(R::HubpreqSurfacePitch, 1), !0)); e }),
            Outcome::Failed { stage: 15, why: Failure::Underflow { offset: cntl1, value: 0 } }),
        ("a one-off underflow 17 frames into the watch", Box::new(move || { let mut e = engine(); e.frame_us = DP2; e.arm.push((reg(R::DpgControl, 1), 1, cntl1, 1 << 28, 17 * DP2 + 8_000)); e }),
            Outcome::Failed { stage: 15, why: Failure::Underflow { offset: cntl1, value: 0 } }),
        ("the compositor refusing the offer", Box::new(|| { let mut e = engine(); e.frame_us = DP2; e.refuse_offer = Some(ScreenRefused::Denied); e }),
            Outcome::NotReady { done: 15, stage: 16, why: Failure::Screen(ScreenRefused::Denied) }),
        ("the firmware's pipe changed after stage 7", Box::new(|| { let mut e = engine(); e.frame_us = DP2; e.hook = Some((reg(R::DigBeCntl, 0), reg(R::OtgHTotal, 0), 0x777)); e }),
            Outcome::Failed { stage: 7, why: Failure::FirmwareChanged { offset: reg(R::OtgHTotal, 0), before: 0, after: 0 } }),
    ];
    for (name, make, want) in &cases {
        for undo in [false, true] {
            let (s, p) = both_orders(&l, &good(), &**make, 16, undo);
            let kind = |o: &Outcome| match o { Outcome::Failed { stage, why } => (0, *stage, core::mem::discriminant(why)), Outcome::NotReady { stage, why, .. } => (1, *stage, core::mem::discriminant(why)),
                                                Outcome::Stopped { done } | Outcome::Undone { done } => (2, *done, core::mem::discriminant(&Failure::JournalFull)) };
            if !matches!(want, Outcome::Stopped { .. }) { assert_eq!(kind(&s.outcome), kind(want), "{}: {:?}", name, s.outcome); }
            same_device(&s, &p, &format!("{}, undo {}", name, undo));
        }
    }
    // A stage without its reference: stage 9 not ready, first light in place, in either order.
    let l9 = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap();
    let (s, p) = both_orders(&l9, &good(), &|| { let mut e = engine(); e.frame_us = DP2; e }, 16, false);
    assert_eq!(s.outcome, Outcome::NotReady { done: 8, stage: 9, why: Failure::NoReference });
    same_device(&s, &p, "no reference");
}

#[test]
fn the_production_orders_log_is_a_line_a_stage_with_its_time_and_the_detail_only_of_a_part_that_fails() {
    let (f, c) = EXPECTED;
    let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
    let paced = || { let mut e = engine(); e.frame_us = DP2; e };
    let (s, p) = both_orders(&l, &good(), &paced, 16, false);
    // The pipes and the mode, a line a stage, step 3's times and its end: 21 lines, none of a write or a plan.
    for x in &p.log { eprintln!("production, 3440x1440 on the model: {}", x); }
    assert_eq!(p.log.len(), 3 + 16 + 2, "{:#?}", p.log);
    assert!(p.log[0].starts_with("STEP 3: FIRST LIGHT ON HDMI-A-1 IN 8 STAGES") && p.log[2].ends_with("MODE 3440x1440 AT 324 MHZ IN HDMI SIGNALLING (CTA VIC 0)"), "{:#?}", &p.log[..3]);
    for n in 1..=16u8 {
        let line = &p.log[2 + n as usize];
        assert!(line.starts_with(&format!("S{} {} DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS); ", n, dcn::STAGE_NAMES[n as usize - 1])) && line.contains(" MS"), "{}", line);
        assert!(line.len() <= 180, "{} bytes: {}", line.len(), line);
        // The same line ends the stage in the survey's log, its time the survey's own.
        let head = line.split("); ").next().unwrap();
        assert!(s.log.iter().any(|x| x.starts_with(head)), "{}", head);
    }
    assert!(!p.log.iter().any(|x| x.contains(" WROTE ") || x.contains(" WILL ") || x.contains("RUNNING TABLE")));
    let line = |n: u8| p.log[2 + n as usize].as_str();
    assert!(line(2).contains(", 1 WRITES, TABLE 12 ") && line(2).contains("(3 STEPS, 2 WRITES), THE PHY SET UP ALREADY: THE SHORT PATH"), "{}", line(2));
    assert!(line(5).contains(", 3 WRITES, 2 WAITS "), "{}", line(5));
    assert!(line(9).contains(", OTG0 AT 26") && line(9).ends_with("KHZ, THE REFERENCE 48000 KHZ"), "{}", line(9));
    assert!(line(10).ends_with(", 3440x1440 DRAWN AND READ BACK"), "{}", line(10));
    assert!(line(13).ends_with(", THE PITCH READ BACK AS WRITTEN"), "{}", line(13));
    assert!(line(15).contains(", WATCHED 300 FRAMES IN ") && line(15).contains(" MS CLEAN, LONGEST GAP ") && line(15).contains(" US AT FRAME "), "{}", line(15));
    // The gap and its frame as the survey's detail gives them.
    let gap = number_after(&s.log, "S15 WATCHED 300 FRAMES", "THE LONGEST GAP BETWEEN READS ");
    let at = number_after(&s.log, "S15 WATCHED 300 FRAMES", "US, BEFORE FRAME ");
    let (pgap, pat) = (number_after(&p.log, "S15 UNBLANK DONE", "LONGEST GAP "), number_after(&p.log, "S15 UNBLANK DONE", "US AT FRAME "));
    assert!(pat >= 1 && pat <= 300 && at >= 1 && at <= 300 && pgap > 0 && gap > 0, "{} {} {} {}", gap, at, pgap, pat);
    assert!(line(16).ends_with(", OFFERED 3440x1440 PITCH 13824"), "{}", line(16));
    assert!(p.log[19].starts_with("STEP 3'S TIMES (MS): IN ALL ") && p.log[20].starts_with("STEP 3 STOPPED AFTER STAGE 16 OF 16 AS THIS BUILD ALLOWS"), "{:#?}", &p.log[19..]);
    // The times: the watch 300 of HDMI-A-1's frames (from a read inside the first), stage 9's count 30 of DP-2's from
    // an edge, the offer after both.
    let t = p.times;
    assert!((299 * DP2..301 * DP2).contains(&t.watch_us), "{:?}", t);
    assert!((30 * DP2..32 * DP2).contains(&t.count_us), "{:?}", t);
    assert!(t.stages_us[14] >= t.watch_us && t.stages_us[8] >= t.count_us && t.stages_us.iter().sum::<u64>() <= t.total_us);
    assert!(t.offered_us.is_some_and(|o| o >= t.stages_us[..15].iter().sum::<u64>() && o <= t.total_us), "{:?}", t);
    // A stage that fails: its detail as the survey logs it, before its failure; none of the stages before it.
    let cntl1 = reg(R::HubpCntl, 1);
    let underflow = || { let mut e = engine(); e.frame_us = DP2; e.derived.push((cntl1, 1 << 28, reg(R::HubpreqSurfacePitch, 1), !0)); e };
    let (s, p) = both_orders(&l, &good(), &underflow, 16, false);
    let failed = p.log.iter().position(|x| x.starts_with("S15 UNBLANK FAILED: AN UNDERFLOW AT 0EE3C")).expect("the failure");
    let begin = p.log.iter().position(|x| x == "S15 UNBLANK: STAGE 15 OF 16 NEXT; ITS WRITES:").expect("stage 15's detail");
    let survey_begin = s.log.iter().position(|x| x == "S15 UNBLANK: STAGE 15 OF 16 NEXT; ITS WRITES:").unwrap();
    let survey_failed = s.log.iter().position(|x| x.starts_with("S15 UNBLANK FAILED: AN UNDERFLOW AT 0EE3C")).unwrap();
    let masked = |l: &[String]| l.iter().map(|x| words(x)).collect::<Vec<_>>();
    assert!(begin < failed);
    assert_eq!(masked(&p.log[begin..failed]), masked(&s.log[survey_begin..survey_failed]), "stage 15's detail, as the survey's but for its times");
    assert!(p.log[begin..failed].iter().any(|x| x.starts_with("S15 WROTE HUBP1_DCHUBP_CNTL AT 0EE3C = ")));
    assert!(p.log[begin..failed].iter().any(|x| x.starts_with("U15 RESTORED HUBP1_DCHUBP_CNTL AT 0EE3C = ") && x.ends_with(" AT ONCE")));
    assert!(!p.log[..begin].iter().any(|x| x.contains(" WROTE ")), "{:#?}", &p.log[..begin]);
    // The undo after it in full, as the survey's.
    assert_eq!(masked(&p.log[failed..]), masked(&s.log[survey_failed..]));
    assert!(p.log.last().unwrap().starts_with("STEP 3 FAILED AT STAGE 15 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED"));
    // A stage not ready: its detail, then the stop.
    let refused = || { let mut e = engine(); e.frame_us = DP2; e.refuse_offer = Some(ScreenRefused::Unavailable); e };
    let (_, p) = both_orders(&l, &good(), &refused, 16, false);
    let not_ready = p.log.iter().position(|x| x.starts_with("S16 OFFER NOT READY: THE COMPOSITOR DID NOT TAKE THE SECOND SCREEN")).unwrap();
    assert!(p.log[not_ready - 1].starts_with("S16 UNDO: "), "{:#?}", &p.log[not_ready - 3..]);
    // Every line fits a log record.
    for x in &p.log { assert!(x.len() <= 180, "{} bytes: {}", x.len(), x); }
}

#[test]
fn the_production_orders_held_detail_keeps_the_latest_lines_of_a_part() {
    // A buffer for a few lines: the oldest let go and counted, the rest logged in order when the part fails.
    let mut buf = [0u8; 64];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    held.note(format_args!("A NOTE"));
    for i in 0..10 { held.line(format_args!("DETAIL {:02}", i)); }
    assert_eq!(held.inner.0, ["A NOTE"]);
    held.failed();
    held.line(format_args!("AFTER"));
    let kept: Vec<&str> = held.inner.0.iter().map(|x| x.as_str()).collect();
    assert_eq!(kept, ["A NOTE", "4 EARLIER LINES OF THIS PART'S DETAIL WERE NOT KEPT", "DETAIL 04", "DETAIL 05", "DETAIL 06", "DETAIL 07", "DETAIL 08", "DETAIL 09", "AFTER"]);
    // A new part: held again, nothing logged; its pauses are none.
    held.part();
    held.line(format_args!("NEXT"));
    held.pause(1000);
    assert_eq!(held.inner.0.len(), 9);
    // A line longer than a record is cut at 255 bytes.
    let mut big = vec![0u8; 4096];
    let mut held = batches::Held::new(Log(vec![]), &mut big);
    held.line(format_args!("{}", "X".repeat(300)));
    held.failed();
    assert_eq!(held.inner.0[0].len(), 255);
}

#[test]
fn the_ryzens_tables_make_the_same_writes_in_either_order() {
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("the_ryzens_tables_make_the_same_writes_in_either_order SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    let (f, c) = EXPECTED;
    // On the warm state the Ryzen showed (the short path) and from a cold PHY A (the long one), in each mode.
    let warm16 = || { let mut e = warm(); e.frame_us = DP2; e };
    let cold16 = || { let mut e = answered(); e.frame_us = DP2; e };
    for (name, mode, signal) in [("1080p DVI", timing::CEA_1080P60, dcn::Signal::Dvi), ("1080p HDMI", timing::CEA_1080P60, dcn::Signal::Hdmi), ("3440x1440 HDMI", WIDE, dcn::Signal::Hdmi)] {
        let l = Light::new_with(f, c, mode, signal).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD");
        for (state, make) in [("warm", &warm16 as &dyn Fn() -> Engine), ("cold", &cold16)] {
            for undo in [false, true] {
                let (s, p) = both_orders(&l, &bytes, make, 16, undo);
                assert_eq!(s.outcome, if undo { Outcome::Undone { done: 16 } } else { Outcome::Stopped { done: 16 } }, "{} {} {:#?}", name, state, &s.log[s.log.len().saturating_sub(8)..]);
                same_device(&s, &p, &format!("{} {}, undo {}", name, state, undo));
                let path = if state == "warm" { "THE PHY SET UP ALREADY: THE SHORT PATH" } else { "THE LONG PATH: 37 WRITES THROUGH THE PHY'S BUS" };
                assert!(p.log.iter().any(|x| x.starts_with("S2 PIXEL CLOCK DONE") && x.ends_with(path)), "{} {}: {:#?}", name, state, p.log.iter().find(|x| x.starts_with("S2 ")));
            }
        }
    }
}

// --- the second output plugged and unplugged (213-DRV-0096) ---

// Every register written is as before step 3, the mark of stages in place too, but for the status bits the model sets.
fn as_before(e: &Engine, before: &HashMap<u32, u32>, except: &[u32]) {
    for (o, _) in writes(e) {
        if except.contains(&o) { continue; }
        let status = e.set.iter().chain(e.follow.iter()).filter(|(at, _)| *at == o).fold(0, |m, (_, bits)| m | bits);
        let value = |m: &HashMap<u32, u32>| m.get(&o).copied().unwrap_or(0) & !status;
        assert_eq!(value(&e.regs), value(before), "{:05X} not as before step 3", o);
    }
}

// Steps 3 and 4 on the model kept for an unplug: the plan, the interpreter and the journal.
struct Lit<'a> { l: Light, interp: atom::Interpreter<'a>, journal: Box<light::Journal> }
fn light_up<'a>(l: Light, img: &'a [u8], e: &mut Engine, allowed: u8) -> (Outcome, Lit<'a>, Vec<String>) {
    let mut lit = Lit { l, interp: atom::Interpreter::new(atom::Image::new(img).unwrap()), journal: Box::new(light::Journal::default()) };
    let mut log = Log(vec![]);
    let (o, _) = light::run_timed(&lit.l, e, &mut lit.interp, &mut lit.journal, allowed, false, &mut log);
    (o, lit, log.0)
}
fn take_back(lit: &mut Lit, e: &mut Engine, done: u8) -> (bool, Vec<String>) {
    let mut log = Log(vec![]);
    let same = light::take_back(&lit.l, e, &mut lit.interp, &mut lit.journal, done, &mut log);
    (same, log.0)
}

fn wide() -> Light {
    let (f, c) = EXPECTED;
    Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap().with_reference(Some(boot70_reference())).with_build("TESTBUILD")
}

#[test]
fn an_unplug_after_stage_16_lets_the_screen_go_first_then_puts_every_register_back_the_mark_too() {
    let img = good();
    let mut e = engine();
    e.frame_us = DP2;
    let before = e.regs.clone();
    let (outcome, mut lit, _) = light_up(wide(), &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let first = device(&e);
    let made = e.ev.len();
    let (same, log) = take_back(&mut lit, &mut e, 16);
    assert!(same, "{:#?}", log);
    // screen_gone before anything else the device sees, once; then stages 15 to 1 undone.
    let after: Vec<Ev> = e.ev[made..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(after.first(), Some(&Ev::Gone));
    assert_eq!(after.iter().filter(|x| **x == Ev::Gone).count(), 1);
    assert!(log.iter().any(|x| x == "U16 SCREEN_GONE: THE COMPOSITOR HAS LET THE SURFACE GO"));
    let undone: Vec<u8> = log.iter().filter_map(|x| x.strip_prefix('U').and_then(|r| r.split(' ').next()).filter(|_| x.contains(" UNDONE: ")).and_then(|n| n.parse().ok())).collect();
    assert_eq!(undone, (1..=16).rev().collect::<Vec<u8>>());
    // Every register as before step 3, the mark cleared to its value before; the journal empty for the next run.
    as_before(&e, &before, &[]);
    assert_eq!(e.regs[&reg(R::OtgVerticalInterrupt2Position, 1)], 0);
    assert!(lit.journal.is_empty());
    assert!(log.iter().any(|x| x == "U0 RESTORED OTG1_OTG_VERTICAL_INTERRUPT2_POSITION AT 14298 = 00000000: THE MARK OF STAGES IN PLACE CLEARED"), "{:#?}", log);
    assert!(log[0].starts_with("STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR; UNDOING STAGES 16 TO 1 ("));
    assert!(log.last().unwrap().starts_with("STEP 3 TAKEN BACK IN ") && log.last().unwrap().ends_with(" MS: THE SECOND PIPE IS OFF, ITS MARK CLEARED; THE FIRMWARE'S PIPE UNCHANGED"), "{}", log.last().unwrap());
    // A monitor plugged in again: the stages run once more on the same hardware, the same writes and the same offer as the
    // first time (the model's frame a whole number of stage 9's polls, so its count is the same too).
    let from = e.ev.len();
    let mut log = Log(vec![]);
    let (outcome, _) = light::run_timed(&lit.l, &mut e, &mut lit.interp, &mut lit.journal, 16, false, &mut log);
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log.0[log.0.len().saturating_sub(6)..]);
    let again: Vec<Ev> = e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(again, first);
    // And unplugged again: as before step 3 once more.
    assert!(take_back(&mut lit, &mut e, 16).0);
    as_before(&e, &before, &[]);
}

#[test]
fn a_start_of_display_again_while_lit_runs_no_stage_and_after_an_unplug_runs_them_all() {
    let img = good();
    let mut e = engine();
    e.frame_us = DP2;
    let (_, mut lit, _) = light_up(wide(), &img, &mut e, 16);
    // display started again (init restarts a program that faults) while the stages are in place: no stage, no write.
    let made = writes(&e).len();
    let (outcome, _, _) = light_up(wide(), &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Failed { stage: 0, why: Failure::StartedAgain });
    assert_eq!(writes(&e).len(), made);
    // After an unplug took them back, a start runs them all.
    take_back(&mut lit, &mut e, 16);
    let (outcome, _, _) = light_up(wide(), &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
}

#[test]
fn an_unplug_after_fewer_stages_takes_back_those_that_ran_and_lets_no_screen_go() {
    let img = good();
    // A build of 8 (first light), and stage 16 refused by the compositor (stages 1 to 15 in place).
    for (allowed, refuse, done) in [(8u8, None, 8u8), (16, Some(ScreenRefused::Unavailable), 15)] {
        let mut e = engine();
        e.frame_us = DP2;
        e.refuse_offer = refuse;
        let before = e.regs.clone();
        let (outcome, mut lit, _) = light_up(wide(), &img, &mut e, allowed);
        assert_eq!(hpd::state_after(Some(outcome)), hpd::State::Lit, "{:?}", outcome);
        let (same, log) = take_back(&mut lit, &mut e, done);
        assert!(same, "{:#?}", log);
        assert!(!e.ev.contains(&Ev::Gone), "screen_gone without an offer taken");
        as_before(&e, &before, &[]);
    }
}

#[test]
fn the_ryzens_tables_taken_back_for_an_unplug_and_run_again() {
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { eprintln!("the_ryzens_tables_taken_back_for_an_unplug_and_run_again SKIPPED: MIND_AMD_VBIOS is not set"); return };
    let bytes = std::fs::read(&path).unwrap();
    // Warm (the short path): taken back whole, the off calls run to their end, then the same writes again.
    let mut e = warm();
    e.frame_us = DP2;
    let before = e.regs.clone();
    let (outcome, mut lit, _) = light_up(wide(), &bytes, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let first = device(&e);
    let made = e.ev.len();
    let (same, log) = take_back(&mut lit, &mut e, 16);
    assert!(same);
    for t in ["U2 TABLE 12 DONE", "U8 TABLE 76 DONE"] { assert!(log.iter().any(|x| x.starts_with(t)), "{}: {:#?}", t, log.iter().filter(|x| x.contains("TABLE")).collect::<Vec<_>>()); }
    as_before(&e, &before, &[]);
    let from = e.ev.len();
    let mut out = Log(vec![]);
    let (outcome, _) = light::run_timed(&lit.l, &mut e, &mut lit.interp, &mut lit.journal, 16, false, &mut out);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let again: Vec<Ev> = e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(again, first);
    assert!(e.ev[made..from].contains(&Ev::Gone));
    // Cold (the long path): taken back but PHY A's control register bus, which the undo leaves by design.
    let mut e = answered();
    e.frame_us = DP2;
    let before = e.regs.clone();
    let (outcome, mut lit, _) = light_up(wide(), &bytes, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    assert!(take_back(&mut lit, &mut e, 16).0);
    as_before(&e, &before, &[PHY_A_CR_ADDR, PHY_A_CR_DATA]);
}

// Table 76 waits for a PHY A status the model shows while the stages run: its DISABLE stops by the time limit once it is gone.
fn waiting_76() -> Vec<u8> {
    tables([mov(seg(1, 0xA0), 1), mov(PHY_A_CNTL0, 0x0020_0001), vec![EOT]].concat(), [mov(seg(2, 0x20B0), 1), wait_bit(PHY_A_CNTL0, 28, 8), vec![EOT]].concat())
}

#[test]
fn an_unplug_whose_undo_goes_wrong_logs_the_held_detail_in_the_production_order_and_one_that_goes_right_two_lines() {
    let (good, waiting) = (good(), waiting_76());
    // What goes wrong: nothing; stage 8's off call stopping; screen_gone not taken; the firmware's pipe changed by the hardware.
    for case in 0..4 {
        let mut e = engine();
        e.frame_us = DP2;
        e.set.push((PHY_A_CNTL0, 1 << 28));
        let before = e.regs.clone();
        let (outcome, mut lit, _) = light_up(wide(), if case == 1 { &waiting } else { &good }, &mut e, 16);
        assert_eq!(outcome, Outcome::Stopped { done: 16 }, "case {}", case);
        e.set.clear();
        if case == 2 { e.refuse_gone = Some(ScreenRefused::NoScreen); }
        let firmwares = watched(&lit.l)[0];
        if case == 3 { e.hook = Some((seg(2, 0x20B0), firmwares, 0xBAD)); }
        let mut buf = vec![0u8; batches::HELD_BYTES];
        let mut held = batches::Held::new(Log(vec![]), &mut buf);
        let same = light::take_back(&lit.l, &mut e, &mut lit.interp, &mut lit.journal, 16, &mut held);
        let log = held.inner.0;
        let last = log.last().unwrap().clone();
        assert!(log[0].starts_with("STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR; UNDOING STAGES 16 TO 1 ("), "case {}: {}", case, log[0]);
        assert!(last.starts_with("STEP 3 TAKEN BACK IN "), "case {}: {}", case, last);
        let at = |p: &str| log.iter().position(|x| x.starts_with(p));
        match case {
            // Gone right: the two notes only, as on boot 94.
            0 => { assert!(same); assert_eq!(log.len(), 2, "{:#?}", log); assert!(last.ends_with(" MS: THE SECOND PIPE IS OFF, ITS MARK CLEARED; THE FIRMWARE'S PIPE UNCHANGED"), "{}", last); }
            // A stage's undo gone wrong: the detail held before it, its own lines, every line after it, then the last line saying so.
            1 | 2 => {
                assert!(same, "{:#?}", log);
                let (wrong, first) = if case == 1 { ("U8 TABLE 76 STOPPED (", "U16 SCREEN_GONE: THE COMPOSITOR HAS LET THE SURFACE GO") } else { ("U16 SCREEN_GONE NOT TAKEN: NO SCREEN; THE UNDO GOES ON", "U16 ") };
                let w = at(wrong).unwrap_or_else(|| panic!("case {}: {:#?}", case, log));
                assert!(at(first).unwrap() <= w);
                assert!(log[w..].iter().any(|x| x.starts_with("U1 OTG CLOCKS UNDONE")), "case {}: {:#?}", case, &log[w..]);
                assert!(at("U0 RESTORED OTG1_OTG_VERTICAL_INTERRUPT2_POSITION").unwrap() > w);
                assert!(last.ends_with("THE FIRMWARE'S PIPE UNCHANGED; 1 STAGES' UNDOS WENT WRONG, THEIR DETAIL ABOVE"), "case {}: {}", case, last);
                // Every journaled write put back all the same (PHY A's status bit, read into the journal, aside).
                as_before(&e, &before, &[PHY_A_CNTL0]);
                assert_eq!(e.regs[&PHY_A_CNTL0] & !(1 << 28), before.get(&PHY_A_CNTL0).copied().unwrap_or(0));
            }
            // The firmware's pipe changed: the whole take-back's detail logged before the last line.
            _ => {
                assert!(!same);
                assert!(at("U16 SCREEN_GONE: THE COMPOSITOR HAS LET THE SURFACE GO").is_some() && at("U1 OTG CLOCKS UNDONE").is_some(), "{:#?}", log);
                assert!(last.ends_with("THE FIRMWARE'S PIPE CHANGED"), "{}", last);
            }
        }
    }
}

#[test]
fn the_builds_own_undo_after_the_stages_logs_an_off_call_that_stops_in_the_production_order() {
    let img = waiting_76();
    let mut e = engine();
    e.frame_us = DP2;
    // The status shown for the first 300 ms: past stage 8, which ends within stage 5's two frames and some tables' steps,
    // and gone before the build's undo, which comes after stage 9's count of 30 frames and stage 15's watch.
    e.window.push((PHY_A_CNTL0, 1 << 28, 0, 300_000));
    let before = e.regs.clone();
    let mut interp = atom::Interpreter::new(atom::Image::new(&img).unwrap());
    let mut journal = Box::new(light::Journal::default());
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    let (outcome, _) = light::run_timed(&wide(), &mut e, &mut interp, &mut journal, 16, true, &mut held);
    let log = held.inner.0;
    assert_eq!(outcome, Outcome::Undone { done: 16 }, "{:#?}", &log[log.len().saturating_sub(8)..]);
    let w = log.iter().position(|x| x.starts_with("U8 TABLE 76 STOPPED (")).unwrap_or_else(|| panic!("{:#?}", log));
    assert!(log[..w].iter().any(|x| x.starts_with("STEP 3 RAN 16 OF 16 STAGES (")), "the held detail before it");
    assert!(log[w..].iter().any(|x| x.starts_with("U1 OTG CLOCKS UNDONE")));
    assert!(log.last().unwrap().ends_with("THE FIRMWARE'S PIPE UNCHANGED; 1 STAGES' UNDOS WENT WRONG, THEIR DETAIL ABOVE"), "{}", log.last().unwrap());
    // Every journaled write put back but the mark, which this undo keeps (PHY A's status bit, read into the journal, aside).
    as_before(&e, &before, &[PHY_A_CNTL0, reg(R::OtgVerticalInterrupt2Position, 1)]);
    assert_eq!(e.regs[&PHY_A_CNTL0] & !(1 << 28), before.get(&PHY_A_CNTL0).copied().unwrap_or(0));
}

// The model as step 1 finds the Ryzen: everything steps 3 and 4 take from it (pipe 0's reference as boot 70 read it),
// HPD 1 sensing HDMI-A-1's monitor and enabled.
fn plugged() -> Engine {
    let mut e = engine();
    e.frame_us = DP2;
    let mut put = |r: R, i: u8, v: u32| { e.regs.insert(reg(r, i), v); };
    put(R::HpdIntStatus, 0, 0b10);
    put(R::HpdControl, 0, 1 << 28);
    put(R::OptcDataSourceSelect, 0, 0);
    put(R::HubpCntl, 0, 0x000F_0002);
    for n in 1..4u8 { put(R::HubpCntl, n, 0x000F_000B | (n as u32) << 4); }
    for d in 0..4u8 { put(R::DigFeCntl, d, 7); }
    put(R::DigFeCntl, 4, 1 << 24);
    put(R::DigBeCntl, 4, (1 << 4) << 8);
    put(R::DigBeEnCntl, 4, 0x101);
    put(R::DmcubCntl, 0, 1 << 17);
    for (r, v) in [(R::OtgHTotal, 2719), (R::OtgVTotal, 1645), (R::OtgHBlankStartEnd, 112 << 16 | 2672), (R::OtgVBlankStartEnd, 43 << 16 | 1643),
        (R::HubpPriViewportDimension, 1600 << 16 | 2560), (R::HubpreqRefFreqToPixFreq, 0x14E5E), (R::HubpreqDstDimensions, 0x1E641),
        (R::HubpreqBlankOffset0, 0x13 << 16 | 0x35), (R::HubpreqSurf0TtuCntl0, 8 << 24), (R::HubpreqSurfacePitch, 2560), (R::DcnVmFbLocationBase, 0xF400),
        (R::OtgVstartupParam, 0xD), (R::OtgVupdateParam, 0x0140_02A8), (R::OtgVreadyParam, 0x12C), (R::DchubbubGlobalTimerCntl, 0x1001)] {
        put(r, 0, v);
    }
    for n in 0..4u8 { put(R::MpccTopSel, n, n as u32); put(R::MpccBotSel, n, 0xF); put(R::MpccOppId, n, n as u32); put(R::MpcOutMux, n, n as u32); }
    e
}

// Steps 1 to 4 as display runs them, in the production order: step 1's values, the plan from them, the stages.
fn start_on<'a>(e: &mut Engine, img: &'a [u8]) -> (Outcome, Lit<'a>) {
    let mut store = Store::default();
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    batches::read_into(e, batches::BATCHES, batches::Order::Production, &mut held, &mut store);
    let l = light::prepare(&store, WIDE, dcn::Signal::Hdmi, "TESTBUILD").expect("step 3 refused");
    assert!(l.reference.is_some());
    let mut lit = Lit { l, interp: atom::Interpreter::new(atom::Image::new(img).unwrap()), journal: Box::new(light::Journal::default()) };
    let (o, _) = light::run_timed(&lit.l, e, &mut lit.interp, &mut lit.journal, 16, false, &mut held);
    (o, lit)
}

// HPD 1 sensing `sense` for `ms`, display polling it every POLL_MS on the model's clock and doing what it asks.
fn poll_hpd<'a>(e: &mut Engine, f: &mut hpd::Follow, lit: &mut Option<Lit<'a>>, img: &'a [u8], sense: bool, ms: u64) -> Vec<hpd::Action> {
    let hpd1 = reg(R::HpdIntStatus, 0);
    e.regs.insert(hpd1, (sense as u32) << 1);
    let mut acted = vec![];
    let end = e.clock + ms * 1000;
    while e.clock < end {
        e.clock += hpd::POLL_MS as u64 * 1000;
        let now = e.clock;
        let sense = Bus::read(e, Reg::new(hpd1).unwrap()).unwrap() & 2 != 0;
        match f.feed(sense, now) {
            hpd::Action::TakeBack => { assert!(take_back(lit.as_mut().unwrap(), e, 16).0); f.set(hpd::State::Dark); acted.push(hpd::Action::TakeBack); }
            hpd::Action::LightUp => { let (o, l) = start_on(e, img); f.set(hpd::state_after(Some(o))); *lit = Some(l); acted.push(hpd::Action::LightUp); }
            hpd::Action::None => {}
        }
    }
    acted
}

#[test]
fn hpd_followed_on_the_model_an_unplug_takes_back_a_plug_runs_steps_1_to_4_again_a_bounce_neither() {
    let img = good();
    let mut e = plugged();
    let before = e.regs.clone();
    let (outcome, lit) = start_on(&mut e, &img);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let first = device(&e);
    let mut lit = Some(lit);
    let mut f = hpd::Follow::new(hpd::state_after(Some(outcome)), true);
    // A bounce of 300 ms: neither; the screen stays offered.
    assert!(poll_hpd(&mut e, &mut f, &mut lit, &img, false, 300).is_empty());
    assert!(poll_hpd(&mut e, &mut f, &mut lit, &img, true, 1000).is_empty());
    assert!(!e.ev.contains(&Ev::Gone));
    // Unplugged for two seconds: taken back once, screen_gone first, every register as before.
    assert_eq!(poll_hpd(&mut e, &mut f, &mut lit, &img, false, 2000), [hpd::Action::TakeBack]);
    assert_eq!(e.ev.iter().filter(|x| **x == Ev::Gone).count(), 1);
    as_before(&e, &before, &[]);
    // A bounce while dark: nothing lights it.
    let quiet = e.ev.len();
    assert!(poll_hpd(&mut e, &mut f, &mut lit, &img, true, 300).is_empty());
    assert!(poll_hpd(&mut e, &mut f, &mut lit, &img, false, 1000).is_empty());
    assert_eq!(e.ev.len(), quiet);
    // Plugged in for eight seconds: steps 1 to 4 run again, the same writes and offer as at start.
    let from = e.ev.len();
    assert_eq!(poll_hpd(&mut e, &mut f, &mut lit, &img, true, 8000), [hpd::Action::LightUp]);
    let again: Vec<Ev> = e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(again, first);
    assert_eq!(f.state(), hpd::State::Lit);
}

// --- the standing check (213-DRV-0103) ---

// All sixteen stages at 3440x1440 on `e`, with a watch of `frames`: the run kept for the check, and the clock at the offer.
fn lit_on<'a>(e: &mut Engine, img: &'a [u8], frames: u32) -> (Lit<'a>, u64) {
    let (outcome, lit, log) = light_up(wide().with_watch(frames), img, e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log[log.len().saturating_sub(6)..]);
    let offered = e.clock;
    (lit, offered)
}

// The check once a second for `seconds` (display's loop, on the model's clock): the lines it logged, and the second
// and failure of the first finding, which it then undoes as display does.
fn standing_for(e: &mut Engine, lit: &mut Lit, offered: u64, seconds: u64) -> (Vec<String>, Option<(u64, Failure)>) {
    let mut log = Log(vec![]);
    let mut account = light::Standing::new(e.clock);
    for second in 1..=seconds {
        e.clock += light::STANDING_US;
        if let Err(why) = light::standing_check(&lit.l, e, &lit.journal, &mut account, &mut log) {
            let since = e.clock - offered;
            assert!(light::standing_undo(&lit.l, e, &mut lit.interp, &mut lit.journal, 16, why, since, &mut log));
            return (log.0, Some((second, why)));
        }
    }
    (log.0, None)
}

#[test]
fn the_standing_check_finds_nothing_for_an_hour_says_so_every_ten_minutes_and_writes_nothing() {
    let img = good();
    let mut e = engine();
    e.frame_us = DP2;
    let (mut lit, offered) = lit_on(&mut e, &img, light::WATCH_FRAMES);
    let made = writes(&e).len();
    let (log, found) = standing_for(&mut e, &mut lit, offered, 3600);
    assert_eq!(found, None);
    assert_eq!(writes(&e).len(), made, "the check wrote");
    assert!(!e.ev.contains(&Ev::Gone));
    assert_eq!(log, vec!["S15 STANDING CHECK: 600 CHECKS IN 10 MIN, THE LONGEST 1000 MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE".to_string(); 6]);
    // A check kept from running for three seconds shows in the next line's longest time.
    let mut account = light::Standing::new(e.clock);
    let mut out = Log(vec![]);
    for k in 1..=600u64 {
        e.clock += if k == 300 { 3 * light::STANDING_US } else { light::STANDING_US };
        light::standing_check(&lit.l, &mut e, &lit.journal, &mut account, &mut out).unwrap();
    }
    // 598 checks fill the ten minutes with the three seconds lost.
    assert_eq!(out.0, ["S15 STANDING CHECK: 598 CHECKS IN 10 MIN, THE LONGEST 3000 MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE"]);
}

#[test]
fn an_underflow_10_s_after_the_offer_is_caught_within_a_second_and_undone_as_the_watch_undoes_the_mark_kept() {
    let img = good();
    let (cntl1, dpg1, mark) = (reg(R::HubpCntl, 1), reg(R::DpgControl, 1), reg(R::OtgVerticalInterrupt2Position, 1));
    let mut e = engine();
    e.frame_us = DP2;
    e.regs.insert(cntl1, 0x000F_001B); // HUBP1 blanked, as step 1 read it on the Ryzen
    let before = e.regs.clone();
    let (mut lit, offered) = lit_on(&mut e, &img, light::WATCH_FRAMES);
    // HUBP1's underflow status latched 10.4 s after the offer, as the hardware latches it until its clear bit.
    e.timed.push((cntl1, 1 << 28, offered + 10_400_000));
    let made = e.ev.len();
    let (log, found) = standing_for(&mut e, &mut lit, offered, 60);
    let (second, why) = found.expect("not caught");
    assert_eq!(second, 11, "caught at the first check after it");
    assert!(matches!(why, Failure::Underflow { offset, .. } if offset == cntl1), "{:?}", why);
    // The watch's own undo: the pattern on and HUBP1 blanked first, then screen_gone, then 14 to 1.
    let after: Vec<Ev> = e.ev[made..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Gone)).cloned().collect();
    assert!(matches!(after[0], Ev::Write(o, v) if o == dpg1 && v & 1 == 1), "{:?}", &after[..3]);
    assert!(matches!(after[1], Ev::Write(o, v) if o == cntl1 && v & 1 == 1), "{:?}", &after[..3]);
    assert_eq!(after[2], Ev::Gone);
    let line = log.iter().find(|x| x.starts_with("S15 STANDING CHECK: AN UNDERFLOW AT 0EE3C")).expect("its line");
    assert!(line.contains(" 11 S AFTER THE OFFER; STAGE 15'S WRITES WERE PUT BACK AT ONCE, BEFORE THIS LINE; UNDOING 16 (SCREEN_GONE) AND STAGES 14 TO 1"), "{}", line);
    assert_eq!(log.last().unwrap(), "STEP 3 UNDONE AFTER THE STANDING CHECK; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED; THE MARK KEPT: NOT TRIED AGAIN THIS BOOT");
    let undone: Vec<u8> = log.iter().filter_map(|x| x.strip_prefix('U').and_then(|r| r.split(' ').next()).filter(|_| x.contains(" UNDONE: ")).and_then(|n| n.parse().ok())).collect();
    // Stage 15 at once, then 16 (screen_gone), then 14 to 1.
    assert_eq!(undone, [15u8, 16].into_iter().chain((1..=14).rev()).collect::<Vec<u8>>());
    // Every register as before step 3 but the mark, which stays: a start again runs no stage.
    restored_but(&e, &before, &[]);
    assert_eq!(e.regs[&mark], light::RESTART_MARK);
    let (outcome, _, _) = light_up(wide(), &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Failed { stage: 0, why: Failure::StartedAgain });
    for x in &log { assert!(x.len() <= 180, "{} bytes: {}", x.len(), x); }
}

#[test]
fn the_firmwares_underflow_and_a_request_timeout_are_caught_alike() {
    let img = good();
    let (cntl0, cntl1) = (reg(R::HubpCntl, 0), reg(R::HubpCntl, 1));
    for (at, bits, want) in [(cntl0, 1u32 << 28, format!("AN UNDERFLOW AT {:05X}", cntl0)), (cntl1, 2 << 20, format!("A REQUEST TIMEOUT AT {:05X}", cntl1))] {
        let mut e = engine();
        e.frame_us = DP2;
        let before = e.regs.clone();
        let (mut lit, offered) = lit_on(&mut e, &img, light::WATCH_FRAMES);
        e.timed.push((at, bits, offered + 3_500_000));
        let (log, found) = standing_for(&mut e, &mut lit, offered, 10);
        assert_eq!(found.map(|f| f.0), Some(4), "{:#?}", log);
        assert!(log.iter().any(|x| x.starts_with(&format!("S15 STANDING CHECK: {}", want))), "{} {:#?}", want, log);
        assert_eq!(e.ev.iter().filter(|x| **x == Ev::Gone).count(), 1);
        restored_but(&e, &before, &[at]);
    }
}

#[test]
fn an_odm_underflow_that_is_over_within_the_second_is_still_caught_by_its_latched_status() {
    // OPTC_UNDERFLOW_OCCURRED_STATUS (bit 10) is latched until OPTC_UNDERFLOW_CLEAR (bit 12); _OCCURRED_CURRENT (bit 13)
    // is live (dcn_2_1_0_sh_mask.h; Display Core reads the status, then clears it). An underflow of 100 ms between two
    // checks: the live bit is back to 0 at the check, the latched one is not.
    let img = good();
    let odm1 = reg(R::OptcInputGlobalControl, 1);
    let mut e = engine();
    e.frame_us = DP2;
    let (mut lit, offered) = lit_on(&mut e, &img, light::WATCH_FRAMES);
    let t = offered + 5_200_000;
    e.timed.push((odm1, 1 << 10, t));
    e.window.push((odm1, 1 << 13, t, t + 100_000));
    let checked_at = offered + 6_000_000;
    let (log, found) = standing_for(&mut e, &mut lit, offered, 10);
    assert_eq!(found.map(|f| f.0), Some(6));
    assert!(matches!(found.unwrap().1, Failure::Underflow { offset, value } if offset == odm1 && value & 1 << 10 != 0 && value & 1 << 13 == 0), "{:?}", found);
    assert!(checked_at > t + 100_000, "the live bit was over before the check");
    assert!(log.iter().any(|x| x.starts_with(&format!("S15 STANDING CHECK: AN UNDERFLOW AT {:05X}", odm1))), "{:#?}", log);
}

#[test]
fn an_underflow_one_frame_after_the_watch_ends_is_caught_within_a_second_with_either_watch() {
    let img = good();
    let (cntl1, dpg1) = (reg(R::HubpCntl, 1), reg(R::DpgControl, 1));
    for frames in [light::WATCH_FRAMES, light::SHORT_WATCH_FRAMES] {
        let mut e = engine();
        e.frame_us = DP2;
        let before = e.regs.clone();
        // Latched one frame after the watch's last, counted from the pattern going off.
        e.arm.push((dpg1, 1, cntl1, 1 << 28, (frames as u64 + 1) * DP2));
        let (outcome, mut lit, log) = light_up(wide().with_watch(frames), &img, &mut e, 16);
        assert_eq!(outcome, Outcome::Stopped { done: 16 }, "the watch of {} saw it: {:#?}", frames, &log[log.len().saturating_sub(6)..]);
        assert!(log.iter().any(|x| x.starts_with("S15 UNBLANK DONE") && x.contains(&format!("WATCHED {} FRAMES IN ", frames))), "{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>());
        let offered = e.clock;
        let (_, found) = standing_for(&mut e, &mut lit, offered, 5);
        assert_eq!(found.map(|f| f.0), Some(1), "watch {}", frames);
        restored_but(&e, &before, &[cntl1]);
    }
}

#[test]
fn the_watch_is_300_frames_unless_the_build_gives_60_with_the_standing_check() {
    let (f, c) = EXPECTED;
    let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap();
    assert_eq!((l.watch_frames, light::WATCH_FRAMES, light::SHORT_WATCH_FRAMES), (300, 300, 60));
    assert_eq!(l.with_watch(1000).watch_frames, 300, "never longer than the watch was");
    let short = wide().with_watch(light::SHORT_WATCH_FRAMES);
    let mut e = engine();
    e.frame_us = DP2;
    let img = good();
    let (outcome, _, log) = light_up(short, &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let watched = log.iter().find(|x| x.starts_with("S15 WATCHED ")).unwrap();
    assert!(watched.starts_with("S15 WATCHED 60 FRAMES OF OTG1 IN "), "{}", watched);
    assert_eq!(log.iter().filter(|x| x.starts_with("S15 FRAMES ")).count(), 1, "one line of 60 marks");
    // The short watch comes only with the standing check (213-DRV-0099: the setting's plan decides both, not a build
    // switch): for every setting and every bring-up build.
    for setting in [setting::Setting::Off, setting::Setting::Read, setting::Setting::Light] {
        for b in bringups() {
            let p = setting::plan(setting, b);
            assert_eq!(p.watch_frames == light::SHORT_WATCH_FRAMES, p.standing_check, "{:?} {:?}", setting, b);
            assert!(p.watch_frames == light::SHORT_WATCH_FRAMES || p.watch_frames == light::WATCH_FRAMES);
        }
    }
}

// Every build the bring-up switches can make (213-DRV-0099).
fn bringups() -> Vec<setting::Bringup> {
    let mut all = vec![];
    for survey_order in [false, true] { for undo_after_stages in [false, true] { for stages_allowed in 0..=dcn::STAGES + 1 {
        all.push(setting::Bringup { survey_order, undo_after_stages, stages_allowed });
    } } }
    all
}

#[test]
fn an_unplug_and_a_plug_inside_ten_minutes_count_apart_from_the_time_between_two_checks() {
    // Checks once a second while lit; an unplug's take-back at 100 s, dark for 12 s, then the stages run again (on the
    // model's clock, some 6 s) and the checks go on: the line's longest time is the lit one's, the dark counted once.
    let img = good();
    let mut e = engine();
    e.frame_us = DP2;
    let (mut lit, _) = lit_on(&mut e, &img, light::WATCH_FRAMES);
    let start = e.clock;
    let mut account = light::Standing::new(start);
    let mut out = Log(vec![]);
    let check = |e: &mut Engine, lit: &Lit, account: &mut light::Standing, out: &mut Log| { e.clock += light::STANDING_US; light::standing_check(&lit.l, e, &lit.journal, account, out).unwrap(); };
    for _ in 0..100 { check(&mut e, &lit, &mut account, &mut out); }
    assert!(take_back(&mut lit, &mut e, 16).0);
    account.dark(e.clock);
    e.clock += 12_000_000;
    let (outcome, _) = light::run_timed(&lit.l, &mut e, &mut lit.interp, &mut lit.journal, 16, false, &mut Log(vec![]));
    assert_eq!(outcome, Outcome::Stopped { done: 16 });
    let relit = e.clock;
    while e.clock < start + light::STANDING_SAID_US { check(&mut e, &lit, &mut account, &mut out); }
    let dark_s = (relit + light::STANDING_US - (start + 100 * light::STANDING_US)) / 1_000_000;
    assert_eq!(out.0.len(), 1, "{:#?}", out.0);
    let line = &out.0[0];
    assert!(line.starts_with("S15 STANDING CHECK: ") && line.contains(" CHECKS IN 10 MIN, THE LONGEST 1000 MS BETWEEN TWO WHILE LIT; DARK 1 TIMES, ")
        && line.ends_with(&format!(", {} S IN ALL; NOTHING NEW ON EITHER PIPE", dark_s)), "{} (dark {} s)", line, dark_s);
    assert!(dark_s >= 18, "the 12 s and the run again: {} s", dark_s);
    // The next ten minutes, all lit: the line as before, the dark not carried over.
    for _ in 0..600 { check(&mut e, &lit, &mut account, &mut out); }
    assert_eq!(out.0.last().unwrap(), "S15 STANDING CHECK: 600 CHECKS IN 10 MIN, THE LONGEST 1000 MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE");
}

// --- the setting (213-DRV-0099) ---

use std::cell::RefCell;

#[test]
fn the_word_after_the_devices_index_is_the_setting_and_none_or_an_unknown_one_is_off() {
    use setting::{parse, Given, Setting};
    let cases = [("24 light", "24", Setting::Light, Given::Word), ("24 read", "24", Setting::Read, Given::Word), ("24 off", "24", Setting::Off, Given::Word),
        ("  24   LiGhT ", "24", Setting::Light, Given::Word), ("24", "24", Setting::Off, Given::Missing), ("", "", Setting::Off, Given::Missing),
        ("24 bright", "24", Setting::Off, Given::Unknown("bright")), ("24 light now", "24", Setting::Off, Given::Unknown("light now"))];
    for (args, device, s, g) in cases {
        let a = parse(args);
        assert_eq!((a.device, a.setting, a.given), (device, s, g), "{:?}", args);
    }
    // The log's line: the setting, how it was given, and what it does.
    assert_eq!(parse("24 light").to_string(), "THE SECOND OUTPUT: LIGHT, THE WORD AFTER DEVICE 24: STEPS 1 TO 4, THE EDID DECIDING THE MODE, THEN HPD 1 FOLLOWED AND THE STANDING CHECK");
    assert_eq!(parse("24 read").to_string(), "THE SECOND OUTPUT: READ, THE WORD AFTER DEVICE 24: STEP 1 READ, THE STAGES DESCRIBED, NOTHING WRITTEN");
    assert_eq!(parse("24 off").to_string(), "THE SECOND OUTPUT: OFF, THE WORD AFTER DEVICE 24: NO REGISTER MAPPED, NOTHING READ OR WRITTEN");
    assert_eq!(parse("24").to_string(), "THE SECOND OUTPUT: OFF, NO WORD AFTER DEVICE 24: NO REGISTER MAPPED, NOTHING READ OR WRITTEN");
    assert_eq!(parse("24 bright").to_string(), "THE SECOND OUTPUT: OFF, \"bright\" AFTER DEVICE 24 IS NOT OFF, READ OR LIGHT: NO REGISTER MAPPED, NOTHING READ OR WRITTEN");
    // An unknown word is shown up to 32 bytes, cut at a character.
    let long = format!("24 {}", "x".repeat(100));
    let wide = format!("24 {}", "ж".repeat(40));
    let odd = format!("24 x{}", "ж".repeat(40));
    assert!(parse(&long).to_string().contains(&format!("\"{}...\"", "x".repeat(32))), "{}", parse(&long));
    assert!(parse(&wide).to_string().contains(&format!("\"{}...\"", "ж".repeat(16))), "{}", parse(&wide));
    assert!(parse(&odd).to_string().contains(&format!("\"x{}...\"", "ж".repeat(15))), "{}", parse(&odd));
    for args in ["24 light", "24 read", "24", long.as_str(), wide.as_str(), odd.as_str()] {
        let line = format!("[DISPLAY] {}", parse(args));
        assert!(line.len() <= 180, "{} bytes: {}", line.len(), line);
    }
}

#[test]
fn each_setting_plans_what_it_says_and_a_bring_up_switch_changes_only_light_and_says_so_first() {
    use setting::{plan, Bringup, Setting};
    let r = Bringup::RELEASE;
    let off = plan(Setting::Off, r);
    assert_eq!((off.batches, off.edid, off.stages, off.undo_after_stages, off.hotplug, off.standing_check, off.writes()), (0, false, 0, false, false, false, false));
    let read = plan(Setting::Read, r);
    assert_eq!((read.batches, read.edid, read.stages, read.hotplug, read.standing_check, read.writes()), (batches::BATCHES, false, 0, false, false, false));
    // Light: what boots 95 to 100 ran on the Ryzen (11 batches, the EDID, 16 stages, HOTPLUG, STANDING_CHECK, SHORT_WATCH).
    let lit = plan(Setting::Light, r);
    assert_eq!((lit.order, lit.batches, lit.edid, lit.stages, lit.undo_after_stages, lit.hotplug, lit.standing_check, lit.watch_frames, lit.writes()),
               (batches::Order::Production, 11, true, 16, false, true, true, light::SHORT_WATCH_FRAMES, true));
    for b in bringups() {
        // Off and read whatever the build, but for the order step 1 names.
        for s in [Setting::Off, Setting::Read] { assert_eq!(setting::Plan { order: batches::Order::Production, ..plan(s, b) }, plan(s, r), "{:?} {:?}", s, b); }
        let p = plan(Setting::Light, b);
        assert_eq!(p.order == batches::Order::Survey, b.survey_order);
        assert_eq!(p.stages, b.stages_allowed.min(dcn::STAGES));
        assert!(!p.hotplug || (p.stages > 0 && !p.undo_after_stages), "{:?}", b);
        assert!(!p.standing_check || (p.stages >= 15 && !p.undo_after_stages), "{:?}", b);
        assert_eq!(b.is_release(), p == lit, "{:?}", b);
        // A bring-up build names its switches.
        let line = b.to_string();
        assert_eq!(line.starts_with("A BRING-UP BUILD, NOT A RELEASE:"), !b.is_release(), "{}", line);
        assert_eq!(line.contains(" SURVEY_ORDER"), b.survey_order);
        assert_eq!(line.contains(" UNDO_AFTER_STAGES"), b.undo_after_stages);
        assert_eq!(line.contains(&format!(" STAGES_ALLOWED {} OF 16", b.stages_allowed)), b.stages_allowed < 16);
        assert!(line.len() + "[DISPLAY] ".len() <= 180, "{}", line);
    }
    // main.rs: the release's values committed; its first line the build's when it is a bring-up one, then the setting's,
    // before anything is read.
    let main = include_str!("../display/src/main.rs");
    for c in ["const SURVEY_ORDER: bool = false;", "const UNDO_AFTER_STAGES: bool = false;", "const STAGES_ALLOWED: u8 = dcn::STAGES;"] { assert!(main.contains(c), "{}", c); }
    let body = &main[main.find("fn main(").unwrap()..];
    let at = |x: &str| body.find(x).unwrap_or_else(|| panic!("{}", x));
    assert!(at("mind::println!") == at("mind::println!(\"[DISPLAY] {}\", BRINGUP)"));
    assert!(at("BRINGUP); }") < at("mind::println!(\"[DISPLAY] {}\", args);") && at("\", args);") < at("cap_info(ENGINE)"));
}

// The model shared by display's window, its writer and the case, so that what each part did shows after it.
struct Shared<T>(Rc<RefCell<T>>);
impl<T> Clone for Shared<T> { fn clone(&self) -> Self { Shared(self.0.clone()) } }
impl<T: Bus> Bus for Shared<T> { fn read(&mut self, r: Reg) -> Option<u32> { self.0.borrow_mut().read(r) } }
impl<T: Poke> Poke for Shared<T> { fn write(&mut self, w: Writable, v: u32) { self.0.borrow_mut().write(w, v) } }
impl<T: Clock> Clock for Shared<T> {
    fn now_us(&mut self) -> u64 { self.0.borrow_mut().now_us() }
    fn delay_us(&mut self, us: u32) { self.0.borrow_mut().delay_us(us) }
}
impl<T: Canvas> Canvas for Shared<T> {
    fn row(&mut self, y: usize, pitch: usize, pixels: &[u32]) -> bool { self.0.borrow_mut().row(y, pitch, pixels) }
    fn sample(&mut self, x: usize, y: usize, pitch: usize) -> Option<u32> { self.0.borrow_mut().sample(x, y, pitch) }
}
impl<T: Screens> Screens for Shared<T> {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> { self.0.borrow_mut().offer(width, height, pitch) }
    fn gone(&mut self) -> Result<(), ScreenRefused> { self.0.borrow_mut().gone() }
}

// display's device on the model: how often its window was mapped and a writer made, and display's own lines.
struct Model<T> { at: Shared<T>, maps: u32, writers: u32, said: Vec<String> }
impl<T: Bus + Poke + Clock + Canvas + Screens> start::Device for Model<T> {
    type Window = Shared<T>;
    type Writer = Shared<T>;
    fn map(&mut self) -> Option<Shared<T>> { self.maps += 1; Some(self.at.clone()) }
    fn writer(&mut self, window: Shared<T>) -> Shared<T> { self.writers += 1; window }
    fn now_us(&mut self) -> u64 { self.at.0.borrow_mut().now_us() }
    fn say(&mut self, a: fmt::Arguments) { self.said.push(a.to_string()); }
}

// Step 2 on the model: DCN 2.1's I2C engine and a monitor at 0x50, as display_ddc_host.rs models them (its arbitration,
// its indexed buffer, its transactions run on GO), found as batch 11 found it there.
const I2C_CONTROL: u32 = 0x14D60;
const I2C_ARBITRATION: u32 = 0x14D64;
const I2C_SW_STATUS: u32 = 0x14D6C;
const I2C_TRANSACTION: u32 = 0x14DB8;
const I2C_DATA: u32 = 0x14DC8;
const I2C_TIME_BASE: u32 = 0x004EC;
const I2C_PADS: u32 = 0x17640;
const I2C_FOUND: [(u32, u32); 12] = [(I2C_ARBITRATION, 0), (I2C_CONTROL, 0x0000_0002), (I2C_SW_STATUS, 0), (0x14D70, 0), (0x14D88, 0x12C0_0102),
    (0x14D8C, 0x0300_0044), (I2C_TRANSACTION, 0x0001_1100), (I2C_TRANSACTION + 4, 0x0080_3101), (I2C_TRANSACTION + 8, 0), (I2C_TRANSACTION + 12, 0),
    (I2C_TIME_BASE, 0x0000_0130), (I2C_PADS, 0x0000_0101)];
// DDC2's own registers as batch 12 finds them (213-DRV-0112): set up as DDC1's, its pads in GPIO mode.
const I2C_PADS_2: u32 = 0x17650;
const I2C_FOUND_2: [(u32, u32); 4] = [(0x14D74, 0), (0x14D90, 0x12C0_0102), (0x14D94, 0x0300_0044), (I2C_PADS_2, 0x0000_0101)];
fn i2c_reg(o: u32) -> bool { (I2C_CONTROL..=I2C_DATA).contains(&o) || o == I2C_TIME_BASE || (I2C_PADS..=I2C_PADS + 12).contains(&o) || (I2C_PADS_2..=I2C_PADS_2 + 12).contains(&o) }

struct I2c { regs: HashMap<u32, u32>, buf: [u8; 538], index: usize, reading: bool, monitor: Vec<u8>, pointer: usize, granted: bool, busy_until: u64, done: u32, reads: u64, writes: Vec<(u32, u32)>,
    // The monitor on DDC2 (DC_I2C_DDC_SELECT 1), none when empty; `monitor` is DDC1's (213-DRV-0112).
    monitor2: Vec<u8> }
impl I2c {
    // No monitor answers when `monitor` is empty.
    fn new(monitor: Vec<u8>) -> I2c {
        I2c { regs: I2C_FOUND.into_iter().chain(I2C_FOUND_2).collect(), buf: [0; 538], index: 0, reading: false, monitor, pointer: 0, granted: false, busy_until: 0, done: 0, reads: 0, writes: vec![],
              monitor2: vec![] }
    }
    fn go(&mut self, now: u64) {
        if !self.granted { self.done = 1 << 4; return; }
        let count = ((self.regs[&I2C_CONTROL] >> 20) & 3) as usize + 1;
        // The line's monitor: DDC1's or DDC2's, as DC_I2C_DDC_SELECT names it.
        let monitor = if (self.regs[&I2C_CONTROL] >> 8) & 7 == 1 { self.monitor2.clone() } else { self.monitor.clone() };
        let mut p = 0;
        for t in 0..count {
            let tr = self.regs.get(&(I2C_TRANSACTION + 4 * t as u32)).copied().unwrap_or(0);
            let (read, n) = (tr & 1 == 1, ((tr >> 16) & 0x3FF) as usize);
            let address = self.buf[p];
            p += 1;
            if monitor.is_empty() || address >> 1 != 0x50 { self.done = 1 << 8; self.busy_until = now + 300; return; }
            if read {
                for k in 0..n { self.buf[p + k] = monitor[(self.pointer + k) % monitor.len()]; }
                self.pointer += n;
            } else {
                self.pointer = self.buf[p] as usize;
            }
            p += n;
        }
        self.done = 1 << 2;
        self.busy_until = now + 12_000; // 129 bytes at 100 kHz
    }
    fn read(&mut self, o: u32, now: u64) -> u32 {
        self.reads += 1;
        match o {
            I2C_ARBITRATION => (self.regs[&o] & !0x30_000C) | (self.granted as u32) << 2,
            I2C_SW_STATUS => if now < self.busy_until { 1 } else { self.done },
            I2C_DATA => { if !self.reading { return 0; } let v = (self.buf[self.index] as u32) << 8 | 1; self.index += 1; v }
            _ => self.regs.get(&o).copied().unwrap_or(0),
        }
    }
    fn write(&mut self, o: u32, v: u32, now: u64) {
        self.writes.push((o, v));
        match o {
            I2C_ARBITRATION => {
                if v & 1 << 20 != 0 { self.granted = true; }
                if v & 1 << 21 != 0 { self.granted = false; }
                self.regs.insert(o, v & !0x30_000C);
            }
            I2C_DATA => {
                if v & 1 << 31 != 0 { self.index = ((v >> 16) & 0x3FF) as usize; }
                self.reading = v & 1 == 1;
                if !self.reading { self.buf[self.index] = (v >> 8) as u8; self.index += 1; }
            }
            I2C_CONTROL => {
                self.regs.insert(o, v);
                if v & 8 != 0 { self.done = 0; self.busy_until = 0; }
                if v & 1 != 0 { self.go(now); }
            }
            _ => { self.regs.insert(o, v); }
        }
    }
}

// The Ryzen as step 1 finds it (`plugged`), with HDMI-A-1's DDC line and a monitor on it.
struct Both { e: Engine, i2c: I2c }
impl Bus for Both {
    fn read(&mut self, r: Reg) -> Option<u32> {
        let o = r.offset();
        if !i2c_reg(o) { return self.e.read(r); }
        self.e.catch_up();
        self.e.clock += 2;
        Some(self.i2c.read(o, self.e.clock))
    }
}
impl Poke for Both {
    fn write(&mut self, w: Writable, v: u32) {
        let o = w.reg().offset();
        if !i2c_reg(o) { return self.e.write(w, v); }
        self.e.catch_up();
        self.i2c.write(o, v, self.e.clock);
    }
}
impl Clock for Both {
    fn now_us(&mut self) -> u64 { self.e.now_us() }
    fn delay_us(&mut self, us: u32) { self.e.delay_us(us) }
}
impl Canvas for Both {
    fn row(&mut self, y: usize, pitch: usize, pixels: &[u32]) -> bool { self.e.row(y, pitch, pixels) }
    fn sample(&mut self, x: usize, y: usize, pitch: usize) -> Option<u32> { self.e.sample(x, y, pitch) }
}
impl Screens for Both {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> { self.e.offer(width, height, pitch) }
    fn gone(&mut self) -> Result<(), ScreenRefused> { self.e.gone() }
}

// A synthetic monitor shaped like the Ryzen's HDMI one (its own bytes are not kept): `preferred` first, an HDMI 1.4 block
// of 340 MHz and an HDMI Forum one of 600 MHz when `hdmi`, a CTA extension with VICs 16, 4 and 31.
fn dtd(t: &timing::Timing) -> [u8; 18] {
    let mut d = [0u8; 18];
    d[0..2].copy_from_slice(&((t.pixel_khz / 10) as u16).to_le_bytes());
    let (hb, vb) = (t.h_front + t.h_sync + t.h_back, t.v_front + t.v_sync + t.v_back);
    d[2] = t.h_active as u8; d[3] = hb as u8; d[4] = ((t.h_active >> 8) as u8) << 4 | (hb >> 8) as u8;
    d[5] = t.v_active as u8; d[6] = vb as u8; d[7] = ((t.v_active >> 8) as u8) << 4 | (vb >> 8) as u8;
    d[8] = t.h_front as u8; d[9] = t.h_sync as u8;
    d[10] = ((t.v_front & 0xF) as u8) << 4 | (t.v_sync & 0xF) as u8;
    d[11] = ((t.h_front >> 8) as u8) << 6 | ((t.h_sync >> 8) as u8) << 4 | ((t.v_front >> 4) as u8) << 2 | (t.v_sync >> 4) as u8;
    d[12] = 0x20; d[13] = 0x50; d[14] = 0x21;
    d[17] = 0x18 | (t.v_sync_positive as u8) << 2 | (t.h_sync_positive as u8) << 1;
    d
}
fn descriptor(tag: u8, payload: &[u8]) -> [u8; 18] { let mut d = [0u8; 18]; d[3] = tag; d[5..5 + payload.len()].copy_from_slice(payload); d }
fn seal(block: &mut [u8]) { let sum = block[..127].iter().fold(0u8, |s, b| s.wrapping_add(*b)); block[127] = 0u8.wrapping_sub(sum); }
fn monitor_with(preferred: timing::Timing, hdmi: bool) -> Vec<u8> {
    let mut b = vec![0u8; 256];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    let id: u16 = (((b'W' - b'A' + 1) as u16) << 10) | (((b'I' - b'A' + 1) as u16) << 5) | (b'D' - b'A' + 1) as u16;
    b[8..10].copy_from_slice(&id.to_be_bytes());
    b[10..12].copy_from_slice(&0x3440u16.to_le_bytes());
    b[17] = 31; b[18] = 1; b[19] = 4; b[20] = 0x80 | 2 << 4; b[21] = 80; b[22] = 34;
    for s in b[38..54].chunks_mut(2) { s.copy_from_slice(&[1, 1]); }
    b[38] = (1920 / 8 - 31) as u8; b[39] = 3 << 6;
    b[54..72].copy_from_slice(&dtd(&preferred));
    b[72..90].copy_from_slice(&descriptor(0x10, &[]));
    b[90..108].copy_from_slice(&descriptor(0xFD, &[24, 85, 30, 160, 60]));
    b[108..126].copy_from_slice(&descriptor(0xFC, b"WIDE MONITOR\n"));
    b[126] = 1;
    seal(&mut b[..128]);
    let e = &mut b[128..];
    e[0] = 0x02; e[1] = 3;
    let mut blocks: Vec<u8> = vec![0x40 | 3, 16 | 0x80, 4, 31];
    if hdmi {
        blocks.extend_from_slice(&[0x60 | 7, 0x03, 0x0C, 0x00, 0x10, 0x00, 0x38, 68]);
        blocks.extend_from_slice(&[0x60 | 6, 0xD8, 0x5D, 0xC4, 1, 120, 0x80]);
    }
    e[4..4 + blocks.len()].copy_from_slice(&blocks);
    e[2] = (4 + blocks.len()) as u8;
    seal(e);
    b
}
fn hdmi_monitor() -> Vec<u8> { monitor_with(WIDE, true) }

// The ACPI VFCT table around a VBIOS image for 1002:1638, as init grants it.
fn vfct(image: &[u8]) -> Vec<u8> {
    let mut t = vec![0u8; 0x40 + 28];
    t[..4].copy_from_slice(b"VFCT");
    t[52..56].copy_from_slice(&0x40u32.to_le_bytes());
    t[0x40 + 12..0x40 + 14].copy_from_slice(&0x1002u16.to_le_bytes());
    t[0x40 + 14..0x40 + 16].copy_from_slice(&0x1638u16.to_le_bytes());
    t[0x40 + 24..0x40 + 28].copy_from_slice(&(image.len() as u32).to_le_bytes());
    t.extend_from_slice(image);
    let len = t.len() as u32;
    t[4..8].copy_from_slice(&len.to_le_bytes());
    t
}

fn ryzen(monitor: Vec<u8>) -> Model<Both> {
    Model { at: Shared(Rc::new(RefCell::new(Both { e: plugged(), i2c: I2c::new(monitor) }))), maps: 0, writers: 0, said: vec![] }
}
// display's start on the model, as main.rs starts it on the Radeon (its engine's window of 108 KiB).
fn start_with<'a>(m: &mut Model<Both>, plan: &setting::Plan, table: &'a [u8], out: &mut dyn Out) -> start::Started<'a, Shared<Both>> {
    start::start(m, plan, start::RADEON, dcn::DCN_BYTES.end as usize, Some(table), "TESTBUILD", 0, out)
}
// What follows the stages for `ms`, HPD 1 sensing `sense`, display sleeping between polls as main.rs does.
fn follow_for<'a>(m: &mut Model<Both>, w: &mut Shared<Both>, f: &mut start::Follower<'a>, plan: &setting::Plan, table: &'a [u8], sense: bool, ms: u64, out: &mut dyn Out) {
    m.at.0.borrow_mut().e.regs.insert(reg(R::HpdIntStatus, 0), (sense as u32) << 1);
    let end = m.at.0.borrow().e.clock + ms * 1000;
    while m.at.0.borrow().e.clock < end {
        m.at.0.borrow_mut().e.clock += f.sleep_ms() as u64 * 1000;
        f.poll(m, w, plan, Some(table), "TESTBUILD", out);
    }
}
fn said(m: &Model<Both>, x: &str) -> bool { m.said.iter().any(|s| s == x) }
fn said_start(m: &Model<Both>, x: &str) -> bool { m.said.iter().any(|s| s.starts_with(x)) }

#[test]
fn off_no_word_and_an_unknown_one_map_nothing_read_nothing_and_write_nothing() {
    let table = vfct(&good());
    let mut first: Option<Vec<String>> = None;
    for args in ["24 off", "24", "24 bright", ""] {
        let plan = setting::plan(setting::parse(args).setting, setting::Bringup::RELEASE);
        assert_eq!(plan, setting::plan(setting::Setting::Off, setting::Bringup::RELEASE), "{:?}", args);
        let mut m = ryzen(hdmi_monitor());
        let mut log = Log(vec![]);
        let s = start_with(&mut m, &plan, &table, &mut log);
        let b = m.at.0.borrow();
        // The window never mapped, no writer made, not one read or write of the engine or its I2C engine, no offer.
        assert_eq!((m.maps, m.writers, b.e.reads, b.i2c.reads, b.i2c.writes.len()), (0, 0, 0, 0, 0), "{:?}", args);
        assert!(device(&b.e).is_empty(), "{:?}", args);
        assert!(s.writer.is_none() && s.run.is_none() && (s.read, s.written) == (0, 0));
        for want in ["STEP 1 MAY READ 0 OF 11 BATCHES, WRITES NOTHING; THE PRODUCTION ORDER", "STEP 2 (THE EDID OVER DDC1) NOT RUN: THE SETTING DOES NOT ASK FOR IT", "NO REGISTER READ OR WRITTEN"] {
            assert!(said(&m, want), "{:?}: {} {:#?}", args, want, m.said);
        }
        assert!(said_start(&m, "STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: STEP 1'S BATCH 1 WAS NOT READ") && said_start(&m, "NO SECOND SCREEN OFFERED, IN THE PRODUCTION ORDER"), "{:#?}", m.said);
        assert!(!m.said.iter().any(|x| x.starts_with("STEP 3'S MODE")));
        assert_eq!(log.0.last().unwrap(), &format!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", dcn::STAGES));
        assert!(log.0.iter().any(|x| x.starts_with("STEP 1 DONE: 0 OF 11 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN")), "{:#?}", log.0);
        // The same, whichever way off was given.
        match &first { None => first = Some(m.said.clone()), Some(f) => assert_eq!(f, &m.said, "{:?}", args) }
    }
}

#[test]
fn read_maps_the_window_reads_step_1_and_describes_the_stages_with_no_writer() {
    let table = vfct(&good());
    let plan = setting::plan(setting::parse("24 read").setting, setting::Bringup::RELEASE);
    let mut m = ryzen(hdmi_monitor());
    let mut log = Log(vec![]);
    let s = start_with(&mut m, &plan, &table, &mut log);
    assert_eq!((m.maps, m.writers), (1, 0));
    assert!(s.writer.is_none() && s.run.is_none() && s.read > 0 && s.written == 0, "{} {}", s.read, s.written);
    let b = m.at.0.borrow();
    // Read: the engine and batch 11's I2C registers; written: nothing, no offer.
    assert!(b.e.reads > 0 && b.i2c.reads > 0, "{} {}", b.e.reads, b.i2c.reads);
    assert!(device(&b.e).is_empty() && b.i2c.writes.is_empty(), "{:?}", &b.i2c.writes);
    assert!(log.0.iter().any(|x| x.starts_with("STEP 1 DONE: 11 OF 11 BATCHES READ, ")), "{:#?}", log.0);
    assert!(said(&m, "STEP 1 MAY READ 11 OF 11 BATCHES, WRITES NOTHING; THE PRODUCTION ORDER") && said(&m, "STEP 2 (THE EDID OVER DDC1) NOT RUN: THE SETTING DOES NOT ASK FOR IT"));
    assert!(said(&m, &format!("{} REGISTERS READ, NONE WRITTEN", s.read)) && said_start(&m, "NO SECOND SCREEN OFFERED"), "{:#?}", m.said);
    // The stages described for the pipe step 1 found, the mode today's in DVI (no EDID read).
    assert!(log.0.iter().any(|x| x.starts_with("S0 THE FIRMWARE'S PIPE (AS STEP 1 FOUND)")));
    assert!(log.0.iter().any(|x| x.ends_with("MODE 1920x1080 AT 148.5 MHZ IN DVI SIGNALLING (CTA VIC 16)")), "{:#?}", log.0.iter().filter(|x| x.starts_with("S0")).collect::<Vec<_>>());
    assert_eq!(log.0.last().unwrap(), &format!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", dcn::STAGES));
}

// What the bench builds of boots 95 to 100 ran on the Ryzen: display's main.rs before 213-DRV-0099 with BATCHES_ALLOWED 11,
// STAGES_ALLOWED 16, EDID_ALLOWED, HDMI_SIGNALLING, MODE_FROM_EDID, HOTPLUG, STANDING_CHECK and SHORT_WATCH set: step 1 in
// the production order, step 2, the mode from the EDID in HDMI, the 16 stages with the watch of 60. The engine's writes,
// the offer and the I2C engine's writes, in order.
fn bench_run(table: &[u8], monitor: Vec<u8>) -> (Outcome, Vec<Ev>, Vec<(u32, u32)>) {
    let mut both = Both { e: plugged(), i2c: I2c::new(monitor) };
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    let mut store = Store::default();
    batches::read_into(&mut both, 11, batches::Order::Production, &mut held, &mut store);
    let edid = ddc::read(&store, &mut both, &mut held).ok().and_then(|r| ddc::describe(&r, &mut held));
    let (t, s, _) = light::mode_from_edid(edid.as_ref(), timing::first_light_mode(&start::HDMI_A_1_VICS), dcn::Signal::Hdmi);
    let l = light::prepare(&store, t, s, "TESTBUILD").unwrap().with_watch(light::SHORT_WATCH_FRAMES);
    let (_, _, image) = atom::vfct_image(table).unwrap();
    let mut interp = atom::Interpreter::new(atom::Image::new(image).unwrap());
    let mut journal = Box::new(light::Journal::default());
    let (outcome, _) = light::run_timed(&l, &mut both, &mut interp, &mut journal, 16, false, &mut held);
    (outcome, device(&both.e), both.i2c.writes.clone())
}

#[test]
fn light_reads_the_edid_lights_its_mode_in_16_stages_as_the_bench_did_then_follows_hpd_1_with_the_standing_check() {
    let table = vfct(&good());
    let plan = setting::plan(setting::parse("24 light").setting, setting::Bringup::RELEASE);
    let mut m = ryzen(hdmi_monitor());
    let before = m.at.0.borrow().e.regs.clone();
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    let s = start_with(&mut m, &plan, &table, &mut held);
    assert_eq!((m.maps, m.writers), (1, 1));
    let outcome = s.run.as_ref().unwrap().outcome;
    let log = held.inner.0.clone();
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log[log.len().saturating_sub(8)..]);
    // The EDID read and its mode, the watch of 60, the offer; the lines main.rs logs.
    assert!(log.iter().any(|x| x.starts_with("D STEP 2 DONE: 2 BLOCKS READ (256 BYTES)")), "{:#?}", log);
    assert!(said(&m, "STEP 3'S MODE: 3440x1440 AT 324000 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING"), "{:#?}", m.said);
    assert!(log.iter().any(|x| x.ends_with("MODE 3440x1440 AT 324 MHZ IN HDMI SIGNALLING (CTA VIC 0)")));
    assert!(log.iter().any(|x| x.starts_with("S15 UNBLANK DONE") && x.contains(", WATCHED 60 FRAMES IN ")), "{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>());
    assert!(log.iter().any(|x| x.starts_with("S16 OFFER DONE") && x.ends_with(", OFFERED 3440x1440 PITCH 13824")));
    assert!(said_start(&m, "THE SECOND SCREEN OFFERED ") && said(&m, &format!("{} REGISTERS READ BY STEP 1, {} WRITES BY STEP 3", s.read, s.written)), "{:#?}", m.said);
    // The same writes, offer and I2C transfers, in the same order, as the bench's switches made on the same model.
    let first = device(&m.at.0.borrow().e);
    let (bench_outcome, bench_device, bench_i2c) = bench_run(&table, hdmi_monitor());
    assert_eq!(bench_outcome, outcome);
    assert_eq!(first, bench_device);
    assert_eq!(m.at.0.borrow().i2c.writes, bench_i2c);
    assert!(first.contains(&Ev::Offer(3440, 1440, 13824)) && first.iter().filter(|x| matches!(x, Ev::Write(..))).count() > 100);
    // What follows: HPD 1 every 50 ms and the standing check, the watch named.
    let mut w = s.writer.unwrap();
    let mut f = start::Follower::new(&mut m, &mut w, s.run, &plan).expect("nothing followed");
    assert!(said(&m, "HPD 1 FOLLOWED EVERY 50 MS, A CHANGE COUNTED AFTER 500 MS: SENSE 1, THE SECOND OUTPUT Lit"), "{:#?}", m.said);
    assert!(said(&m, "THE STANDING CHECK: BOTH PIPES' UNDERFLOW STATUSES READ EVERY 1000 MS WHILE THE SECOND PICTURE IS UP; THE WATCH 60 FRAMES"));
    assert_eq!((f.sleep_ms(), f.state()), (hpd::POLL_MS, hpd::State::Lit));
    // Eleven minutes plugged: the check's line once, nothing written.
    let mut out = Log(vec![]);
    follow_for(&mut m, &mut w, &mut f, &plan, &table, true, 660_000, &mut out);
    assert_eq!(device(&m.at.0.borrow().e), first, "the check wrote");
    assert_eq!(out.0.len(), 1, "{:#?}", out.0);
    assert!(out.0[0].starts_with("S15 STANDING CHECK: ") && out.0[0].ends_with(" MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE"), "{}", out.0[0]);
    // Unplugged two seconds: taken back once, screen_gone first, every register as before step 3, the mark too.
    let made = m.at.0.borrow().e.ev.len();
    follow_for(&mut m, &mut w, &mut f, &plan, &table, false, 2000, &mut out);
    assert_eq!(f.state(), hpd::State::Dark);
    {
        let b = m.at.0.borrow();
        let after: Vec<Ev> = b.e.ev[made..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
        assert_eq!(after.first(), Some(&Ev::Gone));
        assert_eq!(after.iter().filter(|x| **x == Ev::Gone).count(), 1);
        as_before(&b.e, &before, &[]);
    }
    // Plugged in eight seconds: steps 1 to 4 again, the EDID read again, the same writes and offer as at start.
    let from = m.at.0.borrow().e.ev.len();
    let transfers = m.at.0.borrow().i2c.writes.len();
    follow_for(&mut m, &mut w, &mut f, &plan, &table, true, 8000, &mut out);
    assert_eq!(f.state(), hpd::State::Lit);
    assert!(said(&m, "A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT Lit"));
    let b = m.at.0.borrow();
    let again: Vec<Ev> = b.e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(again, first);
    assert_eq!(&b.i2c.writes[transfers..], &bench_i2c[..], "the EDID read again");
}

#[test]
fn light_takes_the_edids_mode_only_from_an_hdmi_sink_where_it_fits_and_else_1080p_in_hdmi_or_dvi() {
    let table = vfct(&good());
    let plan = setting::plan(setting::Setting::Light, setting::Bringup::RELEASE);
    let over = timing::Timing { pixel_khz: 540_000, ..WIDE };
    for (name, monitor, mode, line) in [
        ("a DVI sink", monitor_with(WIDE, false), "MODE 1920x1080 AT 148.5 MHZ IN DVI SIGNALLING (CTA VIC 16)", "STEP 3'S MODE: 1920x1080 AT 148500 KHZ IN Dvi SIGNALLING, TODAY'S: THE SINK HAS NO HDMI BLOCK"),
        ("its preferred timing over HDMI's 340 MHz", monitor_with(over, true), "MODE 1920x1080 AT 148.5 MHZ IN HDMI SIGNALLING (CTA VIC 16)", "STEP 3'S MODE: 1920x1080 AT 148500 KHZ IN Hdmi SIGNALLING, TODAY'S: ITS PREFERRED TIMING'S 540000 KHZ IS OVER 340000 KHZ"),
        ("no monitor answering on DDC", vec![], "MODE 1920x1080 AT 148.5 MHZ IN DVI SIGNALLING (CTA VIC 16)", "STEP 3'S MODE: 1920x1080 AT 148500 KHZ IN Dvi SIGNALLING, TODAY'S: THE EDID WAS NOT READ"),
    ] {
        let mut m = ryzen(monitor);
        let mut log = Log(vec![]);
        let s = start_with(&mut m, &plan, &table, &mut log);
        assert!(said(&m, line), "{}: {:#?}", name, m.said.iter().filter(|x| x.starts_with("STEP 3'S MODE")).collect::<Vec<_>>());
        assert!(log.0.iter().any(|x| x.ends_with(mode)), "{}", name);
        assert_eq!(s.run.as_ref().unwrap().outcome, Outcome::Stopped { done: 16 }, "{}", name);
        assert!(m.at.0.borrow().e.ev.contains(&Ev::Offer(1920, 1080, 7680)), "{}", name);
        // The I2C engine given back as batch 11 found it, after a read or a failure.
        let b = m.at.0.borrow();
        for (o, v) in I2C_FOUND { if o != I2C_SW_STATUS { assert_eq!(b.i2c.regs.get(&o).copied().unwrap_or(0), v, "{}: {:05X}", name, o); } }
    }
    // The rule alone: an HDMI sink in its mode where it fits; no EDID in DVI.
    let today = timing::CEA_1080P60;
    let edid = edid::Edid::parse(&hdmi_monitor()).unwrap();
    assert_eq!(light::mode_for(Some(&edid), today).1, dcn::Signal::Hdmi);
    assert_eq!(light::mode_for(Some(&edid), today).0.h_active, 3440);
    assert_eq!(light::mode_for(None, today), (today, dcn::Signal::Dvi, light::ModeFrom::NotRead));
}

// A writer that logs each line with the engine's reads so far: where each stage begins.
struct Marks { at: Shared<Both>, lines: Vec<(String, u64)> }
impl Out for Marks {
    fn line(&mut self, a: fmt::Arguments) { let reads = self.at.0.try_borrow().map(|b| b.e.reads).expect("the model is in use"); self.lines.push((a.to_string(), reads)); }
    fn pause(&mut self, _: u32) {}
}

#[test]
fn under_light_a_failure_at_each_stage_is_undone_whole_the_mark_kept_and_nothing_is_tried_again() {
    let table = vfct(&good());
    let plan = setting::plan(setting::Setting::Light, setting::Bringup::RELEASE);
    let mark = reg(R::OtgVerticalInterrupt2Position, 1);
    // A clean start, every line with the reads so far: the reads from the mark to each stage's first.
    let mut m = ryzen(hdmi_monitor());
    let mut marks = Marks { at: m.at.clone(), lines: vec![] };
    let s = start_with(&mut m, &plan, &table, &mut marks);
    assert_eq!(s.run.as_ref().unwrap().outcome, Outcome::Stopped { done: 16 });
    let at = |p: &str| marks.lines.iter().find(|(x, _)| x.starts_with(p)).unwrap_or_else(|| panic!("{}", p)).1;
    let marked = at("S0 MARKED ");
    for n in 1..=dcn::STAGES {
        let begins = at(&format!("S{} {}: STAGE {} OF 16 NEXT", n, dcn::STAGE_NAMES[n as usize - 1], n));
        // The first read of stage n cannot be read.
        let mut m = ryzen(hdmi_monitor());
        m.at.0.borrow_mut().e.fail = Some((mark, (begins - marked) as usize));
        let before = m.at.0.borrow().e.regs.clone();
        let mut buf = vec![0u8; batches::HELD_BYTES];
        let mut held = batches::Held::new(Log(vec![]), &mut buf);
        let s = start_with(&mut m, &plan, &table, &mut held);
        let log = held.inner.0.clone();
        let run = s.run.unwrap();
        assert!(matches!(run.outcome, Outcome::Failed { stage, why: Failure::Unreadable(_) | Failure::Table(atom::Error::Unreadable { .. }) } if stage == n),
                "stage {}: {:?} {:#?}", n, run.outcome, &log[log.len().saturating_sub(6)..]);
        {
            let b = m.at.0.borrow();
            // Undone whole, the firmware's pipe as before, the one line; its detail before it (the production order).
            undone_whole(&b.e, &before, &log, n);
            let failed = log.iter().position(|x| x.starts_with(&format!("S{} {} FAILED: ", n, dcn::STAGE_NAMES[n as usize - 1]))).expect("its line");
            assert!(log[..failed].iter().any(|x| x == &format!("S{} {}: STAGE {} OF 16 NEXT; ITS WRITES:", n, dcn::STAGE_NAMES[n as usize - 1], n)), "stage {}: its detail", n);
            // The mark kept: nothing is tried again this boot. screen_gone only after an offer.
            assert_eq!(b.e.regs[&mark], light::RESTART_MARK, "stage {}", n);
            assert_eq!(b.e.ev.iter().filter(|x| **x == Ev::Gone).count(), (n == 16) as usize, "stage {}", n);
        }
        assert!(said_start(&m, "NO SECOND SCREEN OFFERED, IN THE PRODUCTION ORDER"), "stage {}", n);
        // What follows: HPD 1 followed, the output stopped; an unplug and a plug do nothing, the check does not run.
        let mut w = s.writer.unwrap();
        let mut f = start::Follower::new(&mut m, &mut w, Some(run), &plan).expect("nothing followed");
        assert_eq!(f.state(), hpd::State::Stopped);
        assert!(said(&m, "HPD 1 FOLLOWED EVERY 50 MS, A CHANGE COUNTED AFTER 500 MS: SENSE 1, THE SECOND OUTPUT Stopped"), "{:#?}", m.said);
        let (made, transfers) = (device(&m.at.0.borrow().e).len(), m.at.0.borrow().i2c.writes.len());
        let mut out = Log(vec![]);
        follow_for(&mut m, &mut w, &mut f, &plan, &table, false, 2000, &mut out);
        follow_for(&mut m, &mut w, &mut f, &plan, &table, true, 8000, &mut out);
        assert_eq!((device(&m.at.0.borrow().e).len(), m.at.0.borrow().i2c.writes.len()), (made, transfers), "stage {}: written after the failure", n);
        assert!(out.0.is_empty() && !said_start(&m, "A MONITOR PLUGGED IN"), "stage {}: {:#?}", n, out.0);
        assert_eq!(f.state(), hpd::State::Stopped);
        // display started again (init restarting it): the mark, no stage, nothing written to the engine.
        if n == 8 {
            let mut log = Log(vec![]);
            let s = start_with(&mut m, &plan, &table, &mut log);
            assert_eq!(s.run.unwrap().outcome, Outcome::Failed { stage: 0, why: Failure::StartedAgain });
            assert_eq!(device(&m.at.0.borrow().e).len(), made);
        }
    }
}

#[test]
fn light_in_the_surveys_order_makes_the_production_orders_writes_and_a_bring_up_build_of_fewer_stages_follows_them() {
    let table = vfct(&good());
    let run = |b: setting::Bringup| {
        let mut m = ryzen(hdmi_monitor());
        let mut log = Log(vec![]);
        let s = start_with(&mut m, &setting::plan(setting::Setting::Light, b), &table, &mut log);
        let outcome = s.run.as_ref().map(|r| r.outcome);
        let b = m.at.0.borrow();
        (outcome, device(&b.e), b.i2c.writes.clone(), m.said.clone())
    };
    let production = run(setting::Bringup::RELEASE);
    let survey = run(setting::Bringup { survey_order: true, ..setting::Bringup::RELEASE });
    assert_eq!((survey.0, &survey.1, &survey.2), (production.0, &production.1, &production.2));
    assert!(survey.3.iter().any(|x| x == "STEP 1 MAY READ 11 OF 11 BATCHES, WRITES NOTHING; THE SURVEY'S ORDER"));
    // Eight stages (first light), and the stages undone after they ran.
    let eight = run(setting::Bringup { stages_allowed: 8, ..setting::Bringup::RELEASE });
    assert_eq!(eight.0, Some(Outcome::Stopped { done: 8 }));
    assert!(!eight.1.iter().any(|x| matches!(x, Ev::Offer(..))));
    let undone = run(setting::Bringup { undo_after_stages: true, ..setting::Bringup::RELEASE });
    assert_eq!(undone.0, Some(Outcome::Undone { done: 16 }));
    assert!(undone.1.contains(&Ev::Gone));
}

#[test]
fn under_light_an_offer_the_compositor_does_not_take_leaves_the_picture_on_the_second_output_checked_and_followed() {
    // Stage 16 not ready (no client of the compositor): stages 1 to 15 stay, no second screen; the standing check and HPD 1 go on.
    let table = vfct(&good());
    let plan = setting::plan(setting::Setting::Light, setting::Bringup::RELEASE);
    let mut m = ryzen(hdmi_monitor());
    m.at.0.borrow_mut().e.refuse_offer = Some(ScreenRefused::Unavailable);
    let before = m.at.0.borrow().e.regs.clone();
    let mut log = Log(vec![]);
    let s = start_with(&mut m, &plan, &table, &mut log);
    assert_eq!(s.run.as_ref().unwrap().outcome, Outcome::NotReady { done: 15, stage: 16, why: Failure::Screen(ScreenRefused::Unavailable) });
    assert!(said_start(&m, "NO SECOND SCREEN OFFERED"));
    let mut w = s.writer.unwrap();
    let mut f = start::Follower::new(&mut m, &mut w, s.run, &plan).expect("nothing followed");
    assert_eq!(f.state(), hpd::State::Lit);
    let made = device(&m.at.0.borrow().e).len();
    let mut out = Log(vec![]);
    follow_for(&mut m, &mut w, &mut f, &plan, &table, true, 601_000, &mut out);
    assert_eq!(out.0.len(), 1, "{:#?}", out.0);
    assert!(out.0[0].ends_with("; NOTHING NEW ON EITHER PIPE"), "{}", out.0[0]);
    assert_eq!(device(&m.at.0.borrow().e).len(), made);
    // Unplugged: stages 15 to 1 taken back, no screen_gone without an offer taken.
    follow_for(&mut m, &mut w, &mut f, &plan, &table, false, 2000, &mut out);
    assert_eq!(f.state(), hpd::State::Dark);
    let b = m.at.0.borrow();
    assert!(!b.e.ev.contains(&Ev::Gone));
    as_before(&b.e, &before, &[]);
}

// 213-DRV-0111: display shares its CPU with other services (on the Ryzen with usb_host and bt_hci, where the mouse moved
// in jerks at each light-up), so stage 9 is awake only for its two edges and the watch sleeps between its looks. A wake
// that comes late spoils neither: the count is longer by the frames slept through, and the watch marks them as passed.
#[test]
fn stage_9_and_the_watch_sleep_between_their_polls_and_a_late_wake_spoils_neither() {
    const F: u64 = 16_674;
    let l = step4(Some(boot70_reference()));
    let run = |allowed: u8, set: &dyn Fn(&mut Engine)| {
        let mut e = engine();
        e.frame_us = F;
        set(&mut e);
        let (outcome, log) = run_light(&l, &good(), &mut e, allowed, false);
        (outcome, log, e)
    };
    // Stage 9 on time: 30 frames between two edges, awake for no more than two of them.
    let (_, _, e8) = run(8, &|_| {});
    let (outcome, log, e9) = run(9, &|_| {});
    assert_eq!(outcome, Outcome::Stopped { done: 9 });
    let (awake, slept) = (e9.awake_us - e8.awake_us, e9.slept_us - e8.slept_us);
    assert!(awake <= 2 * F && (28 * F..=30 * F).contains(&slept), "stage 9 awake {} us, asleep {} us", awake, slept);
    let pixel = number_after(&log, "S9 OTG0 COUNTED 30 FRAMES", "ITS PIXEL CLOCK ");
    // A wake 40 ms late near the count's end: more frames counted, from an edge to an edge as exactly as before.
    let from = e9.clock - 25_000;
    let (outcome, log, _) = run(9, &|e| e.late = Some((from, 40_000)));
    assert_eq!(outcome, Outcome::Stopped { done: 9 });
    let (counted, late_pixel) = (number_after(&log, "S9 OTG0 COUNTED ", "COUNTED "), number_after(&log, "S9 OTG0 COUNTED ", "ITS PIXEL CLOCK "));
    assert!((31..=33).contains(&counted) && late_pixel.abs_diff(pixel) <= 40, "{} frames, {} against {} kHz", counted, late_pixel, pixel);
    assert!(log.iter().any(|x| x.starts_with("S9 OTG0 COUNTED ") && x.ends_with("THE REFERENCE CLOCK 48000 KHZ")), "{:#?}", log.iter().filter(|x| x.starts_with("S9 OTG0")).collect::<Vec<_>>());
    // The watch on time: asleep between its looks, every frame read, none passed between two reads.
    let (_, _, e14) = run(14, &|_| {});
    let (outcome, log, e15) = run(15, &|_| {});
    assert_eq!(outcome, Outcome::Stopped { done: 15 });
    let (awake, slept) = (e15.awake_us - e14.awake_us, e15.slept_us - e14.slept_us);
    assert!(awake <= 3 * F && slept >= 280 * F, "stage 15 awake {} us, asleep {} us", awake, slept);
    assert!(log.iter().any(|x| x.starts_with("S15 WATCHED 300 FRAMES OF OTG1 IN ") && x.contains(" WITH THE PATTERN OFF: 0 PASSED BETWEEN TWO READS")));
    // A wake 40 ms late in the watch: the frames slept through are marked as passed and the gap is named; still clean.
    let (dpg, cntl1) = (reg(R::DpgControl, 1), reg(R::HubpCntl, 1));
    let passed = |log: &[String]| -> u32 {
        let watched = log.iter().find(|x| x.starts_with("S15 WATCHED 300 FRAMES")).unwrap_or_else(|| panic!("{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>()));
        watched.split("WITH THE PATTERN OFF: ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap()
    };
    let (outcome, log, _) = run(15, &|e| e.late_after = Some((dpg, 1, 100 * F, 40_000)));
    assert_eq!(outcome, Outcome::Stopped { done: 15 });
    let gap = number_after(&log, "S15 WATCHED 300 FRAMES", "THE LONGEST GAP BETWEEN READS ");
    assert!((1..=2).contains(&passed(&log)) && (41_000..41_200).contains(&gap), "{} passed, the gap {} us", passed(&log), gap);
    assert!(log.iter().any(|x| x == "S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES"));
    let marks: String = log.iter().filter(|x| x.starts_with("S15 FRAMES ")).map(|x| x.split(": ").nth(1).unwrap()).collect();
    assert_eq!((marks.len(), marks.matches('-').count() as u32), (300, passed(&log)));
    // An underflow while display sleeps late is found at its next look: the status is kept until it is cleared.
    let (outcome, log, _) = run(15, &|e| { e.arm.push((dpg, 1, cntl1, 1 << 28, 17 * F + 8_000)); e.late_after = Some((dpg, 1, 16 * F + 5_000, 40_000)); });
    assert!(matches!(outcome, Outcome::Failed { stage: 15, why: Failure::Underflow { offset, .. } } if offset == cntl1), "{:?}", outcome);
    let first = log.iter().find(|x| x.starts_with("S15 HUBP1: UNDERFLOW IN 1 FRAMES, REQUEST TIMEOUT IN 0; ODM1 UNDERFLOW IN 0; THE FIRST AT FRAME ")).unwrap_or_else(|| panic!("{:#?}", log.iter().filter(|x| x.starts_with("S15")).collect::<Vec<_>>()));
    let frame: u32 = first.split("AT FRAME ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!((18..=20).contains(&frame) && passed(&log) >= 1 && first.ends_with("; CLEARED 1 TIMES"), "{}; {} passed", first, passed(&log));
}

// --- the second socket, HDMI-A-2 (213-DRV-0112) ---

// The Ryzen as step 1 finds it with the monitor in the second socket: HPD 2 sensing and enabled, HPD 1 enabled, sensing none.
fn plugged_2() -> Engine {
    let mut e = plugged();
    e.regs.insert(reg(R::HpdIntStatus, 0), 0);
    e.regs.insert(reg(R::HpdIntStatus, 1), 0b10);
    e.regs.insert(reg(R::HpdControl, 1), 1 << 28);
    e
}
fn ryzen_on(e: Engine, monitor: Vec<u8>) -> Model<Both> {
    Model { at: Shared(Rc::new(RefCell::new(Both { e, i2c: I2c::new(monitor) }))), maps: 0, writers: 0, said: vec![] }
}

#[test]
fn with_a_monitor_in_the_second_socket_alone_read_and_light_log_its_blocks_and_write_nothing() {
    let table = vfct(&good());
    for setting in [setting::Setting::Read, setting::Setting::Light] {
        let plan = setting::plan(setting, setting::Bringup::RELEASE);
        let mut m = ryzen_on(plugged_2(), hdmi_monitor());
        let mut buf = vec![0u8; batches::HELD_BYTES];
        let mut held = batches::Held::new(Log(vec![]), &mut buf);
        let s = start_with(&mut m, &plan, &table, &mut held);
        let log = held.inner.0.clone();
        // Batch 12's values, in the production order's log.
        for x in ["B12 HDMI-A-2 HPD 2 (HPD1_): SENSE 1, ENABLED 1; INT_STATUS 00000002, CONTROL 10000000", "B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN"] {
            assert!(log.iter().any(|l| l == x), "{:?}: no {:?} in {:#?}", setting, x, log);
        }
        // Not one write: of the engine, of the I2C engine; no offer.
        let b = m.at.0.borrow();
        assert!(device(&b.e).is_empty() && b.i2c.writes.is_empty(), "{:?}: {:?} {:?}", setting, device(&b.e), b.i2c.writes);
        assert_eq!(s.written, 0);
        assert!(s.read > 0 && said_start(&m, "NO SECOND SCREEN OFFERED"), "{:?}: {:#?}", setting, m.said);
    }
}

// Step 1's values with the monitor in the second socket: as `found`, HPD 1 sensing nothing, HPD 2 sensing.
fn found_2() -> Store { let mut s = found(); s.set(R::HpdIntStatus, 0, 0); s.set(R::HpdIntStatus, 1, 0b10); s }

#[test]
fn hdmi_a_2s_pipe_is_chosen_from_what_step_1_found_with_its_own_front_end_and_refused_as_hdmi_a_1s_is() {
    use light::{assess_for, expected, HDMI_A_2};
    assert_eq!(HDMI_A_2, dcn::Output { phy: 1, hpd: 1, encoder_id: 0x1E, connector_id: 0x0C });
    assert_eq!(expected(HDMI_A_1), EXPECTED);
    let (f, c) = expected(HDMI_A_2);
    assert_eq!((f, c.pipe, c.output, c.attached), (EXPECTED.0, dcn::Pipe { otg: 1, opp: 1, dig_fe: 1 }, HDMI_A_2, [false, true, false, false]));
    assert_eq!(assess_for(&found_2(), HDMI_A_2), Ok(expected(HDMI_A_2)));
    // HDMI-A-1's choice from the same values as before, its own front end A; each output refused without its monitor.
    assert_eq!(assess_for(&found(), HDMI_A_1), Ok(EXPECTED));
    assert_eq!(assess_for(&found(), HDMI_A_2), Err(Refusal::NoMonitor(HDMI_A_2)));
    assert_eq!(assess_for(&found_2(), HDMI_A_1), Err(Refusal::NoMonitor(HDMI_A_1)));
    assert_eq!(Refusal::NoMonitor(HDMI_A_2).to_string(), "HPD 2 SENSES NO MONITOR ON HDMI-A-2");
    assert_eq!(Refusal::NoMonitor(HDMI_A_1).to_string(), "HPD 1 SENSES NO MONITOR ON HDMI-A-1");
    // A monitor in each: either output's pipe can be told; which is lit is start.rs's choice.
    let mut both = found(); both.set(R::HpdIntStatus, 1, 0b10);
    assert_eq!((assess_for(&both, HDMI_A_1), assess_for(&both, HDMI_A_2)), (Ok(EXPECTED), Ok(expected(HDMI_A_2))));
    // The firmware driving PHY B: HDMI-A-2 refused, as HDMI-A-1 is when it drives PHY A.
    let mut on_b = found_2(); on_b.set(R::DigBeEnCntl, 4, 0); on_b.set(R::DigBeEnCntl, 1, 1);
    assert_eq!(assess_for(&on_b, HDMI_A_2), Err(Refusal::HdmiDriven(HDMI_A_2)));
    assert_eq!(Refusal::HdmiDriven(HDMI_A_2).to_string(), "PHY B (HDMI-A-2) IS ALREADY DRIVEN");
    assert_eq!(Refusal::HdmiDriven(HDMI_A_1).to_string(), "PHY A (HDMI-A-1) IS ALREADY DRIVEN");
    // Front end B busy: the lowest free one; every other refusal as for HDMI-A-1.
    let mut busy = found_2(); busy.set(R::DigFeCntl, 1, 1 << 24);
    assert_eq!(assess_for(&busy, HDMI_A_2).unwrap().1.pipe.dig_fe, 0);
    let refuse = |change: &dyn Fn(&mut Store)| { let mut s = found_2(); change(&mut s); assess_for(&s, HDMI_A_2).unwrap_err() };
    assert_eq!(refuse(&|s| s.set_batches(9)), Refusal::NotRead { batch: 10 });
    assert_eq!(refuse(&|s| s.set(R::OtgControl, 2, 0x0001_0001)), Refusal::RunningOtgs(2));
    assert_eq!(refuse(&|s| s.set(R::DmcubCntl, 0, 1 << 16)), Refusal::DmcubRuns);
    assert_eq!(refuse(&|s| s.set(R::HubpCntl, 1, 1 << 4)), Refusal::AttachedHubp(1));
    assert_eq!(refuse(&|s| for d in 0..4 { s.set(R::DigFeCntl, d, 1 << 24); }), Refusal::NoFreeFrontEnd);
}

// Stage `n`'s calls of a plan.
fn calls(l: &Light, n: u8) -> Vec<dcn::Table> { l.stage(n).filter_map(|s| if let Step::Call(t) = s { Some(t) } else { None }).collect() }
fn updates(l: &Light, n: u8) -> Vec<u32> { l.stage(n).filter_map(|s| if let Step::Update { reg, .. } = s { Some(reg.offset()) } else { None }).collect() }

#[test]
fn each_outputs_plan_gives_the_tables_its_own_pll_phy_hpd_and_front_end() {
    use dcn::Table;
    for (o, p, fe) in [(HDMI_A_1, 0u8, 0u8), (light::HDMI_A_2, 1, 1)] {
        let (f, c) = light::expected(o);
        let l = Light::new_with(f, c, WIDE, dcn::Signal::Hdmi).unwrap();
        // Table 12 on the PHY's own PLL (ATOM_COMBOPHY_PLL0 + p), the UNIPHY encoder, OTG 1; then that PHY's resync.
        assert_eq!(calls(&l, 2), [Table::SetPixelClock { pixclk_100hz: 3_240_000, pll_id: 20 + p, encoder_id: 0x1E, encoder_mode: 3, crtc_id: 1, deep_color: 0, misc: 0 }]);
        assert_eq!(updates(&l, 2), [reg(R::PhypllPixclkResyncCntl, p)]);
        // Table 4 for the front end, its source and HDMI registers; the back end of the PHY fed by that front end.
        assert_eq!(calls(&l, 6), [Table::DigEncoderStreamSetup { dig: fe, mode: 3, lanes: 4, pclk_10khz: 32_400, bpc: 2 }]);
        assert!(updates(&l, 6).contains(&reg(R::DigFeCntl, fe)) && updates(&l, 6).contains(&reg(R::HdmiControl, fe)));
        assert_eq!(updates(&l, 7), [reg(R::DigBeCntl, p)]);
        assert!(l.stage(7).any(|s| matches!(s, Step::Update { value, .. } if value == (3 << 16 | (1 << fe) << 8))));
        // Table 76 for the PHY: its HPD, the front end, the connector; the off calls on the same PHY and PLL.
        assert_eq!(calls(&l, 8), [Table::DigTransmitter { phy: p, action: 1, mode: 3, lanes: 4, symclk_10khz: 32_400, hpd_sel: p + 1, dig_fe_sel: 1 << fe, connector_id: 0x0C }]);
        let (bytes, len) = calls(&l, 8)[0].parameters();
        assert_eq!((len, bytes[0], bytes[8], bytes[9], bytes[10]), (32, p, p + 1, 1 << fe, 0x0C));
        assert_eq!(l.off_call(2), Some(Table::SetPixelClock { pixclk_100hz: 0, pll_id: 20 + p, encoder_id: 0, encoder_mode: 0, crtc_id: 0xFF, deep_color: 0, misc: 1 }));
        assert_eq!(l.off_call(8), Some(Table::DigTransmitter { phy: p, action: 0, mode: 2, lanes: 0, symclk_10khz: 0, hpd_sel: p + 1, dig_fe_sel: 1 << fe, connector_id: 0x0C }));
        // Every planned write passes the output's guard.
        for n in 1..=STAGES { for s in l.stage(n) { if let Step::Update { reg, .. } = s { assert!(l.guard().check(reg).is_ok(), "{} {:05X}", light::name(&o), reg.offset()); } } }
    }
}

#[test]
fn hdmi_a_2s_guard_lets_through_phy_bs_blocks_and_refuses_phy_as_and_the_firmwares() {
    let (f, c) = light::expected(light::HDMI_A_2);
    let g = light::Guard::new(&f, &c).unwrap();
    let check = |o: u32| g.check(Reg::new(o).unwrap());
    // PHY B's: DIG B (front and back end), DP1's link registers, UNIPHYB, RDPCSTX1, PHYPLLB, SYMCLKB; OTG 1's pixel rate.
    for o in [reg(R::DigFeCntl, 1), reg(R::DigBeCntl, 1), reg(R::DigBeEnCntl, 1), seg(2, 0x21E1), seg(2, 0x2208), seg(2, 0x2218), seg(2, 0x286F),
              seg(2, 0x2870), seg(2, 0x2928 + 0xD8), seg(2, 0x2940 + 0xD8), reg(R::PhypllPixclkResyncCntl, 1), seg(1, 0xA1), reg(R::OtgPixelRateCntl, 1)] {
        assert!(check(o).is_ok(), "{:05X}", o);
    }
    // HDMI-A-1's: DIG A, DP0, UNIPHYA, RDPCSTX0, PHYPLLA, SYMCLKA; off the route.
    for o in [reg(R::DigFeCntl, 0), reg(R::DigBeCntl, 0), reg(R::DigBeEnCntl, 0), seg(2, 0x2108), seg(2, 0x286D), seg(2, 0x286E), seg(2, 0x2928),
              seg(2, 0x2940), reg(R::PhypllPixclkResyncCntl, 0), seg(1, 0xA0), reg(R::HpdControl, 0), reg(R::HpdControl, 1)] {
        assert_eq!(check(o).err(), Some(Refused::OffRoute(o)), "{:05X}", o);
    }
    // The firmware's, first.
    for o in [reg(R::OtgControl, 0), reg(R::DigBeCntl, 4), seg(2, 0x2928 + 4 * 0xD8), reg(R::HubpCntl, 0)] { assert_eq!(check(o).err(), Some(Refused::Firmware(o)), "{:05X}", o); }
}

const PHY_B_CNTL0: u32 = (0x34C0 + 0x2940 + 0xD8) * 4; // RDPCSTX1_RDPCSTX_PHY_CNTL0
// The synthetic tables of `good` for PHY B and front end B: SYMCLKB and PHY B's CNTL0, DIG B's clock pattern, DIG B's enable.
fn good_b() -> Vec<u8> {
    image(&[(12, [mov(seg(1, 0xA1), 1), mov(PHY_B_CNTL0, 0x0020_0001), vec![EOT]].concat()), (4, [mov(seg(2, 0x216B), 0x1F), vec![EOT]].concat()),
            (76, [mov(seg(2, 0x21B0), 1), vec![EOT]].concat())])
}
// HDMI-A-1's blocks: DIG A's, DP0's, UNIPHYA, RDPCSTX0 and DPCSTX0, PHYPLLA, SYMCLKA, DDC1's.
fn hdmi_a_1s(o: u32) -> bool {
    (seg(2, 0x2068)..seg(2, 0x2068 + 0x100)).contains(&o) || (seg(2, 0x286D)..seg(2, 0x286F)).contains(&o) || (seg(2, 0x2928)..seg(2, 0x2928 + 0xD8)).contains(&o)
        || o == seg(1, 0x40) || o == seg(1, 0xA0) || o == reg(R::DcI2cDdc1Speed, 0) || o == reg(R::DcI2cDdc1Setup, 0) || o == reg(R::DcGpioDdc1Mask, 0)
}

#[test]
fn all_sixteen_stages_light_hdmi_a_2_on_the_model_from_step_1_and_an_unplug_takes_them_back() {
    let img = good_b();
    let mut e = plugged_2();
    let before = e.regs.clone();
    let mut store = Store::default();
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(vec![]), &mut buf);
    batches::read_into(&mut e, batches::BATCHES, batches::Order::Production, &mut held, &mut store);
    batches::read_second(&mut e, true, &mut held, &mut store);
    let l = light::prepare_on(&store, light::HDMI_A_2, WIDE, dcn::Signal::Hdmi, "TESTBUILD").expect("step 3 refused");
    assert_eq!((l.choice.output, l.choice.pipe), (light::HDMI_A_2, dcn::Pipe { otg: 1, opp: 1, dig_fe: 1 }));
    let (outcome, mut lit, log) = light_up(l, &img, &mut e, 16);
    assert_eq!(outcome, Outcome::Stopped { done: 16 }, "{:#?}", &log[log.len().saturating_sub(10)..]);
    // Its own lines: the output, PHY B, HPD 2; the picture's text.
    assert!(log.iter().any(|x| x == "STEP 3: FIRST LIGHT ON HDMI-A-2 IN 8 STAGES, STEP 4 IN 8 MORE (MIND CORE'S PICTURE); THIS BUILD MAY RUN 16"), "{:#?}", &log[..4]);
    assert!(log.iter().any(|x| x == "S0 STEP 3'S PIPE: OTG1 OPP1 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; MODE 3440x1440 AT 324 MHZ IN HDMI SIGNALLING (CTA VIC 0)"));
    assert!(log.iter().any(|x| x == "S10 ITS TEXT: MIND CORE / DISPLAY / HDMI-A-2 3440X1440 / BUILD TESTBUILD"));
    // Every write on HDMI-A-2's route, none of HDMI-A-1's blocks, the firmware's pipe unchanged, the screen offered.
    let ws = writes(&e);
    for (o, _) in &ws { assert!(lit.l.guard().check_at(Reg::new(*o).unwrap(), 16).is_ok(), "{:05X}", o); assert!(!hdmi_a_1s(*o), "HDMI-A-1's {:05X} written", o); }
    for o in [PHY_B_CNTL0, seg(1, 0xA1), seg(2, 0x21B0), reg(R::DigBeCntl, 1), reg(R::PhypllPixclkResyncCntl, 1)] { assert!(ws.iter().any(|w| w.0 == o), "{:05X} not written", o); }
    for o in watched(&lit.l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    assert!(e.ev.contains(&Ev::Offer(3440, 1440, 13824)));
    // The picture's pixels: HDMI-A-2's text.
    let lines = picture::Lines::on("HDMI-A-2", "TESTBUILD", lit.l.surface);
    for (x, y) in picture::samples(lit.l.surface) { assert_eq!(e.surface[y * e.pitch + x], picture::pixel(&lines, x, y)); }
    // Unplugged: HPD 2 named, every register as before step 3.
    let (same, log) = take_back(&mut lit, &mut e, 16);
    assert!(same && log[0].starts_with("STEP 3 TAKEN BACK: HPD 2 SENSES NO MONITOR; UNDOING STAGES 16 TO 1 ("), "{:#?}", log);
    as_before(&e, &before, &[]);
}

// `answered` for PHY `p`: the same statuses at its RDPCSTX/DPCSTX instance (dpcs_2_1_0_offset.h, a stride of 0xD8).
fn answered_on(p: u32) -> Engine {
    let mut e = engine();
    let at = |d: u32| phy(d + 0xD8 * p);
    e.set.push((at(0x2940), 1 << 28));
    e.follow.push((at(0x2943), 0x2020_2020));
    e.derived.push((at(0x2928), 1 << 2, at(0x2928), 0b11));
    e.follow.push((at(0x2931), 1 << 10));
    e.derived.push((at(0x294C), 1 << 7, at(0x2946), 0x4444));
    e
}

#[test]
fn the_ryzens_own_tables_on_phy_b_write_only_hdmi_a_2s_route() {
    let skip = |why: String| { use std::io::Write; let _ = writeln!(std::io::stderr(), "the_ryzens_own_tables_on_phy_b_write_only_hdmi_a_2s_route SKIPPED: {}", why); };
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else { skip("MIND_AMD_VBIOS is not set (the Ryzen's VBIOS image, not in the repository)".into()); return };
    let Ok(bytes) = std::fs::read(&path) else { skip(format!("{} cannot be read", path)); return };
    // First light's eight stages on HDMI-A-2 with PHY B's statuses answered: tables 12, 4 and 76 with PHY B's PLL, front
    // end B and HPD 2; every write on HDMI-A-2's route, none of HDMI-A-1's blocks, the firmware's pipe unchanged.
    let (f, c) = light::expected(light::HDMI_A_2);
    let l = Light::new(f, c, timing::CEA_1080P60).unwrap();
    let mut e = answered_on(1);
    let before = e.regs.clone();
    let mut interp = atom::Interpreter::new(atom::Image::new(&bytes).unwrap());
    let mut journal = light::Journal::default();
    let mut log = Log(vec![]);
    let outcome = light::run(&l, &mut e, &mut interp, &mut journal, STAGES, &mut log);
    for x in log.0.iter().filter(|x| x.contains("TABLE") && x.contains("DONE") || x.contains("FAILED") || x.contains("REFUSED")) { eprintln!("{}", x); }
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log.0[log.0.len().saturating_sub(20)..]);
    for t in [12, 4, 76] { assert!(log.0.iter().any(|x| x.contains(&format!("TABLE {} DONE", t))), "table {}", t); }
    for (w, _) in writes(&e) { assert!(l.guard().check(Reg::new(w).unwrap()).is_ok(), "{:05X}", w); assert!(!hdmi_a_1s(w), "HDMI-A-1's {:05X}", w); }
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
}

// --- which socket (213-DRV-0112) ---

// The Ryzen as step 1 finds it, with a monitor on each DDC line given (none where empty).
fn ryzen_2(e: Engine, monitor: Vec<u8>, monitor2: Vec<u8>) -> Model<Both> {
    let mut i2c = I2c::new(monitor);
    i2c.monitor2 = monitor2;
    Model { at: Shared(Rc::new(RefCell::new(Both { e, i2c }))), maps: 0, writers: 0, said: vec![] }
}
// A monitor in each socket: HPD 1 and HPD 2 sensing and enabled.
fn plugged_both() -> Engine { let mut e = plugged(); e.regs.insert(reg(R::HpdIntStatus, 1), 0b10); e.regs.insert(reg(R::HpdControl, 1), 1 << 28); e }
// Tables that write nothing of their own, so that either output's stages run on one image (`good` and `good_b` are each
// written for one PHY).
fn plain() -> Vec<u8> { image(&[(12, vec![EOT]), (4, vec![EOT]), (76, vec![EOT])]) }
fn light_with(level: setting::HdmiA2) -> setting::Plan { setting::Plan { hdmi_a_2: level, ..setting::plan(setting::Setting::Light, setting::Bringup::RELEASE) } }
// What follows the stages for `ms`, HPD 1 and HPD 2 sensing `senses`, display sleeping between polls as main.rs does.
fn follow_2<'a>(m: &mut Model<Both>, w: &mut Shared<Both>, f: &mut start::Follower<'a>, plan: &setting::Plan, table: &'a [u8], senses: [bool; 2], ms: u64, out: &mut dyn Out) {
    for (i, s) in senses.into_iter().enumerate() { m.at.0.borrow_mut().e.regs.insert(reg(R::HpdIntStatus, i as u8), (s as u32) << 1); }
    let end = m.at.0.borrow().e.clock + ms * 1000;
    while m.at.0.borrow().e.clock < end {
        m.at.0.borrow_mut().e.clock += f.sleep_ms() as u64 * 1000;
        f.poll(m, w, plan, Some(table), "TESTBUILD", out);
    }
}
// HDMI-A-2's blocks: DIG B's, DP1's, UNIPHYB, RDPCSTX1 and DPCSTX1, PHYPLLB, SYMCLKB, DDC2's.
fn hdmi_a_2s(o: u32) -> bool {
    (seg(2, 0x2168)..seg(2, 0x2268)).contains(&o) || (seg(2, 0x286F)..seg(2, 0x2871)).contains(&o) || (seg(2, 0x2928 + 0xD8)..seg(2, 0x2928 + 2 * 0xD8)).contains(&o)
        || o == seg(1, 0x41) || o == seg(1, 0xA1) || o == reg(R::DcI2cDdc2Speed, 0) || o == reg(R::DcI2cDdc2Setup, 0) || o == reg(R::DcGpioDdc2Mask, 0)
}
// Every register of the blocks `of` names that was ever written reads as before step 3 (the model's status bits aside).
fn blocks_as_before(e: &Engine, before: &HashMap<u32, u32>, of: &dyn Fn(u32) -> bool) {
    let status = |o: u32| e.set.iter().chain(e.follow.iter()).filter(|(at, _)| *at == o).fold(0, |m, (_, bits)| m | bits);
    for (o, _) in writes(e).into_iter().filter(|w| of(w.0)) {
        assert_eq!(e.regs.get(&o).copied().unwrap_or(0) & !status(o), before.get(&o).copied().unwrap_or(0) & !status(o), "{:05X} not as before", o);
    }
}

#[test]
fn under_light_a_monitor_in_the_second_socket_alone_goes_as_far_as_the_build_lets_hdmi_a_2_go() {
    use setting::HdmiA2::*;
    let table = vfct(&good_b());
    for level in [Read, Edid, FirstLight, Full] {
        let plan = light_with(level);
        let mut m = ryzen_2(plugged_2(), vec![], hdmi_monitor());
        let mut buf = vec![0u8; batches::HELD_BYTES];
        let mut held = batches::Held::new(Log(vec![]), &mut buf);
        let s = start_with(&mut m, &plan, &table, &mut held);
        let log = held.inner.0.clone();
        let b = m.at.0.borrow();
        let dev = device(&b.e);
        // At no level is anything of HDMI-A-1's written, nor the I2C engine but on DDC2's line.
        for x in &dev { if let Ev::Write(o, _) = x { assert!(!hdmi_a_1s(*o), "{:?}: HDMI-A-1's {:05X} written", level, o); } }
        for &(o, _) in &b.i2c.writes { assert!(light::ddc_writable_on(&light::DDC2_WRITES, Reg::new(o).unwrap()).is_ok(), "{:?}: {:05X}", level, o); }
        let run = s.run.as_ref().map(|r| (r.outcome, r.light.choice.output));
        match level {
            Read => {
                assert!(said(&m, "HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE; THIS BUILD GOES NO FURTHER THAN ITS BLOCKS READ ONLY (HDMI_A_2 READ)"), "{:#?}", m.said);
                assert!(said(&m, "STEP 3 NOT RUN: HPD 1 SENSES NO MONITOR ON HDMI-A-1"), "{:#?}", m.said);
                assert!(dev.is_empty() && b.i2c.writes.is_empty() && run.is_none());
            }
            Edid => {
                assert!(said(&m, "HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 EDID: ITS EDID READ OVER DDC2, ITS STAGES DESCRIBED"), "{:#?}", m.said);
                assert!(log.iter().any(|x| x.starts_with("D STEP 2 DONE: 2 BLOCKS READ (256 BYTES)") && x.ends_with("AS BATCHES 11 AND 12 READ THEM")), "{:#?}", log);
                assert!(said(&m, "STEP 3'S MODE: 3440x1440 AT 324000 KHZ IN Hdmi SIGNALLING, THE EDID'S PREFERRED TIMING"));
                assert_eq!(log.last().unwrap(), &format!("STEP 3 DONE: 0 OF {} STAGES RUN, NOTHING WRITTEN", dcn::STAGES));
                assert!(dev.is_empty() && !b.i2c.writes.is_empty() && run.is_none());
                for (o, v) in I2C_FOUND.into_iter().chain(I2C_FOUND_2) { if o != I2C_SW_STATUS { assert_eq!(b.i2c.regs.get(&o).copied().unwrap_or(0), v, "{:05X}", o); } }
            }
            FirstLight => {
                assert_eq!(run, Some((Outcome::Stopped { done: 8 }, light::HDMI_A_2)));
                assert!(!dev.iter().any(|x| matches!(x, Ev::Offer(..))) && writes(&b.e).iter().any(|w| w.0 == PHY_B_CNTL0));
            }
            Full => {
                assert_eq!(run, Some((Outcome::Stopped { done: 16 }, light::HDMI_A_2)), "{:#?}", &log[log.len().saturating_sub(8)..]);
                assert!(dev.contains(&Ev::Offer(3440, 1440, 13824)));
            }
        }
    }
    // The read setting takes HDMI-A-2's monitor too, its stages only described: nothing written.
    let mut m = ryzen_2(plugged_2(), vec![], hdmi_monitor());
    let mut log = Log(vec![]);
    let s = start_with(&mut m, &setting::plan(setting::Setting::Read, setting::Bringup::RELEASE), &table, &mut log);
    assert!(said(&m, "HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 READ: ITS BLOCKS READ ONLY") && s.run.is_none(), "{:#?}", m.said);
    assert!(log.0.iter().any(|x| x == "S0 STEP 3'S PIPE: OTG1 OPP1 DIG B FRONT END, PHY B (HDMI-A-2), HPD 2; MODE 1920x1080 AT 148.5 MHZ IN DVI SIGNALLING (CTA VIC 16)"), "{:#?}", log.0.iter().filter(|x| x.starts_with("S0")).collect::<Vec<_>>());
    assert!(device(&m.at.0.borrow().e).is_empty() && m.at.0.borrow().i2c.writes.is_empty());
}

#[test]
fn with_a_monitor_in_each_socket_hdmi_a_1_is_lit_the_other_named_and_lit_once_the_first_is_taken_out() {
    let table = vfct(&plain());
    let plan = light_with(setting::HdmiA2::Full);
    let mut m = ryzen_2(plugged_both(), hdmi_monitor(), hdmi_monitor());
    let before = m.at.0.borrow().e.regs.clone();
    let mut log = Log(vec![]);
    let s = start_with(&mut m, &plan, &table, &mut log);
    assert!(said(&m, "HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND THERE IS ONE SPARE PIPE"), "{:#?}", m.said);
    assert_eq!(s.run.as_ref().map(|r| (r.outcome, r.light.choice.output)), Some((Outcome::Stopped { done: 16 }, HDMI_A_1)));
    assert!(!writes(&m.at.0.borrow().e).iter().any(|w| hdmi_a_2s(w.0)) && !m.at.0.borrow().i2c.writes.iter().any(|w| hdmi_a_2s(w.0)));
    let mut w = s.writer.unwrap();
    let mut f = start::Follower::new(&mut m, &mut w, s.run, &plan).expect("nothing followed");
    assert!(said(&m, "HPD 1 FOLLOWED EVERY 50 MS, A CHANGE COUNTED AFTER 500 MS: SENSE 1, THE SECOND OUTPUT Lit"));
    assert!(said(&m, "HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 FULL): SENSE 1; THE SECOND OUTPUT Lit ON HDMI-A-1"), "{:#?}", m.said);
    // HDMI-A-1's monitor taken out: its stages taken back, screen_gone, then HDMI-A-2's monitor lit.
    let mut out = Log(vec![]);
    let from = m.at.0.borrow().e.ev.len();
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [false, true], 9000, &mut out);
    assert!(said(&m, "A MONITOR ON HDMI-A-2: STEPS 1 TO 4 RAN AGAIN ON IT, THE SECOND OUTPUT Lit"), "{:#?}", m.said);
    assert_eq!((f.state(), f.output()), (hpd::State::Lit, Some(light::HDMI_A_2)));
    {
        let b = m.at.0.borrow();
        let after: Vec<Ev> = b.e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
        assert_eq!(after.first(), Some(&Ev::Gone));
        assert!(after.contains(&Ev::Offer(3440, 1440, 13824)));
        blocks_as_before(&b.e, &before, &hdmi_a_1s);
        assert!(b.i2c.writes.iter().any(|w| w.0 == I2C_PADS_2), "HDMI-A-2's EDID not read");
    }
    // HDMI-A-1's monitor back: named, not driven, nothing written.
    let made = device(&m.at.0.borrow().e).len();
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [true, true], 2000, &mut out);
    assert!(said(&m, "HDMI-A-1 NOT DRIVEN: A MONITOR PLUGGED IN THERE; HDMI-A-2 IS LIT, AND THERE IS ONE SPARE PIPE"), "{:#?}", m.said);
    assert_eq!((device(&m.at.0.borrow().e).len(), f.output()), (made, Some(light::HDMI_A_2)));
    // HDMI-A-2's taken out: back to HDMI-A-1, HDMI-A-2's blocks as before.
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [true, false], 9000, &mut out);
    assert_eq!((f.state(), f.output()), (hpd::State::Lit, Some(HDMI_A_1)));
    assert!(said(&m, "A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT Lit"));
    blocks_as_before(&m.at.0.borrow().e, &before, &hdmi_a_2s);

    // The release's level: HDMI-A-1 lit, and once its monitor is out HDMI-A-2's is named and its blocks read, not driven.
    let plan = light_with(setting::HdmiA2::Read);
    let mut m = ryzen_2(plugged_both(), hdmi_monitor(), hdmi_monitor());
    let before = m.at.0.borrow().e.regs.clone();
    let s = start_with(&mut m, &plan, &table, &mut Log(vec![]));
    assert_eq!(s.run.as_ref().map(|r| r.outcome), Some(Outcome::Stopped { done: 16 }));
    let mut w = s.writer.unwrap();
    let mut f = start::Follower::new(&mut m, &mut w, s.run, &plan).expect("nothing followed");
    let mut out = Log(vec![]);
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [false, true], 3000, &mut out);
    assert!(said(&m, "HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE; THIS BUILD GOES NO FURTHER THAN ITS BLOCKS READ ONLY (HDMI_A_2 READ)"), "{:#?}", m.said);
    assert!(out.0.iter().any(|x| x == "B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN"), "{:#?}", out.0);
    assert_eq!(f.state(), hpd::State::Dark);
    let b = m.at.0.borrow();
    as_before(&b.e, &before, &[]);
    assert!(!writes(&b.e).iter().any(|w| hdmi_a_2s(w.0)) && !b.i2c.writes.iter().any(|w| hdmi_a_2s(w.0)));
}

#[test]
fn a_monitor_moved_from_one_socket_to_the_other_and_back_is_lit_each_time_and_each_unplug_puts_every_register_back() {
    let table = vfct(&plain());
    let plan = light_with(setting::HdmiA2::Full);
    let mut m = ryzen_2(plugged(), hdmi_monitor(), vec![]);
    m.at.0.borrow_mut().e.regs.insert(reg(R::HpdControl, 1), 1 << 28);
    let before = m.at.0.borrow().e.regs.clone();
    let s = start_with(&mut m, &plan, &table, &mut Log(vec![]));
    assert_eq!(s.run.as_ref().map(|r| (r.outcome, r.light.choice.output)), Some((Outcome::Stopped { done: 16 }, HDMI_A_1)));
    let first = device(&m.at.0.borrow().e);
    let mut w = s.writer.unwrap();
    let mut f = start::Follower::new(&mut m, &mut w, s.run, &plan).expect("nothing followed");
    let mut out = Log(vec![]);
    let moved = |m: &mut Model<Both>, to_second: bool| {
        let mut b = m.at.0.borrow_mut();
        let i2c = &mut b.i2c;
        if to_second { i2c.monitor2 = std::mem::take(&mut i2c.monitor); } else { i2c.monitor = std::mem::take(&mut i2c.monitor2); }
    };
    // Out of HDMI-A-1: every register as before step 3.
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [false, false], 2000, &mut out);
    assert_eq!(f.state(), hpd::State::Dark);
    as_before(&m.at.0.borrow().e, &before, &[]);
    // Into HDMI-A-2: its EDID over DDC2, its 16 stages, the screen offered; nothing of HDMI-A-1's written.
    moved(&mut m, true);
    let (from, i2c_from) = (m.at.0.borrow().e.ev.len(), m.at.0.borrow().i2c.writes.len());
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [false, true], 9000, &mut out);
    assert_eq!((f.state(), f.output()), (hpd::State::Lit, Some(light::HDMI_A_2)));
    assert!(said(&m, "A MONITOR ON HDMI-A-2: STEPS 1 TO 4 RAN AGAIN ON IT, THE SECOND OUTPUT Lit"), "{:#?}", m.said);
    {
        let b = m.at.0.borrow();
        let second: Vec<Ev> = b.e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
        assert!(second.contains(&Ev::Offer(3440, 1440, 13824)));
        for x in &second { if let Ev::Write(o, _) = x { assert!(!hdmi_a_1s(*o), "HDMI-A-1's {:05X}", o); } }
        assert!(b.i2c.writes[i2c_from..].iter().all(|w| light::ddc_writable_on(&light::DDC2_WRITES, Reg::new(w.0).unwrap()).is_ok()));
        assert!(b.i2c.writes[i2c_from..].iter().any(|w| w.0 == I2C_PADS_2));
    }
    // Out of HDMI-A-2: every register as before step 3 again.
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [false, false], 2000, &mut out);
    assert_eq!(f.state(), hpd::State::Dark);
    as_before(&m.at.0.borrow().e, &before, &[]);
    // Back into HDMI-A-1: the same writes and offer as at the start.
    moved(&mut m, false);
    let from = m.at.0.borrow().e.ev.len();
    follow_2(&mut m, &mut w, &mut f, &plan, &table, [true, false], 9000, &mut out);
    assert_eq!((f.state(), f.output()), (hpd::State::Lit, Some(HDMI_A_1)));
    let b = m.at.0.borrow();
    let again: Vec<Ev> = b.e.ev[from..].iter().filter(|x| matches!(x, Ev::Write(..) | Ev::Offer(..) | Ev::Gone)).cloned().collect();
    assert_eq!(again, first);
}
