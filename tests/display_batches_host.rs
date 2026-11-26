// Host cases of display's step 1 (213-DRV-0086): the eight batches' registers, the order of names, pauses and reads,
// how many batches a build reads, batch 4's dependence on batch 3, and the decoding of the state Linux's Display Core
// dumped on the Ryzen on 2026-11-21 (hwdocs/cezanne-display.md); the production order's reads against what steps 3
// and 4 take (213-DRV-0098).
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;

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
    assert_eq!(report.batches_read, 11);
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
    assert_eq!(counts, vec![10, 76, 8, 20, 13, 6, 14, 31, 44, 68, 15]);
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
    assert_eq!((1..=BATCHES).map(would).collect::<Vec<_>>(), vec![10, 76, 8, 20, 13, 6, 14, 31, 44, 68, 15]);
    assert!(l.iter().any(|x| x.starts_with("B4 HUBP: NOT READ") && x.contains("ONLY FOR EACH HUBP n THAT B3 SHOWS POWERED")));
    assert!(l.last().unwrap().starts_with("STEP 1 DONE: 0 OF 11 BATCHES READ, 0 REGISTERS, NOTHING WRITTEN"));
    assert!(!events.iter().any(|e| matches!(e, Event::Pause(_))));
}

#[test]
fn a_build_reads_its_allowed_batches_and_no_register_of_the_others() {
    let state = linux_state();
    for allowed in 0..=BATCHES {
        let (report, events) = run_on(state.clone(), allowed, &[]);
        // Linux's state runs two OTGs: batches 9 and 10 have no firmware's pipe to read, and do not count as read.
        assert_eq!(report.batches_read, if allowed >= 11 { 9 } else { allowed.min(8) });
        let mut expected = Vec::new();
        let mut known = Known::default();
        for b in 1..=allowed {
            if b == 4 { known = all_powered(); } // batch 3 of this state shows every HUBP powered
            expected.extend(plan(b, &known).iter().map(|n| n.reg.offset()));
        }
        assert_eq!(reads(&events), expected, "allowed {}", allowed);
        assert_eq!(report.registers_read, expected.len());
    }
    // A number beyond eleven reads the eleven (Linux's state: the nine with registers to read).
    assert_eq!(run_on(linux_state(), 200, &[]).0.batches_read, 9);
    assert_eq!(run_on(firmware_state(), 200, &[]).0.batches_read, 11);
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
    assert_eq!(events.iter().filter(|e| **e == Event::Pause(PAUSE_BETWEEN_BATCHES_MS)).count(), 11);
}

