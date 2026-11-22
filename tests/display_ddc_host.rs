// Host cases of display's step 2 (213-DRV-0095): HDMI-A-1's EDID read over DDC1 on a model of DCN 2.1's I2C engine
// (its arbitration, its indexed buffer, its transactions run on GO against a monitor at 0x50), in Display Core's order;
// the engine given back as batch 11 found it after a read and after every failure; and what the EDID says for the mode.
#![allow(dead_code)]
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;
#[path = "../display/src/ddc.rs"] mod ddc;

use batches::{Bus, Out, Store};
use dcn::{Reg, R};
use light::{Clock, Poke, Writable};
use std::collections::HashMap;
use std::fmt;
use timing::Timing;

const CONTROL: u32 = 0x14D60;
const ARBITRATION: u32 = 0x14D64;
const SW_STATUS: u32 = 0x14D6C;
const HW_STATUS: u32 = 0x14D70;
const SPEED: u32 = 0x14D88;
const SETUP: u32 = 0x14D8C;
const TRANSACTION: u32 = 0x14DB8;
const DATA: u32 = 0x14DC8;
const TIME_BASE: u32 = 0x004EC;
const PADS: u32 = 0x17640;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev { Write(u32, u32), Read(u32) }

// The engine and a monitor on its line.
struct I2c {
    regs: HashMap<u32, u32>, clock: u64, ev: Vec<Ev>,
    buf: [u8; 538], index: usize, reading: bool,
    monitor: Vec<u8>, present: bool, pointer: usize,
    granted: bool, dmcu: bool, hang: bool, busy_until: u64, done: u32,
}

impl I2c {
    fn new(monitor: Vec<u8>) -> I2c {
        let mut i = I2c { regs: HashMap::new(), clock: 0, ev: vec![], buf: [0; 538], index: 0, reading: false, monitor, present: true, pointer: 0,
                          granted: false, dmcu: false, hang: false, busy_until: 0, done: 0 };
        for (o, v) in FOUND { i.regs.insert(o, v); }
        i
    }
    // GO: the transactions run over the buffer, the reply after the bytes written.
    fn go(&mut self) {
        if !self.granted { self.done = 1 << 4; return; }
        if self.hang { self.busy_until = u64::MAX; return; }
        let count = ((self.regs[&CONTROL] >> 20) & 3) as usize + 1;
        let mut p = 0;
        for t in 0..count {
            let tr = self.regs.get(&(TRANSACTION + 4 * t as u32)).copied().unwrap_or(0);
            let (read, n) = (tr & 1 == 1, ((tr >> 16) & 0x3FF) as usize);
            let address = self.buf[p];
            p += 1;
            if !self.present || address >> 1 != 0x50 { self.done = 1 << 8; self.busy_until = self.clock + 300; return; }
            if read {
                for k in 0..n { self.buf[p + k] = self.monitor[(self.pointer + k) % self.monitor.len()]; }
                self.pointer += n;
            } else {
                self.pointer = self.buf[p] as usize;
            }
            p += n;
        }
        self.done = 1 << 2;
        self.busy_until = self.clock + 12_000; // 129 bytes at 100 kHz
    }
}

impl Bus for I2c {
    fn read(&mut self, r: Reg) -> Option<u32> {
        self.clock += 2;
        let o = r.offset();
        self.ev.push(Ev::Read(o));
        Some(match o {
            ARBITRATION => (self.regs[&o] & !0x30_000C) | (if self.dmcu { 2 } else if self.granted { 1 } else { 0 }) << 2,
            SW_STATUS => if self.clock < self.busy_until { 1 } else { self.done },
            DATA => { if !self.reading { return Some(0); } let v = (self.buf[self.index] as u32) << 8 | 1; self.index += 1; v }
            _ => self.regs.get(&o).copied().unwrap_or(0),
        })
    }
}
impl Poke for I2c {
    fn write(&mut self, w: Writable, v: u32) {
        let o = w.reg().offset();
        self.ev.push(Ev::Write(o, v));
        match o {
            ARBITRATION => {
                if v & 1 << 20 != 0 && !self.dmcu { self.granted = true; }
                if v & 1 << 21 != 0 { self.granted = false; }
                self.regs.insert(o, v & !0x30_000C);
            }
            DATA => {
                if v & 1 << 31 != 0 { self.index = ((v >> 16) & 0x3FF) as usize; }
                self.reading = v & 1 == 1;
                if !self.reading { self.buf[self.index] = (v >> 8) as u8; self.index += 1; }
            }
            CONTROL => {
                self.regs.insert(o, v);
                if v & 8 != 0 { self.done = 0; if self.busy_until != u64::MAX { self.busy_until = 0; } }
                if v & 1 != 0 { self.go(); }
            }
            _ => { self.regs.insert(o, v); }
        }
    }
}
impl Clock for I2c {
    fn now_us(&mut self) -> u64 { self.clock }
    fn delay_us(&mut self, us: u32) { self.clock += us as u64; }
}

