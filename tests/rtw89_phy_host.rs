#![allow(dead_code)]
#[path = "../rtw89_wifi/src/io.rs"] mod io;
#[path = "../rtw89_wifi/src/registers.rs"] mod registers;
#[path = "../rtw89_wifi/src/power.rs"] mod power;
#[path = "../rtw89_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw89_wifi/src/board.rs"] mod board;
#[path = "../rtw89_wifi/src/phy_tables.rs"] mod phy_tables;
#[path = "../rtw89_wifi/src/phy.rs"] mod phy;
#[path = "../rtw89_wifi/src/dack.rs"] mod dack;
#[path = "../rtw89_wifi/src/dack_tables.rs"] mod dack_tables;

use io::{Fault, FaultKind, RegisterIo, Result};
use phy::{Phy, PhyIo, State, Stage};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event { Write(usize, usize, u32), H2c(u8, Vec<u8>, u32) }

struct Device {
    btc: Vec<(dack::BtcNotice,u32)>, btc_event_positions: Vec<usize>, btc_start_fail: bool, btc_stop_fail: bool, btc_start_delay: u64, btc_stop_delay: u64, dack_model: bool,
    rf: [[u32; 256]; 2], swsi_pending: Option<(u64,u8,u8)>, swsi_latched: u32, read_stuck: bool,
    rck_pending: [Option<u64>;2], rck_values: [u8;2], rck_delay: u64, rck_stuck: Option<u8>,
    rck_bus_stuck: bool, restore_fail: bool, late_restore: bool,
    memory: Vec<u8>, xtal: [u8; 256], xtal_stuck: bool, xtal_delay_us: u64, xtal_pending: Option<(u64, u32)>, id: u32, command: u16, now: u64, frozen: bool,
    events: Vec<Event>, reads: usize, writes: usize, fail_write: Option<usize>,
    fail_cleanup: Option<usize>, swsi_stuck: bool, swsi_busy: u8, nctl_stuck: bool,
    now_calls: usize, jump_call: Option<usize>, h2c_delay: u64, h2c_fail: Option<u8>, deadline_write: Option<usize>, deadline_us: u64, deadline_write_number: Option<usize>, delay_jump: Option<u32>,
}

fn identity() -> board::Identity {
    board::Identity { cut: 1, mac: [2, 3, 4, 5, 6, 7], rfe: 1, xtal: 0x20, country: *b"US", channel_plan: 0x7f }
}

