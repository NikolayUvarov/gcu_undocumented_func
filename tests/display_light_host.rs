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

use batches::{Bus, Out, Store};
use dcn::{Reg, Step, R, STAGES};
use light::{assess, Choice, Clock, Failure, Light, Outcome, Poke, Refusal, Refused, Writable, EXPECTED, HDMI_A_1};
use std::collections::HashMap;
use std::fmt;

fn seg(s: usize, dword: u32) -> u32 { (dcn::SEGMENT[s] + dword) * 4 }
fn reg(r: R, i: u8) -> u32 { r.at(i).unwrap().offset() }

// --- step 1's values ---

// What step 1 would find if the GOP drives DP-2 as the survey expects: OTG 0 from OPP 0, HUBP 0 on it, DIG front end E
// into PHY E's enabled back end, HPD 1 sensing HDMI-A-1's monitor, every pipe's domains on.
fn found() -> Store {
    let mut s = Store::default();
    for n in 0..5u8 { s.set(R::HpdIntStatus, n, if n == 0 || n == 4 { 0b10 } else { 0 }); s.set(R::HpdControl, n, 1 << 28); }
    for t in 0..4u8 {
        s.set(R::OtgControl, t, if t == 0 { 0x0001_0001 } else { 0 });
        s.set(R::OptcDataSourceSelect, t, 0);
        s.set(R::OtgClockControl, t, if t == 0 { 0x101 } else { 0 });
    }
    for d in 0..8u8 { s.set(R::DomainPgStatus, d, 0); }
    for n in 0..4u8 { s.set(R::HubpCntl, n, if n == 0 { 0 } else { 0xF << 4 }); }
    for d in 0..5u8 { s.set(R::DigFeCntl, d, if d == 4 { 1 << 24 } else { 7 }); }
    for p in batches::BOARD_PHYS {
        s.set(R::DigBeCntl, p, if p == 4 { (1 << 4) << 8 } else { 0 });
        s.set(R::DigBeEnCntl, p, if p == 4 { 0x101 } else { 0 });
    }
    s.set_batches(5);
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
    let shared = Choice { pipe: dcn::Pipe { otg: 1, opp: 1, dig_fe: 4 }, output: HDMI_A_1 };
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
}
impl Bus for Engine {
    fn read(&mut self, r: Reg) -> Option<u32> {
        self.clock += 10;
        let o = r.offset();
        if (0..4).any(|t| reg(R::OtgStatusFrameCount, t) == o) && self.frames_move { self.frames += 1; return Some(self.frames); }
        let mut v = *self.regs.get(&o).unwrap_or(&0);
        for &(at, bits) in &self.set { if at == o { v |= bits; } }
        for &(at, ack) in &self.follow { if at == o { v = (v & !ack) | ((v & (ack >> 1)) << 1); } }
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
    assert!(log.last().unwrap().starts_with("STEP 3 STOPPED AFTER STAGE 8 OF 8"));
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
        if except.contains(&o) { continue; }
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
    assert_eq!(lines.last().unwrap(), "STEP 3 DONE: 0 OF 8 STAGES RUN, NOTHING WRITTEN");
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
    let answered = || {
        let mut e = engine();
        let phy = |d: u32| seg(2, d);
        e.set.push((phy(0x2940), 1 << 28)); // RDPCS_SRAM_INIT_DONE
        e.set.push((phy(0x294C), 1 << 7)); // RDPCS_PHY_DP_MPLLB_STATE
        e.follow.push((phy(0x2943), 0x2020_2020)); // each lane's TX_ACK after its TX_REQ
        e.follow.push((phy(0x2928), 1 << 2)); // DPCS_SYMCLK_CLOCK_ON after DPCS_SYMCLK_EN
        e.follow.push((phy(0x2931), 1 << 10)); // RDPCS_SYMCLK_DIV2_CLOCK_ON after its EN
        e
    };
    let mut e = answered();
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Stopped { done: 8 }, "{:#?}", &log[log.len().saturating_sub(20)..]);
    for (w, _) in writes(&e) { assert!(l.guard().check(Reg::new(w).unwrap()).is_ok(), "{:05X}", w); }
    for o in watched(&l) { assert_eq!(e.regs.get(&o), before.get(&o), "{:05X}", o); }
    for t in [12, 4, 76] { assert!(log.iter().any(|x| x.contains(&format!("TABLE {} DONE", t))), "table {}", t); }
    // The frame count held still: stage 5 fails, and the undo restores every register stages 1 to 4 wrote, saying how
    // many of table 12's writes went through PHY A's control register bus, which a restore does not reach.
    let mut e = answered();
    e.frames_move = false;
    let before = e.regs.clone();
    let (outcome, log) = run(&bytes, &mut e, STAGES);
    assert_eq!(outcome, Outcome::Failed { stage: 5, why: Failure::NoFrames(0) });
    restored_but(&e, &before, &[]);
    assert!(log.iter().any(|x| x.starts_with("U2 ") && x.contains("WENT THROUGH PHY A'S CONTROL REGISTER BUS")), "{:#?}", log.iter().filter(|x| x.starts_with('U') && !x.contains(" RESTORED ")).collect::<Vec<_>>());
    for x in log.iter().filter(|x| x.starts_with('U') && !x.contains(" RESTORED ") && !x.contains(" WROTE ")) { eprintln!("{}", x); }
}