#[test]
fn batch_11_reads_hdmi_a_1s_ddc_line_and_the_i2c_engine_and_never_its_data_register() {
    // The engine as the firmware might leave it: nobody holding it, DDC1 enabled at Display Core's 100 kHz prescale from
    // the 48 MHz crystal, both pads in I2C mode and high.
    let mut s = firmware_state();
    for (r, v) in [(R::DcI2cArbitration, 0), (R::DcI2cControl, 0), (R::DcI2cSwStatus, 0), (R::DcI2cDdc1HwStatus, 0),
                   (R::DcI2cDdc1Speed, 480 << 16 | 2), (R::DcI2cDdc1Setup, 0x28 << 24 | 1 << 6), (R::MicrosecondTimeBaseDiv, 0x0130),
                   (R::DcGpioDdc1Mask, 0), (R::DcGpioDdc1A, 0), (R::DcGpioDdc1En, 0), (R::DcGpioDdc1Y, 0x0101)] {
        s.insert(reg(r, 0), v);
    }
    let (report, events) = run_on(s, BATCHES, &[]);
    assert_eq!(report.batches_read, 11);
    // Its fifteen registers in their order, all inside the engine's window, and DC_I2C_DATA never read.
    let order: Vec<u32> = plan(11, &Known::default()).iter().map(|n| n.reg.offset()).collect();
    assert_eq!(order, [0x14D64, 0x14D60, 0x14D6C, 0x14D70, 0x14D88, 0x14D8C, 0x14DB8, 0x14DBC, 0x14DC0, 0x14DC4, 0x004EC, 0x17640, 0x17644, 0x17648, 0x1764C]);
    let read = reads(&events);
    assert!(order.iter().all(|o| read.contains(o)));
    assert!(!read.contains(&0x14DC8), "DC_I2C_DATA read");
    let l = lines(&events);
    let has = |s: &str| assert!(l.iter().any(|x| *x == s), "no {:?} in {:#?}", s, l.iter().filter(|x| x.starts_with("B11")).collect::<Vec<_>>());
    has("B11 DC_I2C_TRANSACTION2 AT 14DC0 = 00000000");
    has("B11 I2C ENGINE: HELD BY NOBODY (SW REQ 0, DMCU REQ 0); LAST TRANSFER STATUS 0, DONE 0; DDC SELECT 0; DDC1 HW STATUS 0");
    has("B11 DDC1: ENABLED 1, TIME LIMIT 40, PRESCALE 480, THRESHOLD 2; MICROSECOND TIME BASE 48, XTAL REF DIV 1");
    has("B11 DDC1 PADS: AUX MODE 0; CLOCK MASK 0 EN 0 A 0 Y 1; DATA MASK 0 EN 0 A 0 Y 1");
    // Steps 3 and 4 need the first ten only: a build that reads ten still has its reference.
    assert_eq!((BATCHES, batches::STEP3_BATCHES), (11, 10));
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
    has("B4 HUBP0: SURFACE 0000_00400000, FORMAT 8, PITCH FIELD 3455, ROTATION 0, BLANK 0, TTU_DISABLE 0");
    has("B4 HUBP1: SURFACE 0000_02400000, FORMAT 8, PITCH FIELD 2559");
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
    assert_eq!(report.registers_read, 10 + 76 + 7 + 13 + 6 + 14 + 31 + 15);
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

// --- the production order (213-DRV-0098) ---

// The firmware's state with everything steps 3 and 4 take: HPD 1 sensing, DIG E driving PHY E for OTG 0, the DMCUB held
// in reset, pipe 0's reference (boot 70's values) and the I2C engine idle.
fn ryzen_state() -> HashMap<u32, u32> {
    let mut m = firmware_state();
    m.insert(reg(R::HpdIntStatus, 0), 0b10);
    for d in 0..4u8 { m.insert(reg(R::DigFeCntl, d), 7); }
    m.insert(reg(R::DigFeCntl, 4), 1 << 24);
    m.insert(reg(R::DigBeCntl, 4), (1 << 4) << 8);
    m.insert(reg(R::DigBeEnCntl, 4), 0x101);
    m.insert(reg(R::DmcubCntl, 0), 1 << 17);
    for (r, v) in [(R::OtgHTotal, 2719), (R::OtgVTotal, 1645), (R::OtgHBlankStartEnd, 112 << 16 | 2672), (R::OtgVBlankStartEnd, 43 << 16 | 1643),
        (R::HubpreqBlankOffset0, 0x13 << 16 | 0x35), (R::HubpreqSurfacePitch, 0xA00), (R::DcnVmFbLocationBase, 0xF400), (R::OtgVstartupParam, 0xD),
        (R::OtgVupdateParam, 0x0140_02A8), (R::OtgVreadyParam, 0x12C), (R::HubpreqSurf0TtuCntl0, 8 << 24), (R::DchubbubGlobalTimerCntl, 0x1001)] {
        m.insert(reg(r, 0), v);
    }
    for (o, v) in [(0x14D88, 0x01E0_0002), (0x14D8C, 0x3000_0040), (0x004EC, 0x0012_0230)] { m.insert(o, v); }
    m
}

fn kind_at(o: u32) -> R { dcn::ALL.iter().copied().find(|r| (0..r.instances()).any(|i| r.at(i).map(|x| x.offset()) == Some(o))).unwrap() }

// Step 1 in an order on a model, through the order's own log: the reads, the store, the report and the lines logged.
fn step1_in(order: batches::Order, regs: HashMap<u32, u32>, unmapped: &[u32]) -> (Vec<u32>, batches::Store, batches::Report, Vec<String>) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let mut model = Model { regs, events: events.clone(), unmapped: unmapped.iter().copied().collect() };
    let mut store = batches::Store::default();
    let report = match order {
        batches::Order::Survey => batches::read_into(&mut model, BATCHES, order, &mut Log(events.clone()), &mut store),
        batches::Order::Production => {
            let mut buf = vec![0u8; batches::HELD_BYTES];
            let mut held = batches::Held::new(Log(events.clone()), &mut buf);
            batches::read_into(&mut model, BATCHES, order, &mut held, &mut store)
        }
    };
    let ev = events.borrow().clone();
    (reads(&ev), store, report, lines(&ev).iter().map(|x| x.to_string()).collect())
}