impl Device {
    fn new() -> Self {
        let mut d = Self { btc: Vec::new(), btc_event_positions: Vec::new(), btc_start_fail: false, btc_stop_fail: false, btc_start_delay: 0, btc_stop_delay: 0, dack_model: false, rf: [[0;256];2], swsi_pending: None, swsi_latched: 0, read_stuck: false,
            rck_pending: [None;2], rck_values: [7,11], rck_delay: 4, rck_stuck: None, rck_bus_stuck: false, restore_fail: false, late_restore: false, memory: vec![0; 0x20000], xtal: [0; 256], xtal_stuck: false, xtal_delay_us: 0, xtal_pending: None, id: 0xb852_10ec, command: 6, now: 1000, frozen: false,
            events: Vec::new(), reads: 0, writes: 0, fail_write: None, fail_cleanup: None,
            swsi_stuck: false, swsi_busy: 0, nctl_stuck: false, now_calls: 0, jump_call: None, h2c_delay: 0, h2c_fail: None, deadline_write: None, deadline_us: 100_000, deadline_write_number: None, delay_jump: None };
        for (at, value) in [(0xf0, 0x1000), (0x3f0, 0x100), (0x88, 3), (0x1e0, 0xe0),
            (0x8400, 0xfffd0000), (0x8404, 0x0f9f0000), (0xc000, 0xf000003f),
            (0xc004, 0x4000003f), (0x200, 1 << 17), (0x2f0, 1 << 17), (0x1000, 0x800), (0x1010, 0xb0f03)] {
            d.put(at, 4, value);
        }
        d.put(2, 1, 3); d.put(0x8040, 1, 14); d.xtal[0x80]=0xc7; d.xtal[0x81]=0xc7;
        d
    }
    fn tick(&mut self, us: u64) { if !self.frozen { self.now += us; } }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.memory[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn get(&self, at: usize, width: usize) -> u32 {
        let mut raw = [0; 4]; raw[..width].copy_from_slice(&self.memory[at..at + width]); u32::from_le_bytes(raw)
    }
    fn complete_xtal(&mut self, value: u32) {
        let offset=(value&255) as usize;
        if value & (1<<24) != 0 { self.put(0x270,4,offset as u32 | (self.xtal[offset] as u32)<<8); }
        else {
            let mask=(value>>16) as u8;
            self.xtal[offset]=(self.xtal[offset]&!mask)|((value>>8) as u8 & mask);
            self.put(0x270,4,value&!(1<<31));
        }
    }
    fn read(&mut self, at: usize, width: usize) -> Result<u32> {
        io::check_offset(at, width, self.memory.len())?;
        self.reads += 1; self.tick(1);
        if at == 0x270 && width == 4 {
            if let Some((ready, command)) = self.xtal_pending {
                if self.now >= ready { self.xtal_pending=None; self.complete_xtal(command); }
            }
        }
        if at == 0x1174c && width == 4 {
            if self.swsi_stuck { return Ok(1 << 24); }
            if self.swsi_busy != 0 { self.swsi_busy -= 1; return Ok(1 << 24); }
            for path in 0..2 {
                if let Some(ready)=self.rck_pending[path] {
                    if self.now>=ready && self.rck_stuck!=Some(path as u8) {
                        self.rf[path][0x1c]|=8;
                        self.rf[path][0x1b]=(self.rf[path][0x1b]&!0x7c00)|((self.rck_values[path] as u32)<<10);
                        self.rck_pending[path]=None;
                    }
                }
            }
            if let Some((ready,path,reg))=self.swsi_pending {
                if self.now<ready || self.read_stuck { return Ok(1<<25); }
                self.swsi_latched=(1<<26)|self.rf[path as usize][reg as usize]; self.swsi_pending=None;
            }
            return Ok(self.swsi_latched);
        }
        if self.dack_model && width==4 {
            for (reg,index,base) in [(0x1c05c,0x1c000,0x10),(0x1c080,0x1c020,0x30),(0x1c15c,0x1c100,0x50),(0x1c180,0x1c120,0x70)] {
                if at==reg {return Ok((self.get(at,4)&0xffffff)|((base+((self.get(index,4)>>1)&15))<<24));}
            }
        }
        if at == 0x18080 && self.nctl_stuck { return Ok(0); }
        Ok(self.get(at, width))
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> Result<()> {
        io::check_offset(at, width, self.memory.len())?;
        self.tick(1); self.writes += 1;
        self.events.push(Event::Write(at, width, value));
        self.put(at, width, value);
        if self.deadline_write == Some(at) { self.now += self.deadline_us; self.deadline_write = None; }
        if self.deadline_write_number == Some(self.writes) { self.now+=100_000; self.deadline_write_number=None; }
        if at == 0x10378 && width == 4 {
            assert!(self.swsi_pending.is_none(), "pending RF read cannot be replaced");
            self.swsi_pending=Some((self.now+2,((value>>8)&7) as u8,(value&255) as u8)); self.swsi_latched=0;
        }
        if at == 0x10370 && width == 4 {
            assert!(!self.swsi_stuck && self.swsi_pending.is_none(), "pending RF command cannot be replaced");
            let path=((value>>28)&7) as usize; let reg=((value>>20)&255) as usize;
            let mask=if value&(1<<31)!=0 { self.get(0x10374,4)&0xfffff } else { 0xfffff };
            self.rf[path][reg]=(self.rf[path][reg]&!mask)|(value&mask);
            if reg==0x1b && value&0xfffff==0x240 {
                self.rf[path][0x1c]&=!8; self.rck_pending[path]=Some(self.now+self.rck_delay);
                if self.rck_bus_stuck { self.swsi_stuck=true; }
            }
            if reg==5 && mask==0xfffff && self.restore_fail { return Err(Fault::new(FaultKind::Access,at,value)); }
            if path==1 && reg==5 && mask==0xfffff && self.late_restore { self.now+=100_000; self.late_restore=false; }
        }
        if self.fail_write == Some(self.writes) || self.fail_cleanup == Some(at) {
            return Err(Fault::new(FaultKind::Access, at, value));
        }
        if at == 0x270 && width == 4 && !self.xtal_stuck {
            assert!(self.xtal_pending.is_none(), "a pending XTAL command must not be overwritten");
            if self.xtal_delay_us == 0 { self.complete_xtal(value); }
            else { self.xtal_pending=Some((self.now+self.xtal_delay_us,value)); }
        }
        if at == 0x10370 { self.swsi_busy = 1; }
        Ok(())
    }
    fn reset(&self) {
        assert_eq!(self.get(2, 1) & 3, 0);
        assert_eq!(self.get(0x2f0, 4) & (1<<17), 0);
        assert_eq!(self.get(0x8040, 1), 14);
        assert_eq!(self.xtal[0x80]&7,0); assert_eq!(self.xtal[0x81]&7,0);
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
    fn now_us(&mut self) -> u64 {
        self.now_calls += 1;
        if self.jump_call == Some(self.now_calls) { self.now += 100_000; }
        self.now
    }
    fn delay_us(&mut self, us: u32) -> Result<()> {
        self.tick(us as u64);
        if self.delay_jump == Some(us) { self.tick(100_000); self.delay_jump=None; }
        Ok(())
    }
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
        (0xc004,4,0), (0xe000,4,1<<30), (0x200,4,3<<17), (0x2f0,4,0x8282), (0x8040,1,8), (2,1,0), (0x2f0,4,0), (0x8040,1,0),
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
            3=>d.memory.resize(0x1ffff,0), 4=>board.cut=0, 5=>board.rfe=2, _=>d.command=5 }
        assert!(Phy::new(&mut d, board).baseband().is_err()); assert!(d.events.is_empty());
    }
}

#[test]
fn bb_applies_full_mcu_offset_and_preserves_repeated_writes() {
    let mut d = Device::new();
    let mut p = Phy::new(&mut d, identity()); p.baseband().unwrap();
    assert_eq!(p.state(), State::BasebandLoaded);
    assert_eq!(d.writes, 1034);
    assert_eq!(d.events[0], Event::Write(0x10704,4,0x601e0100));
    assert_eq!(d.events[1012], Event::Write(0x100f8,4,0x20220408));
    let repeated: Vec<_> = d.events.iter().filter(|e| matches!(e, Event::Write(0x10000,4,_))).cloned().collect();
    assert_eq!(repeated, [Event::Write(0x10000,4,0xc580801e), Event::Write(0x10000,4,0x0580801f)]);
    assert!(d.events[..1013].iter().all(|e| matches!(e, Event::Write(at,4,_) if *at>=0x10000 && *at<0x20000)));
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
    assert_eq!(&d.events[..3], &[Event::Write(0x10370,4,0x500000),
        Event::Write(0x10370,4,0x30000), Event::Write(0x1e000,4,0x30000)]);
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
    assert!(d.events.iter().any(|e| matches!(e,Event::Write(0x270,4,v) if v&255==0x81)));
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

#[test]
fn active_grant_may_also_decode_io_while_phy_uses_only_mmio() {
    let mut d=Device::new(); d.command=7;
    let mut p=Phy::new(&mut d,identity()); p.baseband().unwrap(); p.radio().unwrap(); p.nctl().unwrap();
    assert_eq!(p.state(),State::TablesLoaded); assert_eq!(d.command,7);
    assert!(d.events.iter().all(|event| match event { Event::Write(at,_,_) => *at>=0x10000 || matches!(*at,0xd248|0xd220|0xd240|0xd288|0xd28c|0xd290), Event::H2c(_,_,_) => true }));
}

#[test]
fn clock_gap_before_inner_budget_does_not_extend_absolute_stage_success() {
    let mut d=Device::new(); d.jump_call=Some(4);
    let error=Phy::new(&mut d,identity()).baseband().unwrap_err();
    assert_eq!(error.cause.kind,FaultKind::Timeout); d.reset();
}

#[test]
fn chip_specific_disable_preserves_xtal_other_bits_and_clears_both_rf_enables() {
    let mut d=Device::new(); d.xtal[0x80]=0xd7; d.xtal[0x81]=0xf7; d.fail_write=Some(20);
    let error=Phy::new(&mut d,identity()).baseband().unwrap_err(); assert!(error.disable_bb_rf.is_none());
    assert_eq!(d.xtal[0x80],0xd0); assert_eq!(d.xtal[0x81],0xf0); d.reset();
    let cleanup=&d.events[20..];
    assert!(matches!(cleanup[0],Event::Write(0x2f0,4,0)));
    assert!(matches!(cleanup[1],Event::Write(2,1,0)));
}

#[test]
fn cleanup_does_not_overwrite_an_already_pending_xtal_command() {
    let mut d=Device::new(); d.fail_write=Some(20); d.put(0x270,4,1<<31);
    let error=Phy::new(&mut d,identity()).baseband().unwrap_err();
    assert_eq!(error.disable_bb_rf.unwrap().kind,FaultKind::Timeout);
    assert!(!d.events.iter().any(|event| matches!(event,Event::Write(0x270,_,_))));
    assert_eq!(d.get(2,1)&3,0); assert_eq!(d.get(0x2f0,4)&(1<<17),0);
}

#[test]
fn cleanup_xtal_read_timeout_reports_failure_without_overwriting_for_the_other_path() {
    let mut d=Device::new(); d.fail_write=Some(20); d.xtal_stuck=true;
    let error=Phy::new(&mut d,identity()).baseband().unwrap_err();
    assert_eq!(error.disable_bb_rf.unwrap().kind,FaultKind::Timeout);
    let commands:Vec<_>=d.events.iter().filter(|e| matches!(e,Event::Write(0x270,4,_))).collect();
    assert_eq!(commands.len(),1); assert_eq!(d.xtal[0x80],0xc7); assert_eq!(d.xtal[0x81],0xc7);
}

#[test]
fn chip_cleanup_accepts_slow_xtal_commands_within_source_derived_total_budget() {
    let mut d=Device::new(); d.fail_write=Some(20); d.xtal_delay_us=10_000;
    let start=d.now; let error=Phy::new(&mut d,identity()).baseband().unwrap_err();
    assert_eq!(error.cause.kind,FaultKind::Access); assert!(error.disable_bb_rf.is_none());
    assert!(d.now-start>=60_000 && d.now-start<400_000); assert!(d.xtal_pending.is_none()); d.reset();
}

#[test]
fn txpower_and_chip_reset_follow_bb_in_exact_order_preserving_other_fields() {
    let mut d=Device::new();
    d.put(0xd288,4,0x12345678); d.put(0xd28c,4,0x87654335); d.put(0xd290,4,0x76543216);
    d.put(0x158dc,4,0x80001234); d.put(0x178dc,4,0x80005678);
    let gain=Phy::new(&mut d,identity()).baseband().unwrap(); assert_eq!(gain,phy::gain_defaults());
    assert_eq!(&d.events[1013..], &[
        Event::Write(0xd248,4,0x07763333), Event::Write(0xd220,4,0x01ebf000), Event::Write(0xd240,4,0x0002f8ff),
        Event::Write(0xd288,4,0x92345678), Event::Write(0xd28c,4,0x87654320), Event::Write(0xd290,4,0x7654321d),
        Event::Write(0x158dc,4,0xc0001234), Event::Write(0x15818,4,0x482c1800),
        Event::Write(0x178dc,4,0xc0005678), Event::Write(0x17818,4,0x482c1800),
        Event::Write(0x11200,4,0x70010142), Event::Write(0x13200,4,0x70010142),
        Event::Write(0x10704,4,0x601c05ff), Event::Write(0x10704,4,0x601c05fd),
        Event::Write(0x11200,4,0x00010142), Event::Write(0x13200,4,0x00010142),
        Event::Write(0x10704,4,0x601c05ff), Event::Write(0x158dc,4,0x80001234),
        Event::Write(0x15818,4,0x082c1800), Event::Write(0x178dc,4,0x80005678), Event::Write(0x17818,4,0x082c1800),
    ]);
    assert_eq!(d.get(0xc348,4),0); assert_eq!(d.get(0x1010,4),0xb0f03); assert_eq!(d.command,6);
}

#[test]
fn every_post_table_write_failure_poisoned_without_publishing_gains_or_radio() {
    for fail in 1014..=1034 {
        let mut d=Device::new(); d.fail_write=Some(fail);
        let mut p=Phy::new(&mut d,identity()); let error=p.baseband().unwrap_err();
        assert_eq!(error.cause.kind,FaultKind::Access);
        assert_eq!(error.stage,if fail<=1019 { Stage::TxPowerUnit } else { Stage::BasebandReset });
        assert!(error.disable_bb_rf.is_none()); assert_eq!(p.state(),State::Failed);
        assert_eq!(p.bus_between_stages().events[fail],Event::Write(0x2f0,4,0));
        let before=p.bus_between_stages().writes; assert!(p.radio().is_err()); assert!(p.baseband().is_err());
        assert_eq!(p.bus_between_stages().writes,before); p.bus_between_stages().reset();
    }
}

#[test]
fn last_chip_reset_write_cannot_report_success_after_whole_bb_deadline() {
    let mut d=Device::new(); d.deadline_write_number=Some(1034);
    let mut p=Phy::new(&mut d,identity()); let error=p.baseband().unwrap_err();
    assert_eq!(error.stage,Stage::BasebandReset); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(p.state(),State::Failed); assert!(error.disable_bb_rf.is_none()); d.reset();
}

#[test]
fn bb_reset_delay_overrun_blocks_later_reset_writes_and_radio() {
    let mut d=Device::new(); d.delay_jump=Some(1);
    let mut p=Phy::new(&mut d,identity()); let error=p.baseband().unwrap_err();
    assert_eq!(error.stage,Stage::BasebandReset); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert!(error.disable_bb_rf.is_none()); assert!(p.radio().is_err());
    assert_eq!(d.events[1025],Event::Write(0x2f0,4,0)); d.reset();
}


fn to_nctl(p: &mut Phy<Device>) { p.baseband().unwrap(); p.radio().unwrap(); p.nctl().unwrap(); }

#[test]
fn bounded_rf_access_validates_arguments_before_any_register_write() {
    for (path,at,mask,value) in [(2,5,1,0),(0,0x20000,1,0),(0,5,0,0),(0,5,0x100000,0),(0,5,1,2),(0,5,5,2)] {
        let mut d=Device::new(); assert!(phy::rf_write_masked(&mut d,path,at,mask,value).is_err());
        assert!(d.events.is_empty());
        if mask==0 || mask>0xfffff || path>1 || at>0x100ff { assert!(phy::rf_read(&mut d,path,at,mask).is_err()); }
        assert!(d.events.is_empty());
    }
}

#[test]
fn rf_read_and_masked_write_preserve_analog_and_direct_register_fields() {
    let mut d=Device::new(); d.rf[1][0x55]=0xabcde; d.put(0x10374,4,0xabc00000); d.put(0x10378,4,0xfeed0000);
    phy::rf_write_masked(&mut d,1,0x55,0x00f00,3).unwrap();
    assert_eq!(d.rf[1][0x55],0xab3de); assert_eq!(d.get(0x10374,4),0xabc00f00);
    assert_eq!(d.events[1],Event::Write(0x10370,4,0x95500300));
    assert_eq!(phy::rf_read(&mut d,1,0x55,0x00f00).unwrap(),3); assert_eq!(d.get(0x10378,4),0xfeed0155);
    d.put(0x1e154,4,0xabcabcde); phy::rf_write_masked(&mut d,0,0x10055,0x00f00,7).unwrap();
    assert_eq!(d.get(0x1e154,4),0xabcab7de); assert_eq!(phy::rf_read(&mut d,0,0x10055,0x00f00).unwrap(),7);
}

#[test]
fn frozen_swsi_read_has_finite_limit_and_never_replaces_pending_request() {
    let mut d=Device::new(); d.frozen=true; d.read_stuck=true;
    let error=phy::rf_read(&mut d,0,5,0xfffff).unwrap_err(); assert_eq!(error.kind,FaultKind::Timeout);
    assert!(d.reads<40); let commands=d.events.len();
    assert!(phy::rf_read(&mut d,1,5,0xfffff).is_err()); assert_eq!(d.events.len(),commands);
}

#[test]
fn rck_requires_nctl_and_rechecks_live_mac_without_writes_on_refusal() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    assert!(p.rck().is_err()); assert!(p.bus_between_stages().events.is_empty());
    to_nctl(&mut p); let d=p.bus_between_stages(); d.events.clear(); d.put(0xc348,4,1);
    assert!(p.rck().is_err()); assert_eq!(p.state(),State::Failed); assert!(p.bus_between_stages().events.is_empty());
}

#[test]
fn rck_both_paths_follow_backoff_read_result_and_restore_reserved_registers() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    let d=p.bus_between_stages(); d.events.clear(); d.rf[0][5]=0xabcdf; d.rf[1][5]=0x54321;
    d.put(0x144a0,4,(20<<12)|24); d.put(0x181bc,4,0x99000000); d.put(0x182bc,4,0xaa000000);
    let result=p.rck().unwrap(); assert_eq!(result,phy::RckResult { values:[7,11], dpk_gain:0x7f });
    assert_eq!(p.state(),State::RckDone); let d=p.bus_between_stages();
    assert_eq!(d.events[0],Event::Write(0x181bc,4,0x997f7f7f));
    assert_eq!(d.events[1],Event::Write(0x182bc,4,0xaa7f7f7f));
    assert_eq!(d.rf[0][5],0xabcdf); assert_eq!(d.rf[1][5],0x54321);
    assert_eq!(d.rf[0][0x1b],7); assert_eq!(d.rf[1][0x1b],11);
    let writes:Vec<u32>=d.events.iter().filter_map(|e| if let Event::Write(0x10370,4,v)=e {Some(*v)} else {None}).collect();
    assert_eq!(writes,[0x80500000,0x80030000,0x01b00240,0x01b00007,0x005abcdf,
        0x90500000,0x90030000,0x11b00240,0x11b0000b,0x10554321]);
    assert_eq!(d.get(0xc348,4),0); assert_eq!(d.command,6);
}

