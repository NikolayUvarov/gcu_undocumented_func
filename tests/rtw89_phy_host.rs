#![allow(dead_code)]
#[path = "../rtw89_wifi/src/io.rs"] mod io;
#[path = "../rtw89_wifi/src/registers.rs"] mod registers;
#[path = "../rtw89_wifi/src/power.rs"] mod power;
#[path = "../rtw89_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw89_wifi/src/board.rs"] mod board;
#[path = "../rtw89_wifi/src/phy_tables.rs"] mod phy_tables;
#[path = "../rtw89_wifi/src/phy.rs"] mod phy;

use io::{Fault, FaultKind, RegisterIo, Result};
use phy::{Phy, PhyIo, State, Stage};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event { Write(usize, usize, u32), H2c(u8, Vec<u8>, u32) }

struct Device {
    memory: Vec<u8>, id: u32, command: u16, now: u64, frozen: bool,
    events: Vec<Event>, reads: usize, writes: usize, fail_write: Option<usize>,
    fail_cleanup: Option<usize>, swsi_stuck: bool, swsi_busy: u8, nctl_stuck: bool,
    h2c_delay: u64, h2c_fail: Option<u8>, deadline_write: Option<usize>,
}

fn identity() -> board::Identity {
    board::Identity { cut: 1, mac: [2, 3, 4, 5, 6, 7], rfe: 1, xtal: 0x20, country: *b"US", channel_plan: 0x7f }
}