struct Log(Vec<String>);
impl Out for Log {
    fn line(&mut self, a: fmt::Arguments) { self.0.push(a.to_string()); }
    fn pause(&mut self, _: u32) {}
}

// The engine as a release by Display Core leaves it, and batch 11 would read it: nobody holding it, in soft reset,
// DDC1's set-up and its HDCP speed of 5 kHz, last transactions as an EDID read leaves them, the pads in GPIO mode.
const FOUND: [(u32, u32); 12] = [(ARBITRATION, 0), (CONTROL, 0x0000_0002), (SW_STATUS, 0), (HW_STATUS, 0), (SPEED, 0x12C0_0102),
    (SETUP, 0x0300_0044), (TRANSACTION, 0x0001_1100), (TRANSACTION + 4, 0x0080_3101), (TRANSACTION + 8, 0), (TRANSACTION + 12, 0),
    (TIME_BASE, 0x0000_0130), (PADS, 0x0000_0101)];

fn store(found: &[(u32, u32)], monitor_plugged: bool) -> Store {
    let mut s = Store::default();
    for &(o, v) in found {
        let (r, i) = dcn::ALL.iter().flat_map(|&r| (0..r.instances()).map(move |i| (r, i))).find(|&(r, i)| r.at(i).map(|x| x.offset()) == Some(o)).unwrap();
        s.set(r, i, v);
    }
    s.set(R::HpdIntStatus, 0, (monitor_plugged as u32) << 1);
    s.set_batches(11);
    s
}

// --- a synthetic monitor shaped like the Ryzen's HDMI one (its own bytes are not kept) ---

const WIDE_60: Timing = Timing { pixel_khz: 324_000, h_active: 3440, h_front: 48, h_sync: 32, h_back: 80, v_active: 1440, v_front: 3, v_sync: 5,
    v_back: 52, h_sync_positive: true, v_sync_positive: false, interlaced: false };
const WIDE_30: Timing = Timing { pixel_khz: 162_000, ..WIDE_60 };

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
fn descriptor(tag: u8, payload: &[u8]) -> [u8; 18] { let mut d = [0u8; 18]; d[3] = tag; d[5..5 + payload.len()].copy_from_slice(payload); d }
fn seal(block: &mut [u8]) { let sum = block[..127].iter().fold(0u8, |s, b| s.wrapping_add(*b)); block[127] = 0u8.wrapping_sub(sum); }

fn monitor(wide_30: bool, extension: bool) -> Vec<u8> { monitor_with(WIDE_60, wide_30, extension, true) }
fn monitor_with(preferred: Timing, wide_30: bool, extension: bool, hdmi: bool) -> Vec<u8> {
    let mut b = vec![0u8; 256];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    let id: u16 = (((b'W' - b'A' + 1) as u16) << 10) | (((b'I' - b'A' + 1) as u16) << 5) | (b'D' - b'A' + 1) as u16;
    b[8..10].copy_from_slice(&id.to_be_bytes());
    b[10..12].copy_from_slice(&0x3440u16.to_le_bytes());
    b[17] = 31; b[18] = 1; b[19] = 4; b[20] = 0x80 | 2 << 4; b[21] = 80; b[22] = 34;
    for s in b[38..54].chunks_mut(2) { s.copy_from_slice(&[1, 1]); }
    b[38] = (1920 / 8 - 31) as u8; b[39] = 3 << 6; // 1920x1080 at 60 Hz
    b[40] = (1680 / 8 - 31) as u8; b[41] = 0; // 1680x1050 (16:10) at 60 Hz
    b[54..72].copy_from_slice(&dtd(&preferred));
    b[72..90].copy_from_slice(&if wide_30 { dtd(&WIDE_30) } else { descriptor(0x10, &[]) });
    b[90..108].copy_from_slice(&descriptor(0xFD, &[24, 85, 30, 160, 60]));
    b[108..126].copy_from_slice(&descriptor(0xFC, b"WIDE MONITOR\n"));
    b[126] = extension as u8;
    seal(&mut b[..128]);
    let e = &mut b[128..];
    e[0] = 0x02; e[1] = 3;
    let mut blocks: Vec<u8> = vec![0x40 | 3, 16 | 0x80, 4, 31];
    if hdmi {
        blocks.extend_from_slice(&[0x60 | 7, 0x03, 0x0C, 0x00, 0x10, 0x00, 0x38, 68]); // HDMI 1.4: 1.0.0.0, 340 MHz
        blocks.extend_from_slice(&[0x60 | 6, 0xD8, 0x5D, 0xC4, 1, 120, 0x80]); // HDMI Forum: 600 MHz
    }
    e[4..4 + blocks.len()].copy_from_slice(&blocks);
    e[2] = (4 + blocks.len()) as u8;
    seal(e);
    if !extension { b.truncate(128); }
    b
}

