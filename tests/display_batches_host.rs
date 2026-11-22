// Host cases of display's step 1 (213-DRV-0086): the eight batches' registers, the order of names, pauses and reads,
// how many batches a build reads, batch 4's dependence on batch 3, and the decoding of the state Linux's Display Core
// dumped on the Ryzen on 2026-11-21 (hwdocs/cezanne-display.md).
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;

use batches::{plan, run, Bus, Known, Out, BATCHES, PAUSE_BEFORE_READ_MS, PAUSE_BETWEEN_BATCHES_MS};
use dcn::{Reg, R};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
enum Event { Line(String), Pause(u32), Read(u32) }

type Events = Rc<RefCell<Vec<Event>>>;

// Registers as a map (absent ones read as 0), every read recorded with the lines and pauses in one order.
struct Model { regs: HashMap<u32, u32>, events: Events, unmapped: BTreeSet<u32> }
impl Bus for Model {
    fn read(&mut self, reg: Reg) -> Option<u32> {
        self.events.borrow_mut().push(Event::Read(reg.offset()));
        if self.unmapped.contains(&reg.offset()) { return None; }
        Some(*self.regs.get(&reg.offset()).unwrap_or(&0))
    }
}
struct Log(Events);
impl Out for Log {
    fn line(&mut self, args: fmt::Arguments) { self.0.borrow_mut().push(Event::Line(args.to_string())); }
    fn pause(&mut self, ms: u32) { self.0.borrow_mut().push(Event::Pause(ms)); }
}

fn reg(r: R, inst: u8) -> u32 { r.at(inst).unwrap().offset() }

// The state Linux left with both monitors lit (amdgpu_dm_dtn_log and framebuffer.txt of 2026-11-21): OTG 0 for
// HDMI-A-1 from OPP 0, OTG 1 for DP-2 from OPP 1, OTGs 2 and 3 off; all four pipes powered; HUBPs of format 8 below 4 GiB.
fn linux_state() -> HashMap<u32, u32> {
    let mut m = HashMap::new();
    // v_bs v_be v_se vtot vmin vmax vpol h_bs h_be h_se htot hpol, as the dump prints them
    let otgs = [(1497, 57, 5, 1499, 1499, 1499, 1, 3552, 112, 32, 3599, 0), (1643, 43, 10, 1645, 1645, 1645, 1, 2672, 112, 32, 2719, 0)];
    for (n, &(v_bs, v_be, v_se, vtot, vmin, vmax, vpol, h_bs, h_be, h_se, htot, hpol)) in otgs.iter().enumerate() {
        let n = n as u8;
        m.insert(reg(R::OtgControl, n), 0x0001_0001);
        m.insert(reg(R::OtgHTotal, n), htot);
        m.insert(reg(R::OtgHBlankStartEnd, n), h_bs | h_be << 16);
        m.insert(reg(R::OtgHSyncA, n), h_se << 16);
        m.insert(reg(R::OtgHSyncACntl, n), hpol);
        m.insert(reg(R::OtgVTotal, n), vtot);
        m.insert(reg(R::OtgVTotalMin, n), vmin);
        m.insert(reg(R::OtgVTotalMax, n), vmax);
        m.insert(reg(R::OtgVBlankStartEnd, n), v_bs | v_be << 16);
        m.insert(reg(R::OtgVSyncA, n), v_se << 16);
        m.insert(reg(R::OtgVSyncACntl, n), vpol);
        m.insert(reg(R::OtgClockControl, n), 0x101);
        m.insert(reg(R::OptcDataSourceSelect, n), (n as u32) << 8);
    }
    // HUBPs 0 and 3 (3440 wide, pitch 3456) feed OPP 0, 1 and 2 (pitch 2560) OPP 1; format 8h, high address 0.
    for (n, pitch) in [(0u8, 3456u32), (1, 2560), (2, 2560), (3, 3456)] {
        m.insert(reg(R::HubpSurfaceConfig, n), 8);
        m.insert(reg(R::HubpreqSurfacePitch, n), pitch - 1);
        m.insert(reg(R::HubpreqPrimarySurfaceAddress, n), 0x0040_0000 + n as u32 * 0x0200_0000);
        m.insert(reg(R::HubpreqPrimarySurfaceAddressHigh, n), 0);
    }
    m
}