impl Device {
    fn new() -> Self {
        let mut d = Self { memory: vec![0; 0x20000], id: 0xb852_10ec, command: 6, now: 1000, frozen: false,
            events: Vec::new(), reads: 0, writes: 0, fail_write: None, fail_cleanup: None,
            swsi_stuck: false, swsi_busy: 0, nctl_stuck: false, h2c_delay: 0, h2c_fail: None, deadline_write: None };
        for (at, value) in [(0xf0, 0x1000), (0x3f0, 0x100), (0x88, 3), (0x1e0, 0xe0),
            (0x8400, 0xfffd0000), (0x8404, 0x0f9f0000), (0xc000, 0xf000003f),
            (0xc004, 0x4000003f), (0x2f0, 0x8282), (0x1000, 0x800), (0x1010, 0xb0f03)] {
            d.put(at, 4, value);
        }
        d.put(2, 1, 3); d.put(0x8040, 1, 8);
        d
    }
    fn tick(&mut self, us: u64) { if !self.frozen { self.now += us; } }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.memory[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn get(&self, at: usize, width: usize) -> u32 {
        let mut raw = [0; 4]; raw[..width].copy_from_slice(&self.memory[at..at + width]); u32::from_le_bytes(raw)
    }
    fn read(&mut self, at: usize, width: usize) -> Result<u32> {
        io::check_offset(at, width, self.memory.len())?;
        self.reads += 1; self.tick(1);
        if at == 0x1174c && width == 4 {
            if self.swsi_stuck { return Ok(1 << 24); }
            if self.swsi_busy != 0 { self.swsi_busy -= 1; return Ok(1 << 24); }
            return Ok(0);
        }
        if at == 0x18080 && self.nctl_stuck { return Ok(0); }
        Ok(self.get(at, width))
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> Result<()> {
        io::check_offset(at, width, self.memory.len())?;
        self.tick(1); self.writes += 1;
        self.events.push(Event::Write(at, width, value));
        self.put(at, width, value);
        if self.deadline_write == Some(at) { self.now += 100_000; self.deadline_write = None; }
        if self.fail_write == Some(self.writes) || self.fail_cleanup == Some(at) {
            return Err(Fault::new(FaultKind::Access, at, value));
        }
        if at == 0x10370 { self.swsi_busy = 1; }
        Ok(())
    }
    fn reset(&self) {
        assert_eq!(self.get(2, 1) & 3, 0);
        assert_eq!(self.get(0x2f0, 4) & 0x8282, 0);
        assert_eq!(self.get(0x8040, 1) & 8, 0);
        assert_eq!(self.command, 6, "PHY cleanup cannot release or revoke the command DMA region");
    }
    fn pages(&self) -> Vec<(u8, &[u8], u32)> {
        self.events.iter().filter_map(|event| match event { Event::H2c(path, bytes, us) => Some((*path, bytes.as_slice(), *us)), _ => None }).collect()
    }
}

impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> Result<u8> { self.read(at, 1).map(|v| v as u8) }
    fn read16(&mut self, at: usize) -> Result<u16> { self.read(at, 2).map(|v| v as u16) }
    fn read32(&mut self, at: usize) -> Result<u32> { self.read(at, 4) }
    fn write8(&mut self, at: usize, v: u8) -> Result<()> { self.write(at, 1, v as u32) }
    fn write16(&mut self, at: usize, v: u16) -> Result<()> { self.write(at, 2, v as u32) }
    fn write32(&mut self, at: usize, v: u32) -> Result<()> { self.write(at, 4, v) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> Result<()> { self.tick(us as u64); Ok(()) }
}

impl PhyIo for Device {
    fn pci_id(&mut self) -> Result<u32> { Ok(self.id) }
    fn pci_command(&mut self) -> Result<u16> { Ok(self.command) }
    fn bar_len(&self) -> usize { self.memory.len() }
    fn send_rf_page(&mut self, path: u8, page: u8, payload: &[u8], timeout_us: u32) -> Result<()> {
        assert_eq!(page, 0); assert!(timeout_us > 0 && timeout_us <= 100_000);
        self.events.push(Event::H2c(path, payload.to_vec(), timeout_us));
        self.tick(self.h2c_delay);
        if self.h2c_fail == Some(path) { return Err(Fault::new(FaultKind::Access, 0x1080, path as u32)); }
        Ok(())
    }
}

fn fingerprint(bytes: impl IntoIterator<Item=u8>) -> u64 {
    bytes.into_iter().fold(0xcbf29ce484222325u64, |h, byte| (h ^ byte as u64).wrapping_mul(0x100000001b3))
}

#[test]
fn exact_selected_tables_retain_order_duplicates_and_pinned_fingerprints() {
    for (table, count, expected) in [(phy_tables::BB, 1013, 0xa12644d2044daf01),
        (phy_tables::GAIN, 66, 0xf912eaf4134c1d8b), (phy_tables::RADIO_A, 947, 0x5feb3aa0be1abf02),
        (phy_tables::RADIO_B, 932, 0x795603f15d8e6482), (phy_tables::NCTL, 1320, 0x3a3a899c10b5e551)] {
        assert_eq!(table.len(), count);
        assert_eq!(fingerprint(table.iter().flat_map(|(a,v)| a.to_le_bytes().into_iter().chain(v.to_le_bytes()))), expected);
    }
}

#[test]
fn wrong_device_board_firmware_dlfw_only_and_live_data_paths_cannot_write() {
    for (at, width, value) in [(0xf0,4,0x2000), (0x3f0,4,0), (0x3f0,4,0x300), (0x88,4,1), (0x1e0,4,0xc0),
        (0x1e0,4,u32::MAX), (0x8400,4,0x64c40000), (0x8404,4,0x04840000), (0xc000,4,0),
        (0xc004,4,0), (0xe000,4,1<<30), (2,1,0), (0x2f0,4,0), (0x8040,1,0),
        (0x1000,4,0x2800), (0x1010,4,0), (0x1010,4,0xf0f03), (0x101c,4,1<<16),
        (0xc348,4,1), (0xc6ec,4,1<<7)] {
        let mut d = Device::new(); d.put(at, width, value);
        let mut p = Phy::new(&mut d, identity());
        assert!(p.baseband().is_err(), "register {at:x}");
        assert_eq!(p.state(), State::Failed);
        assert!(d.events.is_empty(), "guard must not reset an unknown active device");
    }
    for option in 0..7 {
        let mut d = Device::new(); let mut board = identity();
        match option { 0=>d.command=2, 1=>d.command=u16::MAX, 2=>d.id=0xb85b10ec,
            3=>d.memory.resize(0x1ffff,0), 4=>board.cut=0, 5=>board.rfe=2, _=>d.command=7 }
        assert!(Phy::new(&mut d, board).baseband().is_err()); assert!(d.events.is_empty());
    }
}

#[test]
fn bb_applies_full_mcu_offset_and_preserves_repeated_writes() {
    let mut d = Device::new();
    let mut p = Phy::new(&mut d, identity()); p.baseband().unwrap();
    assert_eq!(p.state(), State::BasebandLoaded);
    assert_eq!(d.writes, 1013);
    assert_eq!(d.events[0], Event::Write(0x10704,4,0x601e0100));
    assert_eq!(d.events.last().unwrap(), &Event::Write(0x100f8,4,0x20220408));
    let repeated: Vec<_> = d.events.iter().filter(|e| matches!(e, Event::Write(0x10000,4,_))).cloned().collect();
    assert_eq!(repeated, [Event::Write(0x10000,4,0xc580801e), Event::Write(0x10000,4,0x0580801f)]);
    assert!(d.events.iter().all(|e| matches!(e, Event::Write(at,4,_) if *at>=0x10000 && *at<0x20000)));
}

#[test]
fn gain_values_are_signed_software_defaults_and_not_register_writes() {
    let gain = phy::gain_defaults();
    assert_eq!(gain.lna[0][0], [-73,-35,-5,24,54,84,111]);
    assert_eq!(gain.tia[0][0], [49,79]);
    assert_eq!(gain.offset20[0], [-12,-8]);
    assert_eq!(gain.offset40[0][0], [-8,-8,-8]);
    assert_eq!(gain.offset80[1][0], [-8,-24,-24,8,8,0,0,0,0,-8,-8]);
}

#[test]
fn radio_requires_bb_and_nctl_requires_radio_without_touching_hardware() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    assert!(p.radio().is_err()); assert!(p.nctl().is_err());
    assert!(p.bus_between_stages().events.is_empty());
    p.baseband().unwrap(); let before=p.bus_between_stages().events.len();
    assert!(p.nctl().is_err()); assert_eq!(p.bus_between_stages().events.len(),before);
}

#[test]
fn both_rf_paths_use_swsi_and_direct_windows_then_mirror_the_digital_words() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    p.baseband().unwrap(); p.bus_between_stages().events.clear(); p.radio().unwrap();
    assert_eq!(p.state(),State::RadioLoaded);
    assert_eq!(d.events[0], Event::Write(0x10370,4,0x30000));
    let pages=d.pages(); assert_eq!(pages.len(),2);
    for (index,(path,bytes,_)) in pages.iter().enumerate() {
        assert_eq!(*path,index as u8); assert_eq!(bytes.len(),1176);
        assert_eq!(fingerprint(bytes.iter().copied()),0x5592e87011be1da7);
        assert_eq!(&bytes[..8], &[0,0,3,0,0x24,0x11,0x81,1]);
    }
    let page_a=d.events.iter().position(|e| matches!(e,Event::H2c(0,_,_))).unwrap();
    let path_b=d.events.iter().position(|e| matches!(e,Event::Write(0x10370,4,v) if v&0x70000000==0x10000000)).unwrap();
    assert!(page_a<path_b);
    assert!(d.events.iter().any(|e| matches!(e,Event::Write(at,4,_) if (0x1e000..0x1e400).contains(at))));
    assert!(d.events.iter().any(|e| matches!(e,Event::Write(at,4,_) if (0x1f000..0x1f400).contains(at))));
    assert!(!d.events.iter().any(|e| matches!(e,Event::Write(at,_,_) if *at<0x10000)));
}