fn run(i2c: &mut I2c, s: &Store) -> (Result<ddc::Read, ddc::Failure>, Vec<String>) {
    let mut log = Log(vec![]);
    let r = ddc::read(s, i2c, &mut log);
    (r, log.0)
}
fn writes(i2c: &I2c) -> Vec<(u32, u32)> { i2c.ev.iter().filter_map(|e| if let Ev::Write(o, v) = e { Some((*o, *v)) } else { None }).collect() }
// Every register step 2 put back is as batch 11 read it, and nobody holds the engine.
fn as_found(i2c: &I2c) {
    for (o, v) in FOUND { if [PADS, CONTROL, SPEED, SETUP, TRANSACTION, TRANSACTION + 4].contains(&o) { assert_eq!(i2c.regs[&o], v, "{:05X}", o); } }
    assert!(!i2c.granted, "the engine still held");
}

#[test]
fn step_2_reads_both_blocks_in_display_cores_order_and_gives_the_engine_back_as_found() {
    let edid = monitor(true, true);
    let mut i2c = I2c::new(edid.clone());
    let (r, log) = run(&mut i2c, &store(&FOUND, true));
    let read = r.unwrap_or_else(|why| panic!("{} {:#?}", why, log));
    assert_eq!((read.blocks, read.bytes()), (2, &edid[..]));
    // Display Core's order: the pads, out of soft reset, the engine asked for, the speed, the control, the set-up,
    // the arbitration; then for block 0 its transactions and buffer, the reset of its status, the set-up, GO.
    let w = writes(&i2c);
    let order: Vec<u32> = w.iter().map(|x| x.0).take(19).collect();
    assert_eq!(order, [PADS, CONTROL, ARBITRATION, SPEED, CONTROL, SETUP, ARBITRATION,
                       TRANSACTION, DATA, DATA, TRANSACTION + 4, DATA, CONTROL, SETUP, CONTROL, CONTROL, DATA, TRANSACTION, DATA]);
    // The pads in hardware I2C (masks 0, pull-downs on, not AUX); 100 kHz from the 48 MHz time base; DDC1 enabled with
    // DCN's time limit 3.
    assert_eq!(w[0], (PADS, 0x0000_1010));
    assert_eq!(w[3].1 >> 16, 480);
    assert_eq!(w[5].1 & 0xFF00_0044, 0x0300_0044);
    // The offset written and 128 bytes read: the buffer's index set before the reply is read, never read before.
    assert_eq!(&w[8..12], &[(DATA, 0x8000_A000), (DATA, 0x0000_0000), (TRANSACTION + 4, 0x0080_3101), (DATA, 0x0000_A100)]);
    assert_eq!((w[15].0, w[15].1 & 1), (CONTROL, 1), "GO");
    assert_eq!(w[16], (DATA, 0x8003_0001));
    let first_read = i2c.ev.iter().position(|e| *e == Ev::Read(DATA)).unwrap();
    let reply = i2c.ev.iter().position(|e| *e == Ev::Write(DATA, 0x8003_0001)).unwrap();
    assert!(reply < first_read);
    // Block 1 at offset 128.
    assert!(w.contains(&(DATA, 0x0000_8000)));
    // Only step 2's registers written.
    for (o, _) in &w { assert!(light::ddc_writable(Reg::new(*o).unwrap()).is_ok(), "{:05X}", o); }
    // The engine given back and as found.
    as_found(&i2c);
    assert!(log.iter().any(|x| x == "D THE ENGINE GIVEN BACK: 18 WRITES PUT BACK; 6 OF 6 REGISTERS AS BATCH 11 READ THEM"), "{:#?}", &log[log.len() - 8..]);
    assert!(log.iter().any(|x| x == "D BLOCK 1: 128 BYTES READ, CHECKSUM GOOD"));
    // The time from GO, with no log line between it and the poll: the model's 12 ms on the wire, to a poll's 0.1 ms.
    let line = log.iter().find(|x| x.starts_with("D BLOCK 0: FROM GO TO THE TRANSFER'S END ")).unwrap();
    let us: u64 = line.split("END ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!((12_000..12_200).contains(&us), "{}", line);
    let go = log.iter().position(|x| x.starts_with("D WROTE DC_I2C_CONTROL AT 14D60 = ") && x.ends_with(", LOGGED AFTER THE TRANSFER")).unwrap();
    assert!(log[go + 1].starts_with("D BLOCK 0: FROM GO"));
    assert!(log.iter().any(|x| x == "D THE ENGINE NOW HELD BY NOBODY; BATCH 11 FOUND IT HELD BY NOBODY"));
    assert!(log.last().unwrap() == "D STEP 2 DONE: 2 BLOCKS READ (256 BYTES)");
    // A base block that announces no extension: one block read.
    let mut i2c = I2c::new(monitor(true, false));
    let (r, _) = run(&mut i2c, &store(&FOUND, true));
    assert_eq!(r.unwrap().blocks, 1);
    as_found(&i2c);
}

#[test]
fn a_failed_read_ends_step_2_with_the_engine_given_back_and_nothing_else_written() {
    // No monitor answering on the line: a NACK on block 0.
    let mut i2c = I2c::new(monitor(true, true));
    i2c.present = false;
    let (r, log) = run(&mut i2c, &store(&FOUND, true));
    assert!(matches!(r, Err(ddc::Failure::Nack { block: 0, .. })), "{:?}", r);
    as_found(&i2c);
    assert!(log.last().unwrap().starts_with("D STEP 2 NOT DONE: BLOCK 0: NO ACKNOWLEDGE") && log.last().unwrap().ends_with("; STEPS 3 AND 4 GO ON WITHOUT IT"));
    // An engine that never ends the transfer: no answer within the limit, the engine reset as it is given back.
    let mut i2c = I2c::new(monitor(true, true));
    i2c.hang = true;
    let (r, log) = run(&mut i2c, &store(&FOUND, true));
    assert!(matches!(r, Err(ddc::Failure::NoAnswer { block: 0, status: 1 })), "{:?}", r);
    assert!(i2c.clock >= ddc::WAIT_LIMIT_US && i2c.clock < 2 * ddc::WAIT_LIMIT_US);
    assert!(writes(&i2c).iter().any(|&(o, v)| o == CONTROL && v & 0xA == 0xA), "SOFT_RESET and SW_STATUS_RESET at the give-back");
    as_found(&i2c);
    assert!(log.iter().any(|x| x.starts_with("D STOPPED: BLOCK 0: NO END WITHIN 500 MS")));
    // A bad checksum on the extension.
    let mut bad = monitor(true, true);
    bad[200] ^= 0xFF;
    let mut i2c = I2c::new(bad);
    let (r, _) = run(&mut i2c, &store(&FOUND, true));
    assert_eq!(r.err(), Some(ddc::Failure::Checksum { block: 1 }));
    as_found(&i2c);
    // Held by the DMCU, not read in step 1, no monitor on HPD 1: not a single write.
    let mut i2c = I2c::new(monitor(true, true));
    i2c.dmcu = true;
    assert!(matches!(run(&mut i2c, &store(&FOUND, true)).0, Err(ddc::Failure::Held { .. })));
    assert!(writes(&i2c).is_empty());
    let mut i2c = I2c::new(monitor(true, true));
    assert_eq!(run(&mut i2c, &store(&[], true)).0.err(), Some(ddc::Failure::NotRead));
    assert_eq!(run(&mut i2c, &store(&FOUND, false)).0.err(), Some(ddc::Failure::NoMonitor));
    assert!(writes(&i2c).is_empty());
}

#[test]
fn step_2s_guard_lets_through_the_i2c_engine_and_ddc1s_pads_only() {
    for o in [CONTROL, ARBITRATION, SPEED, SETUP, TRANSACTION, TRANSACTION + 4, TRANSACTION + 8, TRANSACTION + 12, DATA, PADS] {
        assert!(light::ddc_writable(Reg::new(o).unwrap()).is_ok(), "{:05X}", o);
    }
    for (r, i) in [(R::DcI2cSwStatus, 0), (R::DcI2cDdc1HwStatus, 0), (R::MicrosecondTimeBaseDiv, 0), (R::DcGpioDdc1Y, 0), (R::HubpCntl, 1), (R::DpgControl, 1), (R::OtgControl, 1)] {
        assert!(light::ddc_writable(r.at(i).unwrap()).is_err(), "{}", r.name());
    }
}

#[test]
fn what_the_edid_says_for_the_mode_and_a_dvi_mode_nearer_its_shape() {
    let mut log = Log(vec![]);
    let read = ddc::Read { bytes: monitor(true, true).try_into().unwrap(), blocks: 2 };
    assert!(ddc::describe(&read, &mut log).is_some());
    let has = |s: &str| assert!(log.0.iter().any(|x| x == s), "no {:?} in {:#?}", s, log.0);
    has("D PREFERRED: 3440x1440 AT 60.000 HZ, 324000 KHZ, IN 3600x1500");
    has("D AN HDMI SINK: TMDS AT MOST 600 MHZ (HDMI 1.4 BLOCK 340 MHZ, HDMI FORUM BLOCK 600 MHZ)");
    has("D TIMING 1: 3440x1440 AT 60.000 HZ, 324000 KHZ; DVI DOES NOT");
    has("D TIMING 2: 3440x1440 AT 30.000 HZ, 162000 KHZ; DVI CARRIES IT");
    has("D STANDARD TIMING: 1920x1080 AT 60 HZ (ITS CLOCK NOT STATED)");
    has("D STANDARD TIMING: 1680x1050 AT 60 HZ (ITS CLOCK NOT STATED)");
    has("D WITHIN DVI'S 165 MHZ AND NEARER ITS SHAPE THAN 1920x1080: 3440x1440 AT 30.000 HZ, 162000 KHZ");
    assert!(log.0[0].starts_with("D EDID 1.4, FNV-1A ") && log.0[0].ends_with(": WID PRODUCT 3440, 2021, \"WIDE MONITOR\"; 1 EXTENSIONS ANNOUNCED, 2 BLOCKS READ"), "{}", log.0[0]);
    // Without the 30 Hz timing: its VICs are 16:9, so none nearer its shape within DVI's limit.
    let mut log = Log(vec![]);
    let read = ddc::Read { bytes: monitor(false, true).try_into().unwrap(), blocks: 2 };
    ddc::describe(&read, &mut log);
    assert!(log.0.iter().any(|x| x == "D WITHIN DVI'S 165 MHZ, NO TIMING IT GIVES IN FULL IS NEARER ITS SHAPE THAN 1920x1080"), "{:#?}", log.0);
}

#[test]
fn the_mode_is_the_edids_preferred_in_hdmi_when_it_fits_else_todays() {
    use dcn::Signal;
    use light::{mode_from_edid, ModeFrom};
    let today = timing::CEA_1080P60;
    let edid = |b: Vec<u8>| edid::Edid::parse(&b).unwrap();
    // The Ryzen's monitor: 3440x1440 at 60 Hz, 324 MHz, an HDMI sink.
    assert_eq!(mode_from_edid(Some(&edid(monitor(true, true))), today, Signal::Dvi), (WIDE_60, Signal::Hdmi, ModeFrom::Edid));
    // Today's mode, in the signalling asked for, otherwise.
    assert_eq!(mode_from_edid(None, today, Signal::Dvi), (today, Signal::Dvi, ModeFrom::NotRead));
    assert_eq!(mode_from_edid(Some(&edid(monitor_with(WIDE_60, false, true, false))), today, Signal::Hdmi), (today, Signal::Hdmi, ModeFrom::NotHdmi));
    let wide_100 = Timing { pixel_khz: 540_000, ..WIDE_60 };
    assert_eq!(mode_from_edid(Some(&edid(monitor_with(wide_100, false, true, true))), today, Signal::Dvi).2, ModeFrom::OverClock { khz: 540_000, limit: 340_000 });
    let uhd_30 = Timing { pixel_khz: 297_000, h_active: 3840, h_front: 176, h_sync: 88, h_back: 296, v_active: 2160, v_front: 8, v_sync: 10, v_back: 72,
        h_sync_positive: true, v_sync_positive: true, interlaced: false };
    assert_eq!(mode_from_edid(Some(&edid(monitor_with(uhd_30, false, true, true))), today, Signal::Dvi).2, ModeFrom::Surface { bytes: 3840 * 2160 * 4 });
}