// The firmware's state as step 1 found it on the Ryzen: OTG0 alone runs, HUBP0 follows it, HUBPs 1 to 3 blanked on
// their own idle OTGs, every domain on (213-DRV-0086, runs 3 to 7); pipe 0's reference values from the GOP's 2560x1600.
fn firmware_state() -> HashMap<u32, u32> {
    let mut m = HashMap::new();
    m.insert(reg(R::OtgControl, 0), 0x8001_1301);
    m.insert(reg(R::OtgClockControl, 0), 0x0001_0101);
    for n in 1..4u8 { m.insert(reg(R::OtgControl, n), 0x8000_0300); m.insert(reg(R::OptcDataSourceSelect, n), 0xF000 | (n as u32) << 8); }
    m.insert(reg(R::OptcDataSourceSelect, 0), 0xF000);
    for d in 0..8u8 { m.insert(reg(R::DomainPgStatus, d), 0x1000_0000); }
    m.insert(reg(R::HubpCntl, 0), 0x000F_0002);
    for n in 1..4u8 { m.insert(reg(R::HubpCntl, n), 0x000F_000B | (n as u32) << 4); }
    m.insert(reg(R::HubpreqPrimarySurfaceAddressHigh, 0), 0xF4);
    m.insert(reg(R::HubpPriViewportDimension, 0), 1600 << 16 | 2560);
    m.insert(reg(R::HubpreqRefFreqToPixFreq, 0), 195_225); // 100 / 268.5 x 2^19
    m.insert(reg(R::HubpreqDstDimensions, 0), 259_336); // 2720 x 100 / 268.5 x 2^8
    m.insert(reg(R::MpccTopSel, 0), 0); m.insert(reg(R::MpccOppId, 0), 0); m.insert(reg(R::MpcOutMux, 0), 0);
    for n in 1..4u8 { m.insert(reg(R::MpccTopSel, n), 0xF); m.insert(reg(R::MpccBotSel, n), 0xF); m.insert(reg(R::MpccOppId, n), 0xF); m.insert(reg(R::MpcOutMux, n), 0xF); }
    m.insert(reg(R::DppControl, 0), 0x10);
    m.insert(reg(R::DppclkDtoParam, 1), 0x0064_0064);
    m
}

#[test]
fn batches_9_and_10_read_the_firmwares_pipe_and_what_every_pipe_shares() {
    let (report, events) = run_on(firmware_state(), BATCHES, &[]);
    assert_eq!(report.batches_read, 10);
    let read: BTreeSet<u32> = reads(&events).into_iter().collect();
    // Pipe 0's HUBP and DPP blocks, never pipe 1's to 3's; the MPC tree and the DPP clocks of all four.
    for r in [R::HubpreqRefFreqToPixFreq, R::HubpReqSizeConfig, R::DppControl, R::DsclRecoutSize] {
        assert!(read.contains(&reg(r, 0)), "{}", r.name());
        for n in 1..4 { assert!(!read.contains(&reg(r, n)), "{}{}", r.name(), n); }
    }
    for n in 0..4 { assert!(read.contains(&reg(R::MpccTopSel, n)) && read.contains(&reg(R::DppclkDtoParam, n)) && read.contains(&reg(R::MpcOutMux, n))); }
    for n in 0..20 { assert!(read.contains(&reg(R::DchubbubArbWatermark, n))); }
    // At the header's offsets (dcn_2_1_0_offset.h): segment 2 unless said.
    let s2 = |d: u32| (0x34C0 + d) * 4;
    for (r, i, o) in [(R::HubpreqRefFreqToPixFreq, 0, s2(0x65F)), (R::HubpretControl, 0, s2(0x66C)), (R::HubpClkCntl, 1, s2(0x5F4 + 0xDC)),
        (R::DppControl, 1, s2(0xE30)), (R::MpccTopSel, 1, s2(0x128C)), (R::MpcOutMux, 3, s2(0x1391)), (R::DchubbubArbWatermark, 19, s2(0x51C)),
        (R::DchubbubGlobalTimerCntl, 0, s2(0x51F)), (R::DppclkDtoParam, 1, (0xC0 + 0x9A) * 4), (R::DppclkDtoCtrl, 0, (0xC0 + 0xB6) * 4)] {
        assert_eq!(reg(r, i), o, "{} {}", r.name(), i);
    }
    let l = lines(&events);
    let has = |s: &str| assert!(l.iter().any(|x| x.contains(s)), "no line with {:?}: {:#?}", s, l.iter().filter(|x| x.starts_with("B9") || x.starts_with("B10")).collect::<Vec<_>>());
    has("B9 HUBP0: VIEWPORT 2560x1600 AT 0,0");
    has("B9 HUBP0 DLG: REF/PIX 0.3723, PER HTOTAL 259336");
    has("B9 OTG0: VSTARTUP 0");
    has("B10 DPP0: CLOCK 1, FORMAT 0");
    has("B10 MPC: MPCC0 TOP 0 BOT 0 OPP 0 MODE 0 ALPHA 0; MPCC1 TOP 15 BOT 15 OPP 15");
    has("B10 MPC_OUT MUX: 0:0 1:15 2:15 3:15 (15: NONE)");
    has("B10 DCCG: DPPCLK DTO CTRL 00000000; DTO0 0/0 DTO1 100/100");
    // Linux's state runs two OTGs: nothing to read, and why.
    let (_, events) = run_on(linux_state(), BATCHES, &[]);
    assert!(lines(&events).iter().any(|x| x.starts_with("B9 PIPE REFERENCE: NOTHING TO READ: NOT ONE RUNNING OTG")));
}