// What steps 3 and 4 take from step 1's values: the pipes chosen, and the reference of the firmware's.
fn taken(s: &batches::Store) -> (Result<(light::Firmware, light::Choice), light::Refusal>, Option<light::Reference>) {
    let found = light::assess(s);
    let reference = found.as_ref().ok().and_then(|(f, c)| light::reference(s, f, c));
    (found, reference)
}

#[test]
fn the_production_order_reads_only_what_steps_2_to_4_take_in_the_surveys_order_and_pauses_nowhere() {
    let (survey, ss, sr, _) = step1_in(batches::Order::Survey, ryzen_state(), &[]);
    let (production, ps, pr, plines) = step1_in(batches::Order::Production, ryzen_state(), &[]);
    // The survey's reads of the kinds `USED` names, in the survey's order, and no other.
    let want: Vec<u32> = survey.iter().copied().filter(|&o| batches::USED.contains(&kind_at(o))).collect();
    assert_eq!(production, want);
    assert_eq!((sr.registers_read, pr.registers_read, sr.batches_read, pr.batches_read), (305, want.len(), 11, 11));
    assert_eq!((ss.batches(), ps.batches()), (11, 11), "every batch counted read: steps 3 and 4 need the first ten");
    eprintln!("step 1 on the Ryzen's state: the survey reads {} registers, the production order {}", survey.len(), production.len());
    // Each value it read is the survey's.
    for &o in &production { let (r, i) = dcn::ALL.iter().flat_map(|&r| (0..r.instances()).map(move |i| (r, i))).find(|&(r, i)| r.at(i).unwrap().offset() == o).unwrap(); assert_eq!(ps.get(r, i), ss.get(r, i)); }
    // Its log: one line, no pause, no value.
    assert_eq!(plines, [format!("STEP 1 DONE: 11 OF 11 BATCHES READ, {} REGISTERS, NOTHING WRITTEN; ONLY WHAT STEPS 2 TO 4 TAKE (THE PRODUCTION ORDER)", want.len())]);
    // A register the grant does not map: named in either order.
    let (_, _, _, plines) = step1_in(batches::Order::Production, ryzen_state(), &[reg(R::DomainPgStatus, 2)]);
    assert!(plines.iter().any(|x| x.starts_with("B3 DOMAIN2_PG_STATUS AT") && x.ends_with("NOT MAPPED, NOT READ")), "{:#?}", plines);
    assert!(plines.iter().any(|x| x.starts_with("B4 HUBP: NOTHING TO READ")), "{:#?}", plines);
    // A build that may read nothing reads nothing in either order.
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let mut model = Model { regs: ryzen_state(), events: events.clone(), unmapped: BTreeSet::new() };
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let mut held = batches::Held::new(Log(events.clone()), &mut buf);
    assert_eq!(batches::read_into(&mut model, 0, batches::Order::Production, &mut held, &mut batches::Store::default()).registers_read, 0);
    assert!(reads(&events.borrow()).is_empty() && !events.borrow().iter().any(|e| matches!(e, Event::Pause(_))));
}

#[test]
fn steps_3_and_4_take_the_same_from_either_orders_step_1() {
    let (_, ss, _, _) = step1_in(batches::Order::Survey, ryzen_state(), &[]);
    let (_, ps, _, _) = step1_in(batches::Order::Production, ryzen_state(), &[]);
    let (found, reference) = taken(&ss);
    assert_eq!(found, Ok(light::EXPECTED));
    assert!(reference.is_some());
    assert_eq!(taken(&ps), (found, reference));
}

