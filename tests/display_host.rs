// Host cases of the Ryzen's display driver (213-DRV-0086): a mode's timing arithmetic, the EDID parser, and first
// light's register writes against a model of DCN 2.1's blocks, in order.
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;

use dcn::{first_light, Field, Output, Pattern, Pipe, PlanError, Protected, Reg, RegError, Signal, Step, Table, R};
use std::collections::HashMap;
use timing::{Otg, Timing, CEA_1080P60};

// The two monitors' preferred timings, from their EDIDs as Linux read them on 2026-11-21 (hwdocs/cezanne-display.md).
const HDMI_PREFERRED: Timing = Timing { pixel_khz: 324_000, h_active: 3440, h_front: 48, h_sync: 32, h_back: 80,
    v_active: 1440, v_front: 3, v_sync: 5, v_back: 52, h_sync_positive: true, v_sync_positive: false, interlaced: false };
const DP_PREFERRED: Timing = Timing { pixel_khz: 268_500, h_active: 2560, h_front: 48, h_sync: 32, h_back: 80,
    v_active: 1600, v_front: 3, v_sync: 10, v_back: 33, h_sync_positive: true, v_sync_positive: false, interlaced: false };

// --- timing ---

#[test]
fn cea_1080p60_totals_rate_and_otg_values() {
    let t = CEA_1080P60;
    assert_eq!((t.h_total(), t.v_total()), (2200, 1125));
    assert_eq!(t.refresh_millihertz(), 60_000);
    assert_eq!(t.tmds_khz(8), Some(148_500));
    assert_eq!(t.tmds_khz(10), Some(185_625));
    assert_eq!(t.tmds_khz(9), None);
    let o = t.otg().unwrap();
    assert_eq!((o.h_total, o.h_sync_end, o.h_blank_end, o.h_blank_start), (2199, 44, 192, 2112));
    assert_eq!((o.v_total, o.v_sync_end, o.v_blank_end, o.v_blank_start), (1124, 5, 41, 1121));
    assert!(!o.h_sync_negative && !o.v_sync_negative);
    assert_eq!(o.vcount_init, 1121);
    assert_eq!(o.timing(t.pixel_khz), Some(t));
}

// An OTG row as Display Core's dump prints it: v_bs v_be v_se vtot h_bs h_be h_se htot, and the two polarities.
fn dumped(v_bs: u32, v_be: u32, v_se: u32, vtot: u32, h_bs: u32, h_be: u32, h_se: u32, htot: u32, vpol: bool, hpol: bool) -> Otg {
    Otg { h_total: htot, h_blank_start: h_bs, h_blank_end: h_be, h_sync_end: h_se, v_total: vtot, v_blank_start: v_bs,
          v_blank_end: v_be, v_sync_end: v_se, h_sync_negative: hpol, v_sync_negative: vpol, vcount_init: v_bs }
}

#[test]
fn the_ryzens_two_monitors_as_linux_set_them_are_their_edids_preferred_timings() {
    // amdgpu_dm_dtn_log, 2026-11-21: OTG 0 drives HDMI-A-1, OTG 1 drives DP-2.
    let otg0 = dumped(1497, 57, 5, 1499, 3552, 112, 32, 3599, true, false);
    let otg1 = dumped(1643, 43, 10, 1645, 2672, 112, 32, 2719, true, false);
    assert_eq!(otg0.timing(324_000), Some(HDMI_PREFERRED));
    assert_eq!(otg1.timing(268_500), Some(DP_PREFERRED));
    assert_eq!(HDMI_PREFERRED.otg().unwrap(), otg0);
    assert_eq!(DP_PREFERRED.otg().unwrap(), otg1);
    assert_eq!((HDMI_PREFERRED.h_total(), HDMI_PREFERRED.v_total(), HDMI_PREFERRED.refresh_millihertz()), (3600, 1500, 60_000));
    assert_eq!((DP_PREFERRED.h_total(), DP_PREFERRED.v_total(), DP_PREFERRED.refresh_millihertz()), (2720, 1646, 59_972));
}