fn run_on(regs: HashMap<u32, u32>, allowed: u8, unmapped: &[u32]) -> (batches::Report, Vec<Event>) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let mut model = Model { regs, events: events.clone(), unmapped: unmapped.iter().copied().collect() };
    let report = run(&mut model, allowed, &mut Log(events.clone()));
    let out = events.borrow().clone();
    (report, out)
}

fn lines(events: &[Event]) -> Vec<&str> { events.iter().filter_map(|e| if let Event::Line(l) = e { Some(l.as_str()) } else { None }).collect() }
fn reads(events: &[Event]) -> Vec<u32> { events.iter().filter_map(|e| if let Event::Read(o) = e { Some(*o) } else { None }).collect() }

fn all_powered() -> Known { Known { hubp_powered: Some([true; 4]), ..Default::default() } }
// What batches 2 to 4 of the Ryzen's state show: OTG0 runs, HUBP0 follows it.
fn firmware_known() -> Known { Known { hubp_powered: Some([true; 4]), running_otg: Some(0), firmware_pipe: Some(0) } }

#[test]
fn the_batches_are_the_proposal_and_stay_in_the_engine() {
    let counts: Vec<usize> = (1..=BATCHES).map(|b| plan(b, &firmware_known()).len()).collect();
    assert_eq!(counts, vec![10, 76, 8, 20, 13, 6, 14, 31, 43, 68]);
    // Batches 9 and 10 only once batches 2 and 4 named the firmware's pipe.
    assert!(plan(9, &all_powered()).is_empty() && plan(10, &all_powered()).is_empty());
    for b in 1..=BATCHES {
        for n in plan(b, &firmware_known()).iter() {
            assert!(dcn::DCN_BYTES.contains(&n.reg.offset()) && !dcn::MSIX_BYTES.contains(&n.reg.offset()), "{}", n);
        }
    }
    let dwords = |b: u8| plan(b, &all_powered()).iter().map(|n| n.reg.offset() / 4).collect::<Vec<_>>();
    // Batch 3: DOMAIN0..7_PG_STATUS, segment 2 dwords 0x81 + 2n; batch 6: DCN_VM_* at segment 2 dwords 0x493 to 0x498.
    assert_eq!(dwords(3), (0..8).map(|n| 0x34C0 + 0x81 + 2 * n).collect::<Vec<_>>());
    assert_eq!(dwords(6), (0x493..=0x498).map(|d| 0x34C0 + d).collect::<Vec<_>>());
    // Batch 1: HPD1..5 (0x1F14 + 8n, 0x1F16 + 8n); batch 5's back ends only for PHYs A, B, D and E.
    assert_eq!(dwords(1)[..4], [0x34C0 + 0x1F14, 0x34C0 + 0x1F16, 0x34C0 + 0x1F1C, 0x34C0 + 0x1F1E]);
    let be: Vec<u8> = plan(5, &all_powered()).iter().filter(|n| n.r == R::DigBeCntl).map(|n| n.inst).collect();
    assert_eq!(be, vec![0, 1, 3, 4]);
    // Batch 4: HUBPREQ0_DCSURF_PRIMARY_SURFACE_ADDRESS (0x60A) and HUBP3_DCHUBP_CNTL (0x887) from the header.
    assert!(dwords(4).contains(&(0x34C0 + 0x60A)) && dwords(4).contains(&(0x34C0 + 0x887)));
    // Before batch 3, batch 4 reads nothing; only the HUBPs shown powered after it.
    assert!(plan(4, &Known::default()).is_empty());
    let some = Known { hubp_powered: Some([true, false, true, false]), ..Default::default() };
    assert_eq!(plan(4, &some).iter().map(|n| n.inst).collect::<BTreeSet<_>>(), [0u8, 2].into_iter().collect());
}