#[test]
fn no_register_the_production_order_leaves_unread_changes_what_steps_3_and_4_take_and_each_it_reads_does() {
    let (survey, base_store, _, _) = step1_in(batches::Order::Survey, ryzen_state(), &[]);
    let base = taken(&base_store);
    // Whatever value step 1 finds in a register the production order does not read, steps 3 and 4 take the same.
    let unread: Vec<u32> = survey.iter().copied().filter(|&o| !batches::USED.contains(&kind_at(o))).collect();
    assert!(unread.len() > 100, "{}", unread.len());
    for &o in &unread {
        let was = ryzen_state().get(&o).copied().unwrap_or(0);
        for v in [0, !0, 0x5A5A_5A5A, was ^ 1] {
            let mut regs = ryzen_state();
            regs.insert(o, v);
            let (_, s, _, _) = step1_in(batches::Order::Survey, regs, &[]);
            assert_eq!(taken(&s), base, "{:05X} ({}) = {:08X}", o, kind_at(o).name(), v);
        }
    }
    // Each kind it reads in batches 1 to 10 is taken: without it (unmapped), steps 3 and 4 take something else, on the
    // Ryzen's state or on one where front end A is busy or the back end names front end B (batch 11's: display_ddc_host).
    let mut busy_a = ryzen_state();
    busy_a.insert(reg(R::DigFeCntl, 0), 1 << 24);
    let mut names_b = busy_a.clone();
    names_b.insert(reg(R::DigBeCntl, 4), (1 << 1) << 8);
    names_b.insert(reg(R::DigFeCntl, 1), 1 << 24);
    let states = [ryzen_state(), busy_a, names_b];
    for kind in batches::USED {
        if !(1..=10).any(|b| batches::plan(b, &firmware_known()).iter().any(|n| n.r == kind)) { continue; }
        let gone: Vec<u32> = (0..kind.instances()).map(|i| reg(kind, i)).collect();
        let differs = states.iter().any(|st| {
            let (_, with, _, _) = step1_in(batches::Order::Production, st.clone(), &[]);
            let (_, without, _, _) = step1_in(batches::Order::Production, st.clone(), &gone);
            taken(&with) != taken(&without)
        });
        assert!(differs, "{} is read but not taken", kind.name());
    }
}

// --- batch 12: HDMI-A-2's blocks (213-DRV-0112) ---

// The Ryzen's state with a monitor in the second socket: HPD 2 sensing and enabled, DIG B idle, DDC2 set up as Display
// Core leaves DDC1, its pads in I2C mode and high; DDC pad 1 in I2C mode, pad 2 not (a guess for the model, not a reading).
fn second_state() -> HashMap<u32, u32> {
    let mut m = ryzen_state();
    for (r, i, v) in [(R::HpdIntStatus, 1u8, 0b10u32), (R::HpdControl, 1, 1 << 28), (R::DigFeCntl, 1, 7), (R::DcI2cDdc2Speed, 0, 480 << 16 | 2),
                      (R::DcI2cDdc2Setup, 0, 0x28 << 24 | 1 << 6), (R::DcGpioDdc2Y, 0, 0x0101), (R::DcGpioAuxCtrl5, 0, 1 << 12), (R::PhyAuxCntl, 0, 1 << 10)] {
        m.insert(reg(r, i), v);
    }
    m
}

// Step 1 with batch 12 after the eleven, as display's start reads it: the reads, the store and the lines.
fn step1_and_12(order: batches::Order, regs: HashMap<u32, u32>, unmapped: &[u32], allowed: bool) -> (Vec<u32>, batches::Store, batches::Report, Vec<String>) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let mut model = Model { regs, events: events.clone(), unmapped: unmapped.iter().copied().collect() };
    let mut store = batches::Store::default();
    let mut buf = vec![0u8; batches::HELD_BYTES];
    let report = match order {
        batches::Order::Survey => {
            let mut log = Log(events.clone());
            batches::read_into(&mut model, BATCHES, order, &mut log, &mut store);
            batches::read_second(&mut model, allowed, &mut log, &mut store)
        }
        batches::Order::Production => {
            let mut held = batches::Held::new(Log(events.clone()), &mut buf);
            batches::read_into(&mut model, BATCHES, order, &mut held, &mut store);
            batches::read_second(&mut model, allowed, &mut held, &mut store)
        }
    };
    let ev = events.borrow().clone();
    (reads(&ev), store, report, lines(&ev).iter().map(|x| x.to_string()).collect())
}

