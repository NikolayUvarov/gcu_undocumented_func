// Host cases of display's step 3 (213-DRV-0086): first light's eight stages on a model of DCN 2.1's blocks: the pipe
// chosen from step 1's values, the guard's refusals, the order of the writes against the route, each stage's undo, a
// status that never comes, the firmware's pipe changed under a stage, a build that may run nothing, and the mode's
// timing against the CTA table. The tables are synthetic VBIOS tables here; the Ryzen's own run when MIND_AMD_VBIOS
// names its image (the vendor's, not in the repository).
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;

use batches::{Bus, Out, Store};
use dcn::{Reg, Step, R};
// Step 3's stages (first light); step 4's come after them (213-DRV-0092).
const STAGES: u8 = dcn::FIRST_LIGHT;
use light::{assess, Canvas, Choice, Clock, Failure, Light, Outcome, Poke, Refusal, Refused, Writable, EXPECTED, HDMI_A_1};
use std::collections::HashMap;
use std::fmt;

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
    assert_eq!(refuse(&|s| { s.set(R::DigBeEnCntl, 4, 0); s.set(R::DigBeEnCntl, 0, 1); }), Refusal::HdmiDriven);
    assert_eq!(refuse(&|s| s.set(R::HpdIntStatus, 0, 0)), Refusal::NoMonitor);
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
enum Ev { Line(String), Write(u32, u32), Read(u32) }

#[derive(Default)]
struct Engine {
    regs: HashMap<u32, u32>, clock: u64, ev: Vec<Ev>, frames: u32, frames_move: bool,
    // After a write to the first offset, the model sets the second to the third: the firmware's pipe changed by the hardware.
    hook: Option<(u32, u32, u32)>,
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
    surface: Vec<u32>, no_surface: bool,
}
impl Canvas for Engine {
    fn row(&mut self, y: usize, pixels: &[u32]) -> bool {
        if self.no_surface || y >= picture::HEIGHT { return false; }
        if self.surface.is_empty() { self.surface = vec![0; picture::PITCH * picture::HEIGHT]; }
        self.surface[y * picture::PITCH..][..pixels.len()].copy_from_slice(pixels);
        true
    }
    fn sample(&mut self, x: usize, y: usize) -> Option<u32> { self.surface.get(y * picture::PITCH + x).copied() }
}
impl Bus for Engine {
    fn read(&mut self, r: Reg) -> Option<u32> {
        self.clock += 10;
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
        for &(at, bits, src, mask) in &self.derived {
            if at == o { let s = if src == o { *self.regs.get(&o).unwrap_or(&0) } else { *self.regs.get(&src).unwrap_or(&0) }; v = if s & mask != 0 { v | bits } else { v & !bits }; }
        }
        Some(v)
    }
}
impl Poke for Engine {
    fn write(&mut self, w: Writable, v: u32) {
        let o = w.reg().offset();
        self.ev.push(Ev::Write(o, v));
        // The status bits that follow their requests: clocks on, the generator running.
        let v = if (0..4).any(|t| reg(R::OptcInputClockControl, t) == o) { (v & !4) | ((v >> 1) & 1) << 2 }
            else if (0..4).any(|t| reg(R::OtgClockControl, t) == o) { (v & !0x100) | (v & 1) << 8 }
            else if (0..4).any(|t| reg(R::OtgControl, t) == o) { (v & !0x1_0000) | (v & 1) << 16 } else { v };
        self.regs.insert(o, v);
        if let Some((at, target, value)) = self.hook { if at == o { self.regs.insert(target, value); } }
        for (at, status) in self.cut.clone() { if at == o { self.set.retain(|s| s.0 != status); self.follow.retain(|s| s.0 != status); self.derived.retain(|s| s.0 != status); } }
        if let Some((at, n)) = self.fail { if at == o { self.armed = Some(n); self.fail = None; } }
    }
}
impl Clock for Engine {
    fn now_us(&mut self) -> u64 { self.clock }
    fn delay_us(&mut self, us: u32) { self.clock += us as u64; }
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
        mpc_free: true, vready: 1, copies }
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
    assert!(log.iter().any(|x| x == "S9 PIPE 0 BY FORMULA: PER HTOTAL 259335 (SET 259335), H_BLANK_END 41 (SET 41), LINE 953 (SET 953), V_BLANK_END 43 (DLG 43)"), "{:#?}", log.iter().filter(|x| x.starts_with("S9")).collect::<Vec<_>>());
    // Pipe 1's values for 1080p60 at the reference clock found.
    let otg = timing::CEA_1080P60.otg().unwrap();
    let ratio1 = (reference << 19) / 148_500;
    let line = format!("S9 PIPE 1 FOR 1920x1080 AT 148500 KHZ: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, LINE {}",
        batches::Ratio19(Some(ratio1 as u32)), (ratio1 * (otg.h_total as u64 + 1) * 256) >> 19, (ratio1 * otg.h_blank_end as u64) >> 19,
        otg.v_blank_end, (ratio1 * 1920) >> 19);
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
    let lines = picture::Lines { build: "TESTBUILD" };
    for y in (0..picture::HEIGHT).step_by(7) {
        for x in (0..picture::WIDTH).step_by(5) { assert_eq!(e.sample(x, y), Some(picture::pixel(&lines, x, y)), "{},{}", x, y); }
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
    assert_eq!(g(R::HubpCntl, 1) & 0x1101, 0x100, "unblanked, TTU on, VREADY as the firmware's");
    // Its DLG and TTU as stage 9 derived them; the rest copied from pipe 0.
    assert_eq!(g(R::HubpreqRefFreqToPixFreq, 1), ((number_after(&log, "S9 OTG0 COUNTED", "THE REFERENCE CLOCK ") << 19) / 148_500) as u32);
    for (i, reg) in light::COPIED.iter().enumerate().filter(|(_, r)| **r != R::MpccControl) {
        assert_eq!(g(*reg, 1), r.copies[i], "{}", reg.name());
    }
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
    assert!(log.iter().any(|x| x == "S13 PITCH FIELD 1919 (1920 PIXELS LESS ONE, AS DISPLAY CORE WRITES IT); THE FIRMWARE'S HUBP0 HOLDS 2560"));
    assert_eq!(log.iter().filter(|x| x.starts_with("S15 NO NEW UNDERFLOW: HUBP0 AND ODM0 (THE FIRMWARE'S), HUBP1 AND ODM1")).count(), 2);
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
}