#[test]
fn low_backoff_selects_software_gain_without_writing_dpd_coefficients() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    let d=p.bus_between_stages(); d.put(0x144a0,4,(20<<12)|23); d.events.clear();
    assert_eq!(p.rck().unwrap().dpk_gain,0x5b);
    assert!(!p.bus_between_stages().events.iter().any(|e| matches!(e,Event::Write(0x181bc|0x182bc,_,_))));
}

#[test]
fn rck_timeout_restores_rf5_disables_chip_and_does_not_start_later_path() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    let d=p.bus_between_stages(); d.rck_stuck=Some(0); d.rf[0][5]=0x12345; d.events.clear();
    let error=p.rck().unwrap_err(); assert_eq!(error.stage,Stage::RckA); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert!(error.restore_rf.is_none()); assert!(error.disable_bb_rf.is_none()); assert_eq!(p.state(),State::Failed);
    let d=p.bus_between_stages(); assert_eq!(d.rf[0][5],0x12345);
    assert!(!d.events.iter().any(|e| matches!(e,Event::Write(0x10370,4,v) if (v>>28)&7==1))); d.reset();
}

#[test]
fn rck_primary_timeout_and_independent_restore_failure_are_both_reported() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    let d=p.bus_between_stages(); d.rck_stuck=Some(0); d.restore_fail=true;
    let error=p.rck().unwrap_err(); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.restore_rf.unwrap().kind,FaultKind::Access); assert!(error.disable_bb_rf.is_none());
    p.bus_between_stages().reset();
}