#[test]
fn batch_12_reads_hdmi_a_2s_blocks_at_the_headers_offsets_and_never_a_phy_or_the_i2c_data() {
    // dcn_2_1_0_offset.h (Linux v7.0): HPD1_ (HPD 2), DIG1_ (DIG B), PHYPLLB in DCCG, DDC2's engine registers and pads,
    // DC_GPIO_AUX_CTRL_5 and PHY_AUX_CNTL; in that order, whatever the earlier batches showed.
    let (s1, s2) = (|d: u32| (0xC0 + d) * 4, |d: u32| (0x34C0 + d) * 4);
    let want = [s2(0x1F1C), s2(0x1F1E), s2(0x2168), s2(0x21AF), s2(0x21B0), s1(0x41), s2(0x1E9D), s2(0x1EA4), s2(0x1EA5), s2(0x28D4),
                s2(0x28D5), s2(0x28D6), s2(0x28D7), s2(0x291D), s2(0x28FF)];
    for known in [Known::default(), firmware_known()] {
        assert_eq!(plan(batches::SECOND, &known).iter().map(|n| n.reg.offset()).collect::<Vec<_>>(), want);
    }
    for &o in &want {
        assert!(dcn::DCN_BYTES.contains(&o) && !dcn::MSIX_BYTES.contains(&o), "{:05X}", o);
        // Not a PHY's DPCS or RDPCS (the survey's rule), not DC_I2C_DATA (a read moves its index).
        for p in 0..5u8 { let (a, b) = light::phy_block(p); assert!(!(a..b).contains(&o), "{:05X} in PHY {}'s DPCS", o, p); }
        assert_ne!(o, reg(R::DcI2cData, 0));
    }
    let names: Vec<String> = plan(batches::SECOND, &Known::default()).iter().map(|n| n.to_string()).collect();
    for x in ["HPD1_DC_HPD_INT_STATUS AT 14F70", "DIG1_DIG_BE_EN_CNTL AT 159C0", "PHYPLL1_PIXCLK_RESYNC_CNTL AT 00404", "DC_I2C_DDC2_SPEED AT 14D90",
              "DC_GPIO_DDC2_MASK AT 17650", "DC_GPIO_AUX_CTRL_5 AT 17774", "PHY_AUX_CNTL AT 176FC"] {
        assert!(names.iter().any(|n| n == x), "no {:?} in {:#?}", x, names);
    }
    assert_eq!((batches::title(batches::SECOND), batches::HDMI_A_2.len()), ("HDMI-A-2", 15));
}