#[test]
fn an_underflow_at_stage_15_or_a_busy_mpcc_fails_and_is_undone_whole() {
    // HUBP1's underflow status set once it is set up: stage 15 sees it new, fails, and undoes 15 to 1.
    let l = step4(Some(ryzen_reference()));
    let mut e = engine();
    e.frame_us = 16_675;
    e.hook = Some((reg(R::HubpCntl, 1), reg(R::HubpCntl, 1), 1 << 28));
    let before = e.regs.clone();
    let (outcome, log) = run_light(&l, &good(), &mut e, 15, false);
    match outcome { Outcome::Failed { stage: 15, why: Failure::Underflow { offset, .. } } if offset == reg(R::HubpCntl, 1) => {}, o => panic!("{:?} {:#?}", o, &log[log.len().saturating_sub(12)..]) }
    assert!(log.iter().any(|x| x == "STEP 3 FAILED AT STAGE 15 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED"));
    restored_but(&e, &before, &[reg(R::HubpCntl, 1)]);
    assert_eq!(e.regs[&reg(R::DpgControl, 1)] & 1, before.get(&reg(R::DpgControl, 1)).copied().unwrap_or(0) & 1);
    // Pipe 1's MPCC in use: stage 9 fails as it builds stages 11 to 15, before any of pipe 1 is written.
    let l = step4(Some(light::Reference { mpc_free: false, ..ryzen_reference() }));
    let mut e = engine();
    e.frame_us = 16_675;
    let (outcome, _) = run_light(&l, &good(), &mut e, 15, false);
    assert_eq!(outcome, Outcome::Failed { stage: 9, why: Failure::PipeNotFree });
    assert!(!writes(&e).iter().any(|w| w.0 == reg(R::HubpCntl, 1) || w.0 == reg(R::MpccTopSel, 1)));
    // A build of 0 names stages 11 to 15 with what they will do.
    let mut log = Log(vec![]);
    light::describe(&l, false, &mut log);
    assert!(log.0.iter().any(|x| x.starts_with("S13 WOULD SET HUBP1, BLANKED: THE SURFACE AT FB + 0x1000000, PITCH 1919")));
}