#[test]
fn a_build_that_may_read_nothing_reads_nothing_and_names_every_batch() {
    let (report, events) = run_on(linux_state(), 0, &[]);
    assert_eq!(report, batches::Report { batches_read: 0, registers_read: 0 });
    assert!(reads(&events).is_empty());
    let l = lines(&events);
    assert!(l[0].contains("THIS BUILD MAY READ 0"));
    for b in 1..=BATCHES { assert!(l.iter().any(|x| x.starts_with(&format!("B{} ", b)) && x.contains("NOT READ")), "batch {}", b); }
    let would = |b: u8| l.iter().filter(|x| x.starts_with(&format!("B{} WOULD READ ", b))).count();
    assert_eq!((1..=BATCHES).map(would).collect::<Vec<_>>(), vec![10, 76, 8, 20, 13, 6, 14, 31, 43, 68]);
    assert!(l.iter().any(|x| x.starts_with("B4 HUBP: NOT READ") && x.contains("ONLY FOR EACH HUBP n THAT B3 SHOWS POWERED")));
    assert!(l.last().unwrap().starts_with("STEP 1 DONE: 0 OF 10 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN"));
    assert!(!events.iter().any(|e| matches!(e, Event::Pause(_))));
}

#[test]
fn a_build_reads_its_allowed_batches_and_no_register_of_the_others() {
    let state = linux_state();
    for allowed in 0..=BATCHES {
        let (report, events) = run_on(state.clone(), allowed, &[]);
        // Linux's state runs two OTGs: batches 9 and 10 have no firmware's pipe to read, and do not count as read.
        assert_eq!(report.batches_read, allowed.min(8));
        let mut expected = Vec::new();
        let mut known = Known::default();
        for b in 1..=allowed {
            if b == 4 { known = all_powered(); } // batch 3 of this state shows every HUBP powered
            expected.extend(plan(b, &known).iter().map(|n| n.reg.offset()));
        }
        assert_eq!(reads(&events), expected, "allowed {}", allowed);
        assert_eq!(report.registers_read, expected.len());
    }
    // A number beyond ten reads the ten (Linux's state: the eight with registers to read).
    assert_eq!(run_on(linux_state(), 200, &[]).0.batches_read, 8);
    assert_eq!(run_on(firmware_state(), 200, &[]).0.batches_read, 10);
}