#[test]
fn rck_cleanup_refuses_to_replace_a_stuck_swsi_command() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    let d=p.bus_between_stages(); d.rck_bus_stuck=true; d.events.clear();
    let error=p.rck().unwrap_err(); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.restore_rf.unwrap().kind,FaultKind::Timeout); assert!(error.disable_bb_rf.is_none());
    let commands:Vec<_>=p.bus_between_stages().events.iter().filter(|e| matches!(e,Event::Write(0x10370,4,_))).collect();
    assert_eq!(commands.len(),3); p.bus_between_stages().reset();
}

#[test]
fn late_last_rck_restore_cannot_report_completed_calibration() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); to_nctl(&mut p);
    p.bus_between_stages().late_restore=true;
    let error=p.rck().unwrap_err(); assert_eq!(error.stage,Stage::RckB); assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert!(error.restore_rf.is_none()); assert!(error.disable_bb_rf.is_none()); assert_eq!(p.state(),State::Failed);
    p.bus_between_stages().reset();
}

impl dack::DackIo for Device {
    fn btc_rfk(&mut self,notice:dack::BtcNotice,timeout_us:u32)->Result<()> {
        assert!(timeout_us>0 && timeout_us<=100_000); self.btc.push((notice,timeout_us)); self.btc_event_positions.push(self.events.len());
        let (delay,fail)=match notice.state {
            dack::RfkState::Start=>(self.btc_start_delay,self.btc_start_fail),
            dack::RfkState::Stop=>(self.btc_stop_delay,self.btc_stop_fail),
        };
        self.tick(delay);
        if fail {Err(Fault::new(FaultKind::Access,0xaa,notice.state as u32))} else {Ok(())}
    }
}
#[path="rtw89_dack_cases.rs"] mod dack_cases;