#[test]
fn rf_h2c_error_is_not_ignored_and_prevents_later_paths_or_stages() {
    for path in [0,1] {
        let mut d=Device::new(); d.h2c_fail=Some(path);
        let mut p=Phy::new(&mut d,identity()); p.baseband().unwrap();
        let error=p.radio().unwrap_err(); assert_eq!(error.cause.register,0x1080);
        assert_eq!(error.stage,if path==0 { Stage::RadioA } else { Stage::RadioB });
        assert_eq!(p.state(),State::Failed); assert!(error.disable_bb_rf.is_none());
        let before=p.bus_between_stages().events.len(); assert!(p.radio().is_err()); assert!(p.nctl().is_err());
        assert_eq!(p.bus_between_stages().events.len(),before);
        assert_eq!(d.pages().len(),path as usize+1); d.reset();
    }
}

#[test]
fn two_rf_paths_and_transport_share_one_deadline_including_late_success() {
    let mut d=Device::new(); d.h2c_delay=60_000;
    let mut p=Phy::new(&mut d,identity()); p.baseband().unwrap();
    let error=p.radio().unwrap_err(); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.stage,Stage::RadioB);
    let pages=d.pages(); assert_eq!(pages.len(),2); assert!(pages[1].2<pages[0].2-60_000);
    d.reset();
}