#[test]
fn each_batch_is_named_and_paused_before_its_first_read_and_paused_after() {
    let (_, events) = run_on(firmware_state(), BATCHES, &[]);
    for b in 1..=BATCHES {
        let name = events.iter().position(|e| matches!(e, Event::Line(l) if l.starts_with(&format!("B{} ", b)) && l.contains("REGISTERS NEXT"))).unwrap();
        let first = plan(b, &firmware_known()).iter().next().unwrap().reg.offset();
        let read = name + events[name..].iter().position(|e| *e == Event::Read(first)).unwrap();
        assert!(events[name + 1..read].contains(&Event::Pause(PAUSE_BEFORE_READ_MS)), "batch {}", b);
        assert!(!events[..name].contains(&Event::Read(first)), "batch {} read before it was named", b);
    }
    // Each value is logged right after its read, and a batch ends with the long pause before the next is named.
    for (i, e) in events.iter().enumerate() {
        if let Event::Read(o) = e { assert!(matches!(&events[i + 1], Event::Line(l) if l.contains(&format!("AT {:05X} = ", o)))); }
    }
    assert_eq!(events.iter().filter(|e| **e == Event::Pause(PAUSE_BETWEEN_BATCHES_MS)).count(), 10);
}

#[test]
fn the_linux_state_decodes_as_display_core_dumped_it() {
    let (_, events) = run_on(linux_state(), BATCHES, &[]);
    let l = lines(&events);
    let has = |s: &str| assert!(l.iter().any(|x| x.contains(s)), "no line with {:?}", s);
    has("B2 OTG0: RUNNING 1, CLOCK_ON 1, FROM OPP 0; 3440x1440 TOTAL 3600x1500 H+ V-");
    has("B2 OTG1: RUNNING 1, CLOCK_ON 1, FROM OPP 1; 2560x1600 TOTAL 2720x1646 H+ V-");
    has("B2 OTG2: RUNNING 0, CLOCK_ON 0, FROM OPP 0; NO TIMING IN ITS REGISTERS");
    has("B3 PIPE 3: HUBP3 ON, DPP3 ON");
    has("B4 HUBP0: SURFACE 0000_00400000, FORMAT 8, PITCH 3456, ROTATION 0, BLANK 0, TTU_DISABLE 0");
    has("B4 HUBP1: SURFACE 0000_02400000, FORMAT 8, PITCH 2560");
    has("B2 OTG0_OTG_H_TOTAL AT 13FA8 = 00000E0F"); // (0x34C0 + 0x1B2A) x 4
    has("B6 APERTURE: FB 0..0");
    has("B7 DENTIST: DISPCLK WDIVIDER 0, RDIVIDER 0");
    // The OTG values are the monitors' EDID timings (tests/display_host.rs checks those both ways).
    let mut otg0 = vec![];
    for r in [R::OtgHTotal, R::OtgHBlankStartEnd, R::OtgHSyncA, R::OtgVTotal, R::OtgVBlankStartEnd, R::OtgVSyncA, R::OtgHSyncACntl, R::OtgVSyncACntl] {
        otg0.push((r, linux_state()[&reg(r, 0)]));
    }
    let t = batches::otg_state(&otg0).unwrap().timing(324_000).unwrap();
    assert_eq!((t.h_active, t.v_active, t.refresh_millihertz()), (3440, 1440, 60_000));
}

#[test]
fn batch_4_reads_only_the_hubps_batch_3_shows_powered() {
    let mut state = linux_state();
    state.insert(reg(R::DomainPgStatus, 4), 2 << 30); // HUBP2 off
    state.insert(reg(R::DomainPgStatus, 6), 2 << 30); // HUBP3 off
    let (_, events) = run_on(state, BATCHES, &[]);
    let read: BTreeSet<u32> = reads(&events).into_iter().collect();
    for n in 0..4u8 {
        let hubp = [R::HubpreqPrimarySurfaceAddress, R::HubpreqPrimarySurfaceAddressHigh, R::HubpSurfaceConfig, R::HubpreqSurfacePitch, R::HubpCntl];
        for r in hubp { assert_eq!(read.contains(&reg(r, n)), n < 2, "HUBP{} {}", n, r.name()); }
    }
    let l = lines(&events);
    assert!(l.contains(&"B3 PIPE 2: HUBP2 OFF, DPP2 ON") && l.contains(&"B4 HUBP: READING 10 REGISTERS NEXT"));
    // None powered: batch 4 reads nothing and says why.
    let mut dark = linux_state();
    for d in 0..8 { dark.insert(reg(R::DomainPgStatus, d), 2 << 30); }
    let (_, events) = run_on(dark, BATCHES, &[]);
    assert!(lines(&events).iter().any(|x| x.starts_with("B4 HUBP: NOTHING TO READ")));
    assert!(!reads(&events).contains(&reg(R::HubpCntl, 0)));
}

