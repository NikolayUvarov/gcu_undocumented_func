use super::*;
use crate::board::Identity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event { Write(usize, usize, u32), Rf(u8, u8, u32), Mirror(u8) }

struct Device {
    bytes: Vec<u8>, rf: [[u32; 256]; 2], xtal: [u8; 256], events: Vec<Event>, now: u64,
    rf_read: Option<(u8, u8)>, lte_pending: Option<u32>, lte_value: u32,
    lte_stuck: Option<usize>, lte_transactions: usize, unavailable: Option<usize>,
    writes: usize, fail_write: Option<usize>, late_at: Option<usize>, frozen: bool,
    cleanup_fail_path: Option<u8>, swsi_stuck: bool, backward_at: Option<usize>,
}

fn board() -> Board {
    Board { identity: Identity { cut: 1, mac: [2, 3, 4, 5, 6, 7], rfe: 1, xtal: 32, country: *b"US", channel_plan: 127 },
        maps: crate::efuse::Maps { logical: [0xff; crate::efuse::LOGICAL_SIZE], dav_logical: [0xff; crate::efuse::DAV_LOGICAL_SIZE], phycap: [0xff; crate::efuse::PHYCAP_SIZE] } }
}

impl Device {
    fn new() -> Self {
        let mut d = Self { bytes: vec![0; 0x20000], rf: [[0; 256]; 2], xtal: [0xc7; 256], events: Vec::new(), now: 1_000,
            rf_read: None, lte_pending: None, lte_value: 0xa5a5_0100, lte_stuck: None, lte_transactions: 0,
            unavailable: None, writes: 0, fail_write: None, late_at: None, frozen: false, cleanup_fail_path: None,
            swsi_stuck: false, backward_at: None };
        for (at, value) in [(0xf0, 0x1000), (0x3f0, 0x100), (0x88, 3), (0x1e0, 0xe0), (0x8400, 0xfffd0000),
            (0x8404, 0x0f9f0000), (0xc000, 0xf000003f), (0xc004, 0x4000003f), (0x200, 1 << 17),
            (0x2f0, 1 << 17), (0x1000, 0x800), (0x1010, 0xb0f03), (0xda30, 0x80), (0xda10, 0x80)] { d.put(at, 4, value); }
        d.put(2, 1, 3); d.put(0x8040, 1, 14); d.put(0x40, 1, 0xc8); d.put(0x41, 1, 0x85);
        d.put(0xc340, 2, 0x0201); d.put(0xcc07, 1, 0x82); d.put(0xda6c, 1, 0xc0); d
    }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.bytes[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn get(&self, at: usize, width: usize) -> u32 { let mut b = [0; 4]; b[..width].copy_from_slice(&self.bytes[at..at + width]); u32::from_le_bytes(b) }
    fn tick(&mut self, us: u64) { if !self.frozen { self.now += us; } }
    fn read(&mut self, at: usize, width: usize) -> io::Result<u32> {
        io::check_offset(at, width, self.bytes.len())?; self.tick(1);
        if self.unavailable == Some(at) { return Ok(u32::MAX); }
        if at == LTE_CTRL + 3 {
            if self.lte_stuck == Some(self.lte_transactions) { return Ok(0); }
            if let Some(command) = self.lte_pending.take() { if command >> 30 == 3 { self.lte_value = self.get(LTE_WDATA, 4); } }
            return Ok(0x20);
        }
        if at == LTE_RDATA { assert_eq!(self.lte_pending, Some(0x800f003c)); return Ok(self.lte_value); }
        if at == 0x1174c {
            if self.swsi_stuck { return Ok(1 << 24); }
            return Ok(match self.rf_read { Some((path, reg)) => 1 << 26 | self.rf[path as usize][reg as usize], None => 0 });
        }
        Ok(self.get(at, width))
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> io::Result<()> {
        io::check_offset(at, width, self.bytes.len())?; self.tick(1); self.writes += 1;
        self.events.push(Event::Write(at, width, value)); self.put(at, width, value);
        if at == LTE_CTRL { assert!(self.lte_pending.is_none()); self.lte_pending = Some(value); self.lte_transactions += 1; }
        if at == 0x10370 {
            assert!(!self.swsi_stuck);
            let path = (value >> 28 & 3) as u8; let reg = (value >> 20 & 255) as u8;
            let mask = if value >> 31 != 0 { self.get(0x10374, 4) & RF_MASK } else { RF_MASK };
            self.rf[path as usize][reg as usize] = (self.rf[path as usize][reg as usize] & !mask) | (value & mask);
            self.events.push(Event::Rf(path, reg, value & RF_MASK)); self.rf_read = None;
            if reg == 0xef && value & RF_MASK == 0 && self.cleanup_fail_path == Some(path) {
                return Err(Fault::new(FaultKind::Access, at, value));
            }
        }
        if at == 0x10378 { self.rf_read = Some(((value >> 8 & 7) as u8, value as u8)); }
        if at == 0x270 {
            let offset = value as u8 as usize;
            if value >> 24 & 7 == 1 { self.put(at, 4, offset as u32 | (self.xtal[offset] as u32) << 8); }
            else { let mask = (value >> 16) as u8; self.xtal[offset] = (self.xtal[offset] & !mask) | ((value >> 8) as u8 & mask); self.put(at, 4, value & !(1 << 31)); }
        }
        if self.late_at == Some(at) { self.now += STAGE_US; self.late_at = None; }
        if self.backward_at == Some(at) { self.now = 0; self.backward_at = None; }
        if self.fail_write == Some(self.writes) { return Err(Fault::new(FaultKind::Access, at, value)); }
        Ok(())
    }
    fn quiet(&self) {
        assert_eq!(self.get(0x1010, 4), 0xb0f03); assert_eq!(self.get(0x1000, 4), 0x800);
        assert_eq!(self.get(0xc348, 4), 0); assert_eq!(self.get(0xac, 4), 0);
        assert!(!self.events.iter().any(|e| matches!(e, Event::Write(0x18000, _, _))));
    }
    fn disabled(&self) { assert_eq!(self.get(2, 1) & 3, 0); assert_eq!(self.get(0x2f0, 4) & 1 << 17, 0); }
}

impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> io::Result<u8> { self.read(at, 1).map(|v| v as u8) }
    fn read16(&mut self, at: usize) -> io::Result<u16> { self.read(at, 2).map(|v| v as u16) }
    fn read32(&mut self, at: usize) -> io::Result<u32> { self.read(at, 4) }
    fn write8(&mut self, at: usize, v: u8) -> io::Result<()> { self.write(at, 1, v as u32) }
    fn write16(&mut self, at: usize, v: u16) -> io::Result<()> { self.write(at, 2, v as u32) }
    fn write32(&mut self, at: usize, v: u32) -> io::Result<()> { self.write(at, 4, v) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { self.tick(us as u64); Ok(()) }
}
impl PhyIo for Device {
    fn pci_id(&mut self) -> io::Result<u32> { Ok(0xb85210ec) }
    fn pci_command(&mut self) -> io::Result<u16> { Ok(6) }
    fn bar_len(&self) -> usize { self.bytes.len() }
    fn send_rf_page(&mut self, path: u8, _: u8, _: &[u8], _: u32) -> io::Result<()> { self.events.push(Event::Mirror(path)); Ok(()) }
}
fn loaded(d: &mut Device) -> Phy<'_, Device> {
    let mut phy = Phy::new(d, board().identity); phy.baseband().unwrap(); phy.radio().unwrap();
    let bus = phy.bus_between_stages(); bus.events.clear(); bus.writes = 0; phy
}
fn run(phy: &mut Phy<'_, Device>) -> core::result::Result<ConfigPrepared, Failure> { prepare_inner(phy, &board(), [0, 29, 29, 15]) }

#[test]
fn radio_loaded_hardware_init_matches_rfe1_without_scoreboard_policy_or_nctl() {
    let mut d = Device::new(); let mut p = loaded(&mut d); run(&mut p).unwrap(); assert_eq!(p.state(), State::RadioLoaded);
    let d = p.bus_between_stages();
    assert_eq!(d.lte_value, 0x100); assert_eq!(d.lte_transactions, 2); assert!(d.lte_pending.is_none());
    for (at, width, value) in [(0x40, 1, 0x28), (0x41, 1, 0x83), (0xda6c, 1, 0xc5), (0xc340, 2, 0x21),
        (0xcc07, 1, 0x80), (0xda30, 4, 0x88), (0xda10, 4, 0x180), (0xda2c, 4, 0xf0ffffff)] { assert_eq!(d.get(at, width), value, "{at:x}"); }
    let actual: Vec<_> = d.events.iter().filter_map(|e| if let Event::Rf(p,r,v) = e { Some((*p,*r,*v)) } else { None }).collect();
    let mut expected = vec![(0,2,0), (1,2,0)];
    for (path, group, value) in [(0,0,0x5ff), (1,0,0x5ff), (0,2,0x5ff), (1,2,0x55f)] { expected.extend([(path,0xef,0x20000),(path,0x33,group),(path,0x3f,value),(path,0xef,0)]); }
    assert_eq!(actual, expected); assert_eq!(d.get(0xda40, 4) & 0x1000c, 0x1000c); d.quiet();
}

#[test]
fn fresh_or_baseband_only_phase_never_writes_btc() {
    for baseband in [false, true] {
        let mut d = Device::new(); let mut p = Phy::new(&mut d, board().identity);
        if baseband { p.baseband().unwrap(); } p.bus_between_stages().events.clear();
        assert_eq!(run(&mut p).unwrap_err().stage, Stage::Guard); assert!(p.bus_between_stages().events.is_empty());
    }
}

#[test]
fn unsupported_board_security_or_firmware_refuses_before_writes() {
    for case in 0..4 {
        let mut d = Device::new(); let mut p = loaded(&mut d); let mut board = board(); let mut version = [0,29,29,15];
        match case { 0 => board.identity.cut = 2, 1 => board.identity.rfe = 2, 2 => board.maps.phycap[0x6c] = 0, _ => version[3] = 5 }
        assert_eq!(prepare_inner(&mut p, &board, version).unwrap_err().stage, Stage::Guard);
        assert_eq!(p.state(), State::Failed); assert!(p.bus_between_stages().events.is_empty());
    }
}

#[test]
fn active_rx_or_scheduler_guard_cannot_be_bypassed() {
    for (at, value) in [(0x1000,0x2800),(0x1010,0xb0f00),(0xc348,1),(0x8040,15),(0x1e0,0xc0)] {
        let mut d = Device::new(); let mut p = loaded(&mut d); p.bus_between_stages().put(at,4,value);
        assert_eq!(run(&mut p).unwrap_err().stage,Stage::Guard); assert!(p.bus_between_stages().events.is_empty());
    }
}

#[test]
fn lte_stall_before_each_transaction_never_replaces_pending_command() {
    for transaction in 0..3 {
        let mut d = Device::new(); let mut p = loaded(&mut d); p.bus_between_stages().lte_stuck = Some(transaction);
        let before = p.bus_between_stages().now; let failure = run(&mut p).unwrap_err();
        assert_eq!(failure.stage,Stage::Pta); assert_eq!(failure.cause.kind,FaultKind::Timeout);
        assert_eq!(p.state(),State::Failed); let d=p.bus_between_stages(); assert_eq!(d.lte_transactions,transaction);
        assert!(d.now-before < 60_000); assert!(failure.close_lut.iter().all(Option::is_none)); assert_eq!(failure.disable_bb_rf,None); d.disabled(); d.quiet();
    }
}

#[test]
fn unavailable_lte_and_frozen_clock_fail_in_finite_reads() {
    for frozen in [false,true] {
        let mut d=Device::new(); let mut p=loaded(&mut d);
        if frozen {p.bus_between_stages().frozen=true; p.bus_between_stages().lte_stuck=Some(0);} else {p.bus_between_stages().unavailable=Some(LTE_CTRL+3);}
        let f=run(&mut p).unwrap_err(); assert_eq!(f.cause.kind,if frozen {FaultKind::Timeout} else {FaultKind::Unavailable});
        assert_eq!(p.bus_between_stages().lte_transactions,0); assert_eq!(p.state(),State::Failed);
    }
}

#[test]
fn every_modified_write_failure_attempts_both_lut_closes_and_disables_radio() {
    let mut d=Device::new(); let mut p=loaded(&mut d); run(&mut p).unwrap(); let total=p.bus_between_stages().writes;
    for failed in 1..=total {
        let mut d=Device::new(); let mut p=loaded(&mut d); p.bus_between_stages().fail_write=Some(failed);
        let f=run(&mut p).unwrap_err(); assert_eq!(f.cause.kind,FaultKind::Access,"write {failed}"); assert_eq!(p.state(),State::Failed);
        let d=p.bus_between_stages(); assert_eq!(d.rf[0][0xef],0); assert_eq!(d.rf[1][0xef],0); d.disabled(); d.quiet();
        assert_eq!(f.close_lut,[None,None],"write {failed}"); assert_eq!(f.disable_bb_rf,None,"write {failed}");
        let writes=d.writes; assert!(run(&mut p).is_err()); assert_eq!(p.bus_between_stages().writes,writes);
    }
}

#[test]
fn cleanup_error_is_separate_and_other_path_is_attempted() {
    let mut d=Device::new(); let mut p=loaded(&mut d); let d=p.bus_between_stages(); d.fail_write=Some(1); d.cleanup_fail_path=Some(0);
    let f=run(&mut p).unwrap_err(); assert_eq!(f.cause.kind,FaultKind::Access); assert!(f.close_lut[0].is_some()); assert_eq!(f.close_lut[1],None);
    let d=p.bus_between_stages(); assert!(d.events.contains(&Event::Rf(1,0xef,0))); d.disabled();
}

#[test]
fn late_final_write_and_clock_regression_never_publish_prepared() {
    for backward in [false,true] {
        let mut d=Device::new(); let mut p=loaded(&mut d);
        if backward {p.bus_between_stages().backward_at=Some(0xda2c);} else {p.bus_between_stages().late_at=Some(0xda2c);}
        let f=run(&mut p).unwrap_err(); assert_eq!(f.cause.kind,if backward {FaultKind::Clock} else {FaultKind::Timeout});
        assert_eq!(p.state(),State::Failed); p.bus_between_stages().disabled();
    }
}