#[test]
fn timings_the_otg_cannot_run_are_refused() {
    let mut t = CEA_1080P60;
    t.interlaced = true;
    assert_eq!(t.otg(), Err(timing::TimingError::Interlaced));
    let mut t = CEA_1080P60;
    t.h_front = 0;
    assert_eq!(t.otg(), Err(timing::TimingError::NoFrontPorch));
    let mut t = CEA_1080P60;
    t.h_active = 0x7FFF;
    assert_eq!(t.otg(), Err(timing::TimingError::TooLarge));
    let mut t = CEA_1080P60;
    t.pixel_khz = 0;
    assert_eq!(t.check(), Err(timing::TimingError::Empty));
    let mut o = CEA_1080P60.otg().unwrap();
    o.h_blank_end = o.h_blank_start;
    assert_eq!(o.timing(148_500), None);
}

// --- EDID ---

fn dtd(t: &Timing) -> [u8; 18] {
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

fn descriptor(tag: u8, payload: &[u8]) -> [u8; 18] {
    let mut d = [0u8; 18];
    d[3] = tag;
    d[5..5 + payload.len()].copy_from_slice(payload);
    d
}

fn seal(block: &mut [u8]) { let sum = block[..127].iter().fold(0u8, |s, b| s.wrapping_add(*b)); block[127] = 0u8.wrapping_sub(sum); }

// A synthetic monitor shaped like the Ryzen's HDMI one (its own bytes are not kept): base block and CTA extension.
fn monitor(hdmi: bool) -> Vec<u8> {
    let mut b = vec![0u8; 256];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    let id: u16 = (((b'D' - b'A' + 1) as u16) << 10) | (((b'E' - b'A' + 1) as u16) << 5) | (b'L' - b'A' + 1) as u16;
    b[8..10].copy_from_slice(&id.to_be_bytes());
    b[10..12].copy_from_slice(&0xA0F3u16.to_le_bytes());
    b[17] = 31; b[18] = 1; b[19] = 4; b[20] = 0x80 | 2 << 4; b[21] = 80; b[22] = 34;
    b[54..72].copy_from_slice(&dtd(&HDMI_PREFERRED));
    b[72..90].copy_from_slice(&descriptor(0xFD, &[24, 85, 30, 160, 33]));
    b[90..108].copy_from_slice(&descriptor(0xFC, b"WIDE MONITOR\n"));
    b[108..126].copy_from_slice(&descriptor(0x10, &[]));
    b[126] = 1;
    seal(&mut b[..128]);
    let e = &mut b[128..];
    e[0] = 0x02; e[1] = 3;
    let mut blocks: Vec<u8> = vec![0x40 | 3, 16 | 0x80, 4, 31]; // video: VIC 16 native, 4, 31
    if hdmi {
        blocks.extend_from_slice(&[0x60 | 7, 0x03, 0x0C, 0x00, 0x20, 0x00, 0x38, 68]); // HDMI 1.4: 2.0.0.0, 30/36-bit, 340 MHz
        blocks.extend_from_slice(&[0x60 | 6, 0xD8, 0x5D, 0xC4, 1, 120, 0x80]); // HDMI Forum: 600 MHz
    }
    blocks.extend_from_slice(&[0x60 | 3, 0x1A, 0x00, 0x00]); // another vendor's block
    e[4..4 + blocks.len()].copy_from_slice(&blocks);
    let start = 4 + blocks.len();
    e[2] = start as u8;
    e[start..start + 18].copy_from_slice(&dtd(&CEA_1080P60));
    seal(e);
    b
}

#[test]
fn edid_base_block_identity_name_range_and_preferred_timing() {
    let m = edid::Edid::parse(&monitor(true)).unwrap();
    assert_eq!(&m.manufacturer, b"DEL");
    assert_eq!((m.product, m.year, m.version, m.revision), (0xA0F3, 2021, 1, 4));
    assert!(m.digital);
    assert_eq!(m.bits_per_color, 8);
    assert_eq!((m.width_cm, m.height_cm), (80, 34));
    assert_eq!(m.name(), b"WIDE MONITOR");
    assert_eq!(m.range, Some(edid::Range { min_v_hz: 24, max_v_hz: 85, min_h_khz: 30, max_h_khz: 160, max_pixel_mhz: 330 }));
    assert_eq!(m.preferred(), Some(HDMI_PREFERRED));
    assert_eq!(m.extensions, 1);
}

#[test]
fn edid_cta_extension_videos_hdmi_blocks_and_its_timings() {
    let m = edid::Edid::parse(&monitor(true)).unwrap();
    assert!(m.cta);
    assert_eq!(&m.vics[..m.vic_count], &[16, 4, 31]);
    let h = m.hdmi.unwrap();
    assert_eq!(h.physical_address, 0x2000);
    assert!(h.deep_30 && h.deep_36 && !h.deep_48);
    assert_eq!((h.max_tmds_mhz, h.forum_max_tmds_mhz), (340, 600));
    assert_eq!(m.timings().collect::<Vec<_>>(), vec![HDMI_PREFERRED, CEA_1080P60]);
    assert_eq!(m.max_tmds_khz(), 600_000);
    // The largest mode one DVI link carries, and the largest without scrambling.
    assert_eq!(m.best_within(timing::DVI_SINGLE_LINK_KHZ), Some(CEA_1080P60));
    assert_eq!(m.best_within(timing::TMDS_NO_SCRAMBLING_KHZ), Some(HDMI_PREFERRED));
}

#[test]
fn edid_without_an_hdmi_block_is_a_dvi_sink_limited_by_its_range() {
    let m = edid::Edid::parse(&monitor(false)).unwrap();
    assert_eq!(m.hdmi, None);
    assert_eq!(m.max_tmds_khz(), 330_000);
    assert_eq!(m.best_within(timing::TMDS_NO_SCRAMBLING_KHZ), Some(HDMI_PREFERRED));
}

#[test]
fn edid_damage_is_refused() {
    let good = monitor(true);
    assert_eq!(edid::Edid::parse(&good[..100]), Err(edid::Error::Short));
    let mut bad = good.clone(); bad[0] = 1;
    assert_eq!(edid::Edid::parse(&bad), Err(edid::Error::Header));
    let mut bad = good.clone(); bad[60] ^= 1;
    assert_eq!(edid::Edid::parse(&bad), Err(edid::Error::Checksum(0)));
    let mut bad = good.clone(); bad[200] ^= 1;
    assert_eq!(edid::Edid::parse(&bad), Err(edid::Error::Checksum(1)));
    let mut bad = good.clone(); bad[18] = 2; seal(&mut bad[..128]);
    assert_eq!(edid::Edid::parse(&bad), Err(edid::Error::Version));
    // A base block alone is read without its extension.
    let base = edid::Edid::parse(&good[..128]).unwrap();
    assert!(!base.cta && base.hdmi.is_none());
}

#[test]
fn edid_detailed_timing_polarities_and_descriptors() {
    let mut d = dtd(&CEA_1080P60);
    assert_eq!(edid::detailed_timing(&d), Some(CEA_1080P60));
    d[17] = 0x18; // digital separate, both negative
    let t = edid::detailed_timing(&d).unwrap();
    assert!(!t.h_sync_positive && !t.v_sync_positive);
    d[17] = 0x10 | 0x02; // digital composite: only hsync's polarity is given
    assert!(!edid::detailed_timing(&d).unwrap().v_sync_positive);
    assert_eq!(edid::detailed_timing(&descriptor(0xFC, b"X")), None);
    assert_eq!(edid::detailed_timing(&d[..17]), None);
}

// --- DCN 2.1 registers and first light ---

const DCN: [u32; 3] = [0x12, 0xC0, 0x34C0];
fn byte(segment: usize, dword: u32) -> Reg { Reg::new((DCN[segment] + dword) * 4).unwrap() }

#[test]
fn register_addresses_match_the_headers_instances() {
    // dcn_2_1_0_offset.h: instance 1 of each, against its stride from instance 0.
    assert_eq!(R::OtgHTotal.at(1).unwrap(), byte(2, 0x1BAA));
    assert_eq!(R::OtgVBlankStartEnd.at(1).unwrap(), byte(2, 0x1BB6));
    assert_eq!(R::OtgControl.at(1).unwrap(), byte(2, 0x1BC1));
    assert_eq!(R::OtgStatusFrameCount.at(1).unwrap(), byte(2, 0x1BCC));
    assert_eq!(R::OtgClockControl.at(1).unwrap(), byte(2, 0x1C06));
    assert_eq!(R::OtgVreadyParam.at(1).unwrap(), byte(2, 0x1C09));
    assert_eq!(R::OptcDataSourceSelect.at(1).unwrap(), byte(2, 0x1ADB));
    assert_eq!(R::OptcInputClockControl.at(1).unwrap(), byte(2, 0x1ADF));
    assert_eq!(R::VtgControl.at(1).unwrap(), byte(2, 0x0529));
    assert_eq!(R::OppPipeControl.at(1).unwrap(), byte(2, 0x18E6));
    assert_eq!(R::DpgControl.at(1).unwrap(), byte(2, 0x18AE));
    assert_eq!(R::DpgDimensions.at(1).unwrap(), byte(2, 0x18B0));
    assert_eq!(R::DpgOffsetSegment.at(1).unwrap(), byte(2, 0x18B4));
    assert_eq!(R::DigFeCntl.at(1).unwrap(), byte(2, 0x2168));
    assert_eq!(R::DigBeCntl.at(1).unwrap(), byte(2, 0x21AF));
    assert_eq!(R::OtgPixelRateCntl.at(1).unwrap(), byte(1, 0x84));
    assert_eq!(R::OtgPhypllPixelRateCntl.at(1).unwrap(), byte(1, 0x87));
    assert_eq!(R::PhypllPixclkResyncCntl.at(3).unwrap(), byte(1, 0x43));
    assert_eq!(R::PhypllPixclkResyncCntl.at(4).unwrap(), byte(1, 0x4C));
    assert_eq!(R::HpdControl.at(1).unwrap(), byte(2, 0x1F1E));
    assert_eq!(R::DentistDispclkCntl.at(0).unwrap(), byte(1, 0x64));
    for i in 0..dcn::PIPES { assert!(dcn::DCN_BYTES.contains(&R::OtgVreadyParam.at(i).unwrap().offset())); }
    for i in 0..dcn::DIGS { assert!(dcn::DCN_BYTES.contains(&R::DigBeCntl.at(i).unwrap().offset())); }
}

#[test]
fn fields_put_and_get() {
    let f = dcn::field::VTG0_ENABLE;
    assert_eq!((f.mask(), f.put(1), f.get(0x8000_0000)), (0x8000_0000, 0x8000_0000, 1));
    let f = dcn::field::COUNTER_HIGH;
    assert_eq!((f.mask(), f.put(0xFFFF)), (0x7FFF_0000, 0x7FFF_0000));
    assert_eq!(Field { shift: 0, width: 32 }.mask(), u32::MAX);
}

// A model of the blocks: registers as a map, the clock-on and master-enable status bits following their requests,
// and the command tables recorded in order.
struct Model { regs: HashMap<Reg, u32>, tables: Vec<(usize, Table)>, log: Vec<Reg> }

impl Model {
    fn new() -> Model {
        let mut regs = HashMap::new();
        // The firmware's pipe 0 on PHY E (illustrative values).
        regs.insert(R::OtgControl.at(0).unwrap(), 0x0001_0001);
        regs.insert(R::OtgHTotal.at(0).unwrap(), 2719);
        regs.insert(R::DigBeCntl.at(4).unwrap(), 0x0000_0100);
        Model { regs, tables: Vec::new(), log: Vec::new() }
    }
    fn read(&self, r: Reg) -> u32 { *self.regs.get(&r).unwrap_or(&0) }
    fn run(&mut self, plan: &dcn::Plan) {
        for (i, step) in plan.steps().enumerate() {
            match step {
                Step::Update { reg, mask, value } => {
                    assert!(dcn::DCN_BYTES.contains(&reg.offset()), "write outside the engine: {:x}", reg.offset());
                    assert_eq!(value & !mask, 0, "value outside its mask at {:x}", reg.offset());
                    let v = (self.read(reg) & !mask) | value;
                    self.regs.insert(reg, v);
                    self.log.push(reg);
                    self.follow(reg, v);
                }
                Step::Wait { reg, mask, value, .. } => assert_eq!(self.read(reg) & mask, value, "wait {} never ends at {:x}", i, reg.offset()),
                Step::Call(t) => self.tables.push((i, t)),
                Step::Frames { otg, .. } => assert_eq!(self.read(R::OtgControl.at(otg).unwrap()) & 1, 1, "frames waited for on a stopped generator"),
                Step::Reference | Step::Draw | Step::Begin(_) => {} // step 4's, after first light's plan
            }
        }
    }
    fn follow(&mut self, reg: Reg, v: u32) {
        for i in 0..dcn::PIPES {
            if reg == R::OptcInputClockControl.at(i).unwrap() { self.regs.insert(reg, (v & !4) | ((v >> 1) & 1) << 2); }
            if reg == R::OtgClockControl.at(i).unwrap() { self.regs.insert(reg, (v & !0x100) | (v & 1) << 8); }
            if reg == R::OtgControl.at(i).unwrap() { self.regs.insert(reg, (v & !0x1_0000) | (v & 1) << 16); }
        }
    }
    fn index_of(&self, reg: Reg) -> usize { self.log.iter().position(|&r| r == reg).expect("register never written") }
}

// The route's choice (hwdocs/cezanne-display.md): HDMI-A-1 on PHY A through pipe 1 and DIG front end A, with the
// firmware assumed on pipe 0 and PHY E's front end and back end (to be read under MIND Core first).
const PIPE1: Pipe = Pipe { otg: 1, opp: 1, dig_fe: 0 };
const HDMI_A: Output = Output { phy: 0, hpd: 0, encoder_id: 0x1E, connector_id: 0x0C };
const FIRMWARE: Protected = Protected { otg: 0, opp: 0, dig_fe: 4, phy: 4 };

#[test]
fn first_light_programs_the_second_pipe_in_display_cores_order() {
    let plan = first_light(PIPE1, HDMI_A, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap();
    let mut m = Model::new();
    let before: Vec<(Reg, u32)> = FIRMWARE.registers().map(|r| (r, m.read(r))).collect();
    m.run(&plan);
    // The firmware's pipe and PHY: never written, unchanged.
    for r in FIRMWARE.registers() { assert!(!plan.written().any(|w| w == r), "plan writes the firmware's {:x}", r.offset()); }
    assert_eq!(before, FIRMWARE.registers().map(|r| (r, m.read(r))).collect::<Vec<_>>());
    // The timing as the arithmetic gives it.
    let o = CEA_1080P60.otg().unwrap();
    assert_eq!(m.read(R::OtgHTotal.at(1).unwrap()), o.h_total);
    assert_eq!(m.read(R::OtgHBlankStartEnd.at(1).unwrap()), o.h_blank_start | o.h_blank_end << 16);
    assert_eq!(m.read(R::OtgHSyncA.at(1).unwrap()), o.h_sync_end << 16);
    assert_eq!(m.read(R::OtgVTotal.at(1).unwrap()), o.v_total);
    assert_eq!(m.read(R::OtgVTotalMax.at(1).unwrap()), o.v_total);
    assert_eq!(m.read(R::OtgVBlankStartEnd.at(1).unwrap()), o.v_blank_start | o.v_blank_end << 16);
    assert_eq!(m.read(R::OtgVSyncA.at(1).unwrap()), o.v_sync_end << 16);
    assert_eq!(m.read(R::VtgControl.at(1).unwrap()) >> 16 & 0x7FFF, o.vcount_init);
    // The wiring: OTG 1 from OPP 1, DIG front end A from OTG 1, PHY A's back end from front end A in DVI mode.
    assert_eq!(m.read(R::OptcDataSourceSelect.at(1).unwrap()) >> 8 & 0xF, 1);
    assert_eq!(m.read(R::DigFeCntl.at(0).unwrap()) & 7, 1);
    assert_eq!(m.read(R::DigBeCntl.at(0).unwrap()) >> 8 & 0x7F, 1);
    assert_eq!(m.read(R::DigBeCntl.at(0).unwrap()) >> 16 & 7, 2);
    assert_eq!(m.read(R::DpgControl.at(1).unwrap()) & 0xF1, 0x01);
    assert_eq!(m.read(R::DpgDimensions.at(1).unwrap()), 1920 << 16 | 1080);
    // The order.
    let clocks = m.index_of(R::OtgClockControl.at(1).unwrap());
    let timing = m.index_of(R::OtgHTotal.at(1).unwrap());
    let pattern = m.index_of(R::DpgControl.at(1).unwrap());
    let source = m.index_of(R::OptcDataSourceSelect.at(1).unwrap());
    let started = m.log.iter().rposition(|&r| r == R::OtgControl.at(1).unwrap()).unwrap();
    let back_end = m.index_of(R::DigBeCntl.at(0).unwrap());
    assert!(clocks < timing && timing < pattern && pattern < source && source < started && started < back_end);
    let calls: Vec<u8> = m.tables.iter().map(|(_, t)| t.index()).collect();
    assert_eq!(calls, vec![dcn::TABLE_SETPIXELCLOCK, dcn::TABLE_DIGXENCODERCONTROL, dcn::TABLE_DIG1TRANSMITTERCONTROL]);
    let at = |table: u8| m.tables.iter().find(|(_, t)| t.index() == table).unwrap().0;
    let step_of = |reg: Reg| plan.steps().position(|s| matches!(s, Step::Update { reg: r, .. } if r == reg)).unwrap();
    assert!(at(dcn::TABLE_SETPIXELCLOCK) < step_of(R::OtgHTotal.at(1).unwrap()), "the pixel clock before the timing");
    assert!(at(dcn::TABLE_DIG1TRANSMITTERCONTROL) > step_of(R::DigBeCntl.at(0).unwrap()), "the PHY after its back end");
    assert!(matches!(plan.steps().last(), Some(Step::Update { reg, .. }) if reg == R::DigFeCntl.at(0).unwrap()));
}

#[test]
fn first_light_command_table_parameters() {
    let plan = first_light(PIPE1, HDMI_A, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap();
    let tables: Vec<Table> = plan.steps().filter_map(|s| if let Step::Call(t) = s { Some(t) } else { None }).collect();
    let (p, n) = tables[0].parameters();
    assert_eq!(n, 16);
    assert_eq!(&p[..10], &[0xC8, 0xA8, 0x16, 0x00, 20, 0x1E, 2, 0, 1, 0]); // 1 485 000 x 100 Hz, PHY A's PLL, UNIPHY, DVI, OTG 1
    let (p, n) = tables[1].parameters();
    assert_eq!(n, 12);
    assert_eq!(&p[..9], &[0, 0x0F, 2, 4, 0x02, 0x3A, 0, 0, 2]); // DIG A, stream setup, DVI, 4 lanes, 14 850 x 10 kHz, 8 bpc
    let (p, n) = tables[2].parameters();
    assert_eq!(n, 32);
    assert_eq!(&p[..11], &[0, 1, 2, 4, 0x02, 0x3A, 0, 0, 1, 1, 0x0C]); // PHY A on, DVI, 4 lanes, HPD1, front end A, HDMI type A
}

#[test]
fn first_light_refuses_the_firmwares_pipe_bad_instances_and_clocks_beyond_the_signal() {
    let p = |otg, opp, dig_fe| Pipe { otg, opp, dig_fe };
    for pipe in [p(0, 1, 0), p(1, 0, 0), p(1, 1, 4)] {
        assert_eq!(first_light(pipe, HDMI_A, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::Protected);
    }
    let phy_e = Output { phy: 4, ..HDMI_A };
    assert_eq!(first_light(PIPE1, phy_e, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::Protected);
    assert_eq!(first_light(p(4, 1, 0), HDMI_A, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::NoSuchInstance);
    assert_eq!(first_light(PIPE1, Output { phy: 5, ..HDMI_A }, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::NoSuchInstance);
    // The HDMI monitor's own 3440x1440 at 324 MHz: beyond one DVI link, within HDMI without scrambling.
    assert_eq!(first_light(PIPE1, HDMI_A, &HDMI_PREFERRED, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::ClockTooHigh);
    let plan = first_light(PIPE1, HDMI_A, &HDMI_PREFERRED, Signal::Hdmi, Pattern::VerticalBars, FIRMWARE).unwrap();
    let mut m = Model::new();
    m.run(&plan);
    assert_eq!(m.read(R::DigBeCntl.at(0).unwrap()) >> 16 & 7, 3);
    assert_eq!(m.read(R::DpgControl.at(1).unwrap()) >> 4 & 7, 3);
    let mut interlaced = CEA_1080P60;
    interlaced.interlaced = true;
    assert_eq!(first_light(PIPE1, HDMI_A, &interlaced, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap_err(), PlanError::Timing(timing::TimingError::Interlaced));
}

// Every R of the engine, for the cases that walk them all.
const ALL_R: [R; 37] = { use R::*; [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal,
    OtgVTotalMin, OtgVTotalMax, OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl,
    OtgInterlaceControl, OtgStatus, OtgStatusPosition, OtgStatusFrameCount, OtgClockControl, OtgVstartupParam,
    OtgVupdateParam, OtgVreadyParam, OptcDataSourceSelect, OptcDataFormatControl, OptcInputClockControl, VtgControl,
    OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment, DigFeCntl, DigBeCntl, OtgPixelRateCntl,
    OtgPhypllPixelRateCntl, PhypllPixclkResyncCntl, HpdIntStatus, HpdControl, DentistDispclkCntl] };

#[test]
fn no_register_can_name_the_msix_table_or_pending_bits() {
    // KRN's rule: BAR 5 bytes 0x42000 to 0x43FFF are never touched. Every offset there is refused, aligned or not.
    for offset in 0x42000u32..0x44000 { assert_eq!(Reg::new(offset), Err(RegError::MsiX), "{:x}", offset); }
    assert_eq!(dcn::MSIX_BYTES, 0x42000..0x44000);
    // Around it: outside the windows, so refused too, for the other reason.
    assert_eq!(Reg::new(0x41FFC), Err(RegError::Outside));
    assert_eq!(Reg::new(0x44000), Err(RegError::Outside));
    // The windows themselves.
    assert_eq!(Reg::new(0x48).map(Reg::offset), Ok(0x48));
    assert_eq!(Reg::new(0x1AFFC).map(Reg::offset), Ok(0x1AFFC));
    assert_eq!(Reg::new(0x1B000), Err(RegError::Outside));
    assert_eq!(Reg::new(0x44), Err(RegError::Outside));
    assert_eq!(Reg::new(0x4A), Err(RegError::Unaligned));
    for m in dcn::SMU_MAILBOX { assert_eq!(Reg::new(m).map(Reg::offset), Ok(m)); }
    assert_eq!(Reg::new(dcn::SMU_MAILBOX[0] + 4), Err(RegError::Outside));
    assert_eq!(Reg::new(u32::MAX - 3), Err(RegError::Outside));
    // Every register of every instance lies in a window and none in the MSI-X bytes; one past the last instance is None.
    for r in ALL_R {
        for i in 0..r.instances() {
            let reg = r.at(i).unwrap_or_else(|| panic!("{:?} {} has no register", r, i));
            assert!(!dcn::MSIX_BYTES.contains(&reg.offset()));
            assert_eq!(Reg::new(reg.offset()), Ok(reg));
        }
        assert_eq!(r.at(r.instances()), None, "{:?}", r);
    }
}

#[test]
fn every_step_of_first_light_stays_in_the_engine() {
    let plan = first_light(PIPE1, HDMI_A, &CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, FIRMWARE).unwrap();
    for step in plan.steps() {
        if let Step::Update { reg, .. } | Step::Wait { reg, .. } = step {
            assert!(dcn::DCN_BYTES.contains(&reg.offset()) && !dcn::MSIX_BYTES.contains(&reg.offset()));
        }
    }
}