#[test]
fn a_register_the_grant_does_not_map_is_not_read_and_batch_4_waits_on_all_of_batch_3() {
    let (report, events) = run_on(linux_state(), BATCHES, &[reg(R::DomainPgStatus, 2)]);
    let l = lines(&events);
    assert!(l.iter().any(|x| x.starts_with("B3 DOMAIN2_PG_STATUS AT") && x.ends_with("NOT MAPPED, NOT READ")));
    assert!(l.contains(&"B3 PIPE 1: HUBP1 -, DPP1 ON"));
    assert!(l.iter().any(|x| x.starts_with("B4 HUBP: NOTHING TO READ")));
    assert_eq!(report.registers_read, 10 + 76 + 7 + 13 + 6 + 14 + 31);
}

// The DMCUB's registers with its timer counting at each read, for batch 8.
struct Counting { regs: HashMap<u32, u32>, timer: u32, events: Events }
impl Bus for Counting {
    fn read(&mut self, reg: Reg) -> Option<u32> {
        self.events.borrow_mut().push(Event::Read(reg.offset()));
        if reg.offset() == reg_of(R::DmcubTimerCurrent, 0) { self.timer += 0x1000; return Some(self.timer); }
        Some(*self.regs.get(&reg.offset()).unwrap_or(&0))
    }
}
fn reg_of(r: R, i: u8) -> u32 { reg(r, i) }

#[test]
fn batch_8_tells_a_running_dmcub_from_one_held_in_reset() {
    // Its registers lie inside the engine's window, segment 2 dwords 0xCA and 0x3278 to 0x32A7 (dcn_2_1_0_offset.h).
    for n in plan(8, &all_powered()).iter() {
        let o = n.reg.offset();
        assert!(o < 0x1B000 && dcn::DCN_BYTES.contains(&o) && !dcn::MSIX_BYTES.contains(&o), "{}", n);
        assert!(o == (0x34C0 + 0xCA) * 4 || ((0x34C0 + 0x3278) * 4..=(0x34C0 + 0x32A7) * 4).contains(&o), "{}", n);
    }
    let names: Vec<String> = plan(8, &all_powered()).iter().map(|n| n.to_string()).collect();
    assert_eq!(names.first(), names.last(), "the timer is read first and last");
    assert!(names.iter().any(|x| x == "DMCUB_SCRATCH0 AT 19D34") && names.iter().any(|x| x == "DMCUB_SCRATCH15 AT 19D70"), "{:?}", names);
    let run8 = |regs: HashMap<u32, u32>, counts: bool| {
        let events: Events = Rc::new(RefCell::new(Vec::new()));
        if counts {
            let mut bus = Counting { regs, timer: 0, events: events.clone() };
            run(&mut bus, BATCHES, &mut Log(events.clone()));
        } else {
            let mut bus = Model { regs, events: events.clone(), unmapped: BTreeSet::new() };
            run(&mut bus, BATCHES, &mut Log(events.clone()));
        }
        let out = events.borrow().clone();
        lines(&out).iter().filter(|x| x.starts_with("B8 DMCUB: ") && !x.contains("REGISTERS NEXT")).map(|x| x.to_string()).collect::<Vec<_>>()
    };
    // As Linux leaves it: on the board, enabled, out of reset, counting, its mailbox set up, booted (SCRATCH0).
    let mut running = linux_state();
    running.insert(reg(R::CcDcPipeDis, 0), 1 << 16);
    running.insert(reg(R::DmcubCntl, 0), 1 << 16);
    running.insert(reg(R::DmcubScratch, 0), 0x8000_0003);
    running.insert(reg(R::DmcubInbox1BaseAddress, 0), 0x8000_0000);
    running.insert(reg(R::DmcubInbox1Size, 0), 0x1_0000);
    running.insert(reg(R::DmcubInbox1Rptr, 0), 0x40);
    running.insert(reg(R::DmcubInbox1Wptr, 0), 0x40);
    let l = run8(running, true);
    assert_eq!(l[0], "B8 DMCUB: ON THE BOARD 1, ENABLED 1, SOFT RESET 0, PWAIT 0; SECURE RESET 0, ITS STATUS 0; TIMER COUNTS (00002000)");
    assert_eq!(l[1], "B8 DMCUB: SCRATCH0 (BOOT STATUS) 80000003, SCRATCH7 (GPINT ANSWER) 00000000, SCRATCH10 00000000, GPINT IN 00000000 OUT 00000000");
    assert_eq!(l[2], "B8 DMCUB: INBOX1 AT 80000000 OF 65536 BYTES, READ 64 WRITE 64; INBOX0 READ 0 WRITE 0; OUTBOX1 READ 0 WRITE 0");
    // Held in reset, its timer still.
    let mut held = linux_state();
    held.insert(reg(R::CcDcPipeDis, 0), 1 << 16);
    held.insert(reg(R::DmcubCntl, 0), 1 << 17);
    let l = run8(held, false);
    assert!(l[0].starts_with("B8 DMCUB: ON THE BOARD 1, ENABLED 0, SOFT RESET 1") && l[0].ends_with("TIMER STILL (00000000)"), "{}", l[0]);
}