#[test]
fn batch_12_is_read_whole_in_either_order_its_values_logged_in_both_and_what_steps_3_and_4_take_unchanged() {
    let notes = [
        "B12 HDMI-A-2 HPD 2 (HPD1_): SENSE 1, ENABLED 1; INT_STATUS 00000002, CONTROL 10000000",
        "B12 HDMI-A-2 DIG B: FE FROM OTG 7, SYMCLK_FE_ON 0 (00000007); BE ENABLED 0, MODE 0, FROM FE MASK 0, SYMCLK_BE_ON 0 (00000000, 00000000)",
        "B12 HDMI-A-2 PHYPLLB: PIXCLK_ENABLE 0, DEEP COLOR 0 (00000000)",
        "B12 HDMI-A-2 DDC2: HW STATUS 0 (00000000); ENABLED 1, TIME LIMIT 40, PRESCALE 480, THRESHOLD 2 (SPEED 01E00002, SETUP 28000040)",
        "B12 HDMI-A-2 DDC2 PADS: AUX MODE 0; CLOCK MASK 0 EN 0 A 0 Y 1; DATA MASK 0 EN 0 A 0 Y 1 (MASK 00000000)",
        "B12 THE DDC PADS' I2C MODE: DDC1 1 DDC2 0; AUX PAD RXSEL 1:1 2:0 (DC_GPIO_AUX_CTRL_5 00001000, PHY_AUX_CNTL 00000400)",
        "B12 HDMI-A-2 DONE: 15 OF 15 REGISTERS READ, NOTHING WRITTEN",
    ];
    let twelve: Vec<u32> = plan(batches::SECOND, &Known::default()).iter().map(|n| n.reg.offset()).collect();
    let (survey, ss, sr, sl) = step1_and_12(batches::Order::Survey, second_state(), &[], true);
    let (production, ps, pr, pl) = step1_and_12(batches::Order::Production, second_state(), &[], true);
    // Whole in either order, after the eleven: the same fifteen reads last.
    assert_eq!((&survey[survey.len() - 15..], &production[production.len() - 15..]), (&twelve[..], &twelve[..]));
    assert_eq!((sr, pr), (batches::Report { batches_read: 1, registers_read: 15 }, batches::Report { batches_read: 1, registers_read: 15 }));
    // The survey's order: named and paused before, each value after its read, the notes; the production order logs the
    // notes alone after step 1's own line.
    assert!(sl.iter().any(|x| x == "B12 HDMI-A-2: READING 15 REGISTERS NEXT, NEVER WRITTEN (213-DRV-0112)"));
    assert!(sl.iter().any(|x| x == "B12 DC_GPIO_DDC2_MASK AT 17650 = 00000000") && sl.iter().any(|x| x == "B12 HPD1_DC_HPD_CONTROL AT 14F78 = 10000000"), "{:#?}", sl);
    for n in notes { assert!(sl.iter().any(|x| x == n), "survey: no {:?} in {:#?}", n, sl.iter().filter(|x| x.starts_with("B12 HDMI-A-2 ") || x.starts_with("B12 THE")).collect::<Vec<_>>()); }
    assert_eq!(&pl[1..], &notes[..], "the production order's lines: {:#?}", pl);
    assert!(pl[0].starts_with("STEP 1 DONE: 11 OF 11 BATCHES READ, "));
    // What it kept: DDC2's values for step 2; what steps 3 and 4 take is what the eleven alone gave.
    for s in [&ss, &ps] {
        assert_eq!((s.get(R::DcI2cDdc2Setup, 0), s.get(R::DcGpioDdc2Y, 0), s.get(R::HpdIntStatus, 1), s.batches()), (Some(0x2800_0040), Some(0x0101), Some(0b10), 12));
    }
    let (_, eleven, _, _) = step1_in(batches::Order::Survey, second_state(), &[]);
    assert_eq!(taken(&ss), taken(&eleven));
    assert_eq!(taken(&ps), taken(&eleven));
    // A register the grant does not map: named, not read, its fields shown as a dash.
    let (_, _, r, l) = step1_and_12(batches::Order::Production, second_state(), &[reg(R::DcGpioDdc2Mask, 0)], true);
    assert_eq!(r.registers_read, 14);
    assert!(l.iter().any(|x| x == "B12 DC_GPIO_DDC2_MASK AT 17650: NOT MAPPED, NOT READ"), "{:#?}", l);
    assert!(l.iter().any(|x| x.starts_with("B12 HDMI-A-2 DDC2 PADS: AUX MODE -; CLOCK MASK - EN 0") && x.ends_with("(MASK -)")), "{:#?}", l);
    // A step 1 that reads not all eleven reads none of it, and names it.
    let (reads12, _, r, l) = step1_and_12(batches::Order::Survey, second_state(), &[], false);
    assert_eq!((r, reads12), (batches::Report::default(), step1_in(batches::Order::Survey, second_state(), &[]).0));
    assert_eq!(l.iter().filter(|x| x.starts_with("B12 WOULD READ ")).count(), 15);
    for x in sl.iter().chain(&pl).chain(&l) { assert!(x.len() + "[DISPLAY] ".len() <= 180, "{} bytes: {}", x.len(), x); }
}