#[test]
fn stage_9_without_a_reference_and_stage_10_without_a_surface_fail_and_are_undone_whole() {
    let mut e = engine();
    e.frame_us = 16_675;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&step4(None), &good(), &mut e, 10, false);
    assert_eq!(outcome, Outcome::Failed { stage: 9, why: Failure::NoReference });
    assert!(log.iter().any(|x| x == "STEP 3 FAILED AT STAGE 9 AND WAS UNDONE; THE SECOND PIPE IS OFF; THE FIRMWARE'S PIPE UNCHANGED"));
    restored_but(&e, &before, &[]);
    let mut e = engine();
    e.frame_us = 16_675;
    e.no_surface = true;
    let before = e.regs.clone();
    let (outcome, log) = run_light(&step4(Some(ryzen_reference())), &good(), &mut e, 10, false);
    assert_eq!(outcome, Outcome::Failed { stage: 10, why: Failure::NoSurface });
    assert!(log.iter().any(|x| x.starts_with("S10 DRAW FAILED: SLOT 5'S SURFACE IS NOT MAPPED")));
    restored_but(&e, &before, &[]);
    // Step 1's values give the reference only when all its batches were read and they name the firmware's pipe.
    let (f, c) = EXPECTED;
    let mut s = found();
    assert_eq!(light::reference(&s, &f, &c), None, "found() holds no batch 9 values");
    let want = ryzen_reference();
    for (r, v) in [(R::OtgHTotal, 2719), (R::OtgVTotal, 1645), (R::OtgHBlankStartEnd, 112 << 16 | 2672), (R::OtgVBlankStartEnd, 43 << 16 | 1643),
        (R::HubpPriViewportDimension, 1600 << 16 | 2560), (R::HubpreqRefFreqToPixFreq, 195_264), (R::HubpreqDstDimensions, 259_335),
        (R::HubpreqBlankOffset0, 43 << 16 | 41), (R::HubpreqPerLineDelivery, 953), (R::HubpreqSurf0TtuCntl0, 24_396), (R::HubpreqSurfacePitch, 0xA00),
        (R::HubpreqPerLineDeliveryPre, 953), (R::HubpreqSurf0TtuCntl1, 24_396), (R::DcnVmFbLocationBase, 0xF400)] {
        s.set(r, 0, v);
    }
    for (r, v) in light::COPIED.iter().zip(want.copies) { s.set(*r, 0, v); }
    s.set(R::HubpCntl, 0, 0x000F_0102); // HUBP_VREADY_AT_OR_AFTER_VSYNC 1
    s.set(R::MpccTopSel, 1, 0xF); s.set(R::MpcOutMux, 1, 0xF);
    assert_eq!(light::reference(&s, &f, &c), Some(want));
    s.set(R::MpccTopSel, 1, 1);
    assert_eq!(light::reference(&s, &f, &c).map(|r| r.mpc_free), Some(false), "pipe 1's MPCC in use");
    s.set_batches(9);
    assert_eq!(light::reference(&s, &f, &c), None, "batch 10 not read");
}

#[test]
fn step_3_runs_once_a_boot_by_the_mark_it_leaves_in_its_otg() {
    let mark = reg(R::OtgVerticalInterrupt2Position, 1);
    // The first start leaves the mark before stage 1 and keeps it through the undo.
    let mut e = engine();
    let (outcome, log) = run(&good(), &mut e, 1);
    assert_eq!(outcome, Outcome::Stopped { done: 1 });
    assert_eq!(writes(&e)[0], (mark, light::RESTART_MARK));
    assert!(log.iter().any(|x| x == "S0 MARKED OTG1_OTG_VERTICAL_INTERRUPT2_POSITION AT 14298 = 5A5A (WAS 00000000): A START OF DISPLAY AGAIN IN THIS BOOT RUNS NO STAGE; A MACHINE BOOT CLEARS IT"), "{:#?}", log);
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
    assert!(log.iter().any(|x| x == "STEP 3 NOT RUN: DISPLAY HAS STARTED STEP 3 ALREADY IN THIS BOOT (THE MARK IN ITS OTG): THE STAGES RUN ONCE A BOOT (OTG1_OTG_VERTICAL_INTERRUPT2_POSITION AT 14298 HOLDS 5A5A)"), "{:#?}", log);
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