#[test]
fn every_line_fits_a_log_record() {
    // logd keeps records of 200 bytes; main adds "[DISPLAY] " (10).
    for allowed in [0, BATCHES] {
        for l in lines(&run_on(linux_state(), allowed, &[]).1) { assert!(l.len() <= 180, "{} bytes: {}", l.len(), l); }
    }
}

#[test]
fn every_register_of_the_map_is_named_once_in_the_engine_at_the_headers_offset() {
    let mut seen = BTreeSet::new();
    for r in dcn::ALL {
        assert!(seen.insert(r.name()), "{} twice", r.name());
        for i in 0..r.instances() {
            let o = r.at(i).unwrap().offset();
            assert!(dcn::DCN_BYTES.contains(&o) && !dcn::MSIX_BYTES.contains(&o), "{} {}", r.name(), i);
        }
        assert_eq!(r.at(r.instances()), None, "{}", r.name());
    }
    // dcn_2_1_0_offset.h (Linux v7.0): segment, dword, for the registers step 1 adds, at an instance past 0.
    let seg = [0x12u32, 0xC0, 0x34C0];
    let checks: [(R, u8, usize, u32); 24] = [
        (R::OtgVTotalControl, 1, 2, 0x1BB3), (R::OtgVerticalInterrupt1Position, 0, 2, 0x1B64),
        (R::OtgVerticalInterrupt1Control, 0, 2, 0x1B65), (R::OtgVerticalInterrupt2Position, 1, 2, 0x1BE6),
        (R::OptcInputGlobalControl, 1, 2, 0x1ADA), (R::DomainPgStatus, 7, 2, 0x8F),
        (R::HubpSurfaceConfig, 1, 2, 0x6C1), (R::HubpreqSurfacePitch, 1, 2, 0x6E3),
        (R::HubpreqPrimarySurfaceAddress, 3, 2, 0x89E), (R::HubpreqPrimarySurfaceAddressHigh, 0, 2, 0x60B),
        (R::HubpCntl, 3, 2, 0x887), (R::DigBeEnCntl, 4, 2, 0x24B0), (R::DigBeCntl, 4, 2, 0x24AF),
        (R::DigFeCntl, 4, 2, 0x2468), (R::HpdControl, 4, 2, 0x1F36), (R::OtgPixelRateCntl, 3, 1, 0x8C),
        (R::OtgPhypllPixelRateCntl, 3, 1, 0x8F), (R::DcnVmAgpBase, 0, 2, 0x498),
        (R::CcDcPipeDis, 0, 2, 0xCA), (R::DmcubSecCntl, 0, 2, 0x3278), (R::DmcubInbox1Rptr, 0, 2, 0x3281),
        (R::DmcubScratch, 15, 2, 0x329C), (R::DmcubCntl, 0, 2, 0x32A0), (R::DmcubTimerCurrent, 0, 2, 0x32A7),
    ];
    for (r, i, s, d) in checks { assert_eq!(reg(r, i), (seg[s] + d) * 4, "{} {}", r.name(), i); }
}