#[test]
fn frozen_clock_with_stuck_swsi_has_a_finite_poll_limit() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); p.baseband().unwrap();
    let d=p.bus_between_stages(); d.frozen=true; d.swsi_stuck=true; let before=d.reads;
    let error=p.radio().unwrap_err(); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.cause.register,0x1174c); assert!(p.bus_between_stages().reads-before<100);
    assert!(p.bus_between_stages().pages().is_empty()); p.bus_between_stages().reset();
}

#[test]
fn partial_bb_write_failure_is_poisoned_and_reports_independent_cleanup_failure() {
    let mut d=Device::new(); d.fail_write=Some(20); d.fail_cleanup=Some(0x2f0);
    let mut p=Phy::new(&mut d,identity()); let error=p.baseband().unwrap_err();
    assert_eq!(error.cause.kind,FaultKind::Access); assert!(error.cause.register>=0x10000);
    assert_eq!(error.disable_bb_rf.unwrap().register,0x2f0); assert_eq!(p.state(),State::Failed);
    assert!(d.events.iter().any(|e| matches!(e,Event::Write(0x8040,1,0))));
    assert_eq!(d.get(2,1)&3,0);
}

#[test]
fn last_bb_write_reaching_total_deadline_cannot_report_success() {
    let mut d=Device::new(); d.deadline_write=Some(0x100f8);
    let mut p=Phy::new(&mut d,identity()); let error=p.baseband().unwrap_err();
    assert_eq!(error.cause.kind,FaultKind::Timeout); assert_eq!(p.state(),State::Failed); d.reset();
}

#[test]
fn nctl_checks_ready_before_loading_program_and_preserves_final_table_order() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    p.baseband().unwrap(); p.radio().unwrap(); p.bus_between_stages().events.clear(); p.nctl().unwrap();
    assert_eq!(p.state(),State::TablesLoaded);
    let writes=&d.events;
    assert_eq!(writes[5],Event::Write(0x18000,4,8));
    assert_eq!(writes[6],Event::Write(0x18080,4,4));
    assert_eq!(writes.len(),1327);
    assert!(writes.iter().all(|e| matches!(e,Event::Write(at,4,_) if *at>=0x10000 && *at<0x20000)));
    assert_eq!(d.get(0xc348,4),0); assert_eq!(d.command,6);
}

#[test]
fn stuck_nctl_blocks_program_download_and_cleanup_has_its_own_budget() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    p.baseband().unwrap(); p.radio().unwrap(); let d=p.bus_between_stages(); d.events.clear(); d.nctl_stuck=true;
    let before=d.now; let error=p.nctl().unwrap_err();
    assert_eq!(error.stage,Stage::Nctl); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert!(error.disable_bb_rf.is_none());
    let d=p.bus_between_stages(); assert!(d.now-before<2000); d.reset();
}

#[test]
fn subsequent_stage_rechecks_real_firmware_state_and_does_not_write_on_refusal() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); p.baseband().unwrap();
    let d=p.bus_between_stages(); d.put(0x1e0,4,0); let before=d.events.len();
    assert!(p.radio().is_err()); assert_eq!(p.state(),State::Failed);
    assert_eq!(p.bus_between_stages().events.len(),before);
}
