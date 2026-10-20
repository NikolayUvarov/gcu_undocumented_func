#[path = "../rtw89_wifi/src/io.rs"]
#[allow(dead_code)] mod io;
#[path = "../rtw89_wifi/src/registers.rs"]
#[allow(dead_code)] mod registers;
#[path = "../rtw89_wifi/src/fwcmd.rs"]
#[allow(dead_code)] mod fwcmd;
#[path = "../rtw89_wifi/src/mac.rs"]
#[allow(dead_code)] mod mac;
#[path = "../rtw89_wifi/src/pcie.rs"]
#[allow(dead_code)] mod pcie;

use io::{Fault, FaultKind, RegisterIo, Result};
use registers::*;
use std::alloc::{alloc_zeroed, dealloc, Layout};

const INDICES: [usize; 9] = [0x1050, 0x1054, 0x1058, 0x105c, 0x1060, 0x1064, 0x1078, 0x107c, 0x1080];
struct Device {
    registers: Vec<u8>, config: [u8; 4096], mdio: [u16; 128], command: u16, now: u64,
    writes: Vec<(usize, usize, u32)>, dbi_writes: Vec<(usize, u8)>, mdio_writes: Vec<(usize, u16)>,
    fail_write: Option<usize>, ignore: Option<usize>, stuck_mdio: Option<usize>, frozen: bool,
    restore_fails: bool, dbi_delay: usize, pending: Option<(usize, bool, u8, usize)>,
    stuck_index: Option<usize>, stuck_reset: bool, late_final_read: bool, polls: usize,
}
impl Device {
    fn new() -> Self {
        let mut d = Self { registers: vec![0; 0xc004], config: [0; 4096], mdio: [0; 128], command: 2, now: 0,
            writes: Vec::new(), dbi_writes: Vec::new(), mdio_writes: Vec::new(), fail_write: None,
            ignore: None, stuck_mdio: None, frozen: false, restore_fails: false, dbi_delay: 0,
            pending: None, stuck_index: None, stuck_reset: false, late_final_read: false, polls: 0 };
        for (at, value) in [(SYS_CFG1, 0x1000), (R_AX_IC_PWR_STATE, 0x100),
            (R_AX_DMAC_FUNC_EN, 0x64c4_0000), (mac::DMAC_CLOCK, 0x0484_0000),
            (mac::PLE_CONFIG, 0x0040_0801), (0x8c44, 48), (mac::HFC_CONTROL, 8),
            (mac::CHANNEL_PAGE, 40 << 16), (mac::WDE_READY, 3), (mac::PLE_READY, 3),
            (fwcmd::INIT_CONFIG, 0x30)] { d.put(at, 4, value); }
        for at in INDICES { d.put(at, 4, 0x0001_0001); }
        d.config[0x82] = 2; d.config[0x719] = 0x38;
        d.mdio[0x1b] = 0xa5a5; d.mdio[0x1d] = 0xa500;
        d.mdio[0x30] = 0xa234; d.mdio[0x70] = 0xa234; d
    }
    fn get(&self, at: usize, width: usize) -> u32 {
        let mut value = [0; 4]; value[..width].copy_from_slice(&self.registers[at..at + width]); u32::from_le_bytes(value)
    }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.registers[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn complete_dbi(&mut self) {
        let Some((address, write, value, left)) = self.pending else { return; };
        if left != 0 { self.pending = Some((address, write, value, left - 1)); return; }
        if write {
            self.dbi_writes.push((address, value));
            if !(self.restore_fails && address == 0x719 && value == 0x38) { self.config[address] = value; }
        } else {
            let aligned = address & !3;
            for lane in 0..4 { self.registers[0x1098 + lane] = self.config[aligned + lane]; }
        }
        self.pending = None; self.registers[0x1092] = 0;
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> Result<()> {
        assert!(!(at == 0x1095 && self.pending.is_some()), "DBI write data replaced before prior command completed");
        let fails = self.fail_write == Some(self.writes.len());
        self.writes.push((at, width, value));
        if self.ignore != Some(at) {
            self.put(at, width, value);
            if at == 0x10a0 && width == 2 && value & 0x300 != 0 {
                let index = ((value >> 12) & 3) as usize * 32 + (value & 31) as usize;
                if self.stuck_mdio != Some(index) {
                    if value & 0x100 != 0 {
                        let data = self.get(0x10a4, 2) as u16; self.mdio[index] = data; self.mdio_writes.push((index, data));
                    } else { self.put(0x10a6, 2, self.mdio[index] as u32); }
                    self.put(at, 2, value & !0x300);
                }
            }
            if at == 0x1092 && width == 1 {
                let command = self.get(0x1090, 2) as usize;
                let write = value == 1;
                let lane = if write { ((command >> 12) & 15).trailing_zeros() as usize } else { 0 };
                assert!(lane < 4); assert!(value == 1 || value == 2);
                let address = (command & 0xffc) + lane;
                let data = self.registers[0x1094 + lane];
                self.pending = Some((address, write, data, self.dbi_delay));
                if self.dbi_delay == 0 { self.complete_dbi(); }
            }
            if at == fwcmd::CLEAR_INDEX {
                for (bit, index) in [(0, 0x1058), (1, 0x105c), (2, 0x1060), (3, 0x1064), (8, 0x1078), (9, 0x107c), (10, 0x1080)] {
                    if value & (1 << bit) != 0 && self.stuck_index != Some(index) { self.put(index, 4, 0); }
                }
                self.put(at, 4, 0);
            }
            if at == 0x1018 {
                for index in [0x1050, 0x1054] { if self.stuck_index != Some(index) { self.put(index, 4, 0); } }
                self.put(at, 4, 0);
            }
            if at == fwcmd::INIT_CONFIG && value & 8 != 0 && !self.stuck_reset { self.put(at, 4, value & !8); }
            if at == 0x11d8 { self.put(at, 4, value & !2); }
        }
        if fails { return Err(Fault::new(FaultKind::Access, at, value)); }
        Ok(())
    }
    fn stopped(&self) {
        assert_eq!(self.command, 2); assert_eq!(self.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
        assert_eq!(self.get(fwcmd::DMA_STOP, 4) & 0x001f_0f03, 0x001f_0f03);
        assert!(self.writes.iter().all(|(at, _, value)| *at != fwcmd::INIT_CONFIG || value & 0x2800 == 0));
        assert!(self.writes.iter().all(|(at, _, _)| ![R_AX_PLATFORM_ENABLE, R_AX_SYS_FUNC_EN, R_AX_WLRF_CTRL, R_AX_CMAC_FUNC_EN].contains(at)));
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> Result<u8> {
        if at == 0x1092 { self.complete_dbi(); self.polls += 1; }
        Ok(self.get(at, 1) as u8)
    }
    fn read16(&mut self, at: usize) -> Result<u16> { if at == 0x10a0 { self.polls += 1; } Ok(self.get(at, 2) as u16) }
    fn read32(&mut self, at: usize) -> Result<u32> {
        if at == 0x9a00 && self.late_final_read && self.writes.iter().any(|(r, _, _)| *r == at) {
            self.now += 50_000; self.late_final_read = false;
        }
        Ok(self.get(at, 4))
    }
    fn write8(&mut self, at: usize, value: u8) -> Result<()> { self.write(at, 1, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> Result<()> { self.write(at, 2, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> { self.write(at, 4, value) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> Result<()> { if !self.frozen { self.now += us as u64; } Ok(()) }
}
impl fwcmd::Bus for Device { fn pci_command(&mut self) -> Result<u16> { Ok(self.command) } }

struct Region { ptr: *mut u8, layout: Layout }
impl Region {
    fn new() -> Self {
        let layout = Layout::from_size_align(fwcmd::DMA_BYTES, 4096).unwrap();
        let ptr = unsafe { alloc_zeroed(layout) }; assert!(!ptr.is_null()); Self { ptr, layout }
    }
    fn queue<'a>(&'a self, bus: &mut Device) -> fwcmd::Queue<'a> {
        unsafe { fwcmd::Queue::new(bus, self.ptr, fwcmd::DMA_BYTES, 0x200000) }.unwrap()
    }
}
impl Drop for Region { fn drop(&mut self) { unsafe { dealloc(self.ptr, self.layout); } } }

#[test]
fn supported_gen1_and_gen2_prepare_the_pinned_modes_without_enabling_dma() {
    for rate in [1, 2] {
        let mut bus = Device::new(); bus.config[0x82] = rate;
        pcie::prepare(&mut bus).unwrap();
        assert_eq!(bus.mdio[0x1b], 0x15a5); assert_eq!(bus.mdio[0x1d], 0xa50c);
        let index = if rate == 1 { 0x30 } else { 0x70 };
        assert_eq!(bus.mdio[index], 0x8234);
        assert_eq!(bus.dbi_writes, [(0x719, 0x30), (0x719, 0x38)]);
        assert_eq!(bus.config[0x719], 0x38);
        assert_eq!(bus.get(fwcmd::INIT_CONFIG, 4), 0x00c0_d730);
        assert_eq!(bus.get(0x1004, 4), 0x0101_0000);
        assert_eq!(bus.get(0x13f0, 4), 0x0007_0001);
        assert_eq!(bus.get(0x11d8, 4), 0x81);
        assert_eq!(bus.get(0x8810, 4) & 1, 1); assert_eq!(bus.get(0x9a00, 4) & 2, 0);
        for at in INDICES { assert_eq!(bus.get(at, 4), 0); }
        bus.stopped();
    }
}

#[test]
fn already_disabled_l1_and_calibration_are_not_rewritten() {
    let mut bus = Device::new(); bus.config[0x719] = 0x30; bus.mdio[0x70] = 0x8234;
    pcie::prepare(&mut bus).unwrap();
    assert!(bus.dbi_writes.is_empty()); assert!(!bus.mdio_writes.iter().any(|(at, _)| *at == 0x70));
    bus.stopped();
}

#[test]
fn invalid_or_busy_preconditions_cause_no_writes_or_cleanup() {
    for case in 0..7 {
        let mut bus = Device::new();
        match case { 0 => bus.command = 6, 1 => bus.put(mac::CHANNEL_PAGE, 4, 0),
            2 => bus.put(mac::HFC_CONTROL, 4, 9), 3 => bus.put(mac::PLE_CONFIG, 4, 0),
            4 => bus.put(0x10a0, 2, 0x100), 5 => bus.put(0x1092, 1, 1), _ => bus.command = u16::MAX }
        let err = pcie::prepare(&mut bus).unwrap_err();
        assert_eq!(err.stage, pcie::Stage::Guard); assert!(bus.writes.is_empty());
        assert_eq!(err.l1_restore, None); assert_eq!(err.stop, None);
    }
}

#[test]
fn unsupported_link_rate_is_rejected_without_l1_writes() {
    let mut bus = Device::new(); bus.config[0x82] = 3;
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Refclock); assert_eq!(err.cause.register, 0x82);
    assert!(bus.dbi_writes.is_empty()); bus.stopped();
}

#[test]
fn reference_clock_timeout_restores_l1_under_an_independent_budget() {
    let mut bus = Device::new(); bus.stuck_mdio = Some(0x70);
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Refclock); assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(bus.now, 2_000); assert_eq!(bus.config[0x719], 0x38);
    assert_eq!(err.l1_restore, None); assert_eq!(err.stop, None); bus.stopped();
}

#[test]
fn frozen_clock_cannot_make_mdio_poll_forever() {
    let mut bus = Device::new(); bus.stuck_mdio = Some(0x70); bus.frozen = true;
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.cause.kind, FaultKind::Timeout); assert!(bus.polls < 300);
    assert_eq!(bus.now, 0); assert_eq!(bus.config[0x719], 0x38); bus.stopped();
}

#[test]
fn each_applied_preparation_write_failure_retains_primary_error_and_restores_l1() {
    let mut reference = Device::new(); pcie::prepare(&mut reference).unwrap();
    for index in 0..reference.writes.len() {
        let mut bus = Device::new(); bus.fail_write = Some(index);
        let err = pcie::prepare(&mut bus).unwrap_err();
        assert_eq!(err.cause.kind, FaultKind::Access, "write {index}");
        assert_eq!(err.cause.register, reference.writes[index].0);
        assert_eq!(bus.config[0x719], 0x38, "write {index}");
        assert_eq!(err.l1_restore, None); assert_eq!(err.stop, None); bus.stopped();
    }
}

#[test]
fn pending_dbi_write_finishes_before_cleanup_replaces_its_data() {
    let mut reference = Device::new(); pcie::prepare(&mut reference).unwrap();
    let index = reference.writes.iter().position(|entry| *entry == (0x1092, 1, 1)).unwrap();
    let mut bus = Device::new(); bus.dbi_delay = 3; bus.fail_write = Some(index);
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.cause.kind, FaultKind::Access); assert_eq!(err.l1_restore, None);
    assert_eq!(bus.dbi_writes, [(0x719, 0x30), (0x719, 0x38)]); bus.stopped();
}

#[test]
fn l1_restoration_readback_failure_is_preserved() {
    let mut bus = Device::new(); bus.restore_fails = true;
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Refclock); assert_eq!(err.cause.register, 0x719);
    assert_eq!(err.l1_restore.unwrap().register, 0x719); assert_eq!(bus.config[0x719], 0x30);
    bus.stopped();
}

#[test]
fn failed_stop_readback_does_not_claim_cleanup_success() {
    let mut bus = Device::new(); bus.ignore = Some(fwcmd::DMA_STOP);
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.cause.register, fwcmd::DMA_STOP); assert_eq!(err.stop.unwrap().register, fwcmd::DMA_STOP);
    assert_eq!(bus.command, 2); assert_eq!(bus.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
}

#[test]
fn failed_index_clear_cannot_progress_to_modes_or_a_ring() {
    let mut bus = Device::new(); bus.stuck_index = Some(0x1050);
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Indices); assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(bus.now, 100); assert!(!bus.writes.iter().any(|(at, _, _)| *at == 0x8810)); bus.stopped();
}

#[test]
fn late_final_readback_is_not_reported_as_success() {
    let mut bus = Device::new(); bus.late_final_read = true;
    let err = pcie::prepare(&mut bus).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Modes); assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(bus.now, 50_000); bus.stopped();
}

#[test]
fn bdram_reset_follows_installed_ring_and_keeps_all_channels_stopped() {
    let region = Region::new(); let mut bus = Device::new();
    let prepared = pcie::prepare(&mut bus).unwrap(); let mut q = region.queue(&mut bus); q.install(&mut bus).unwrap();
    prepared.finish(&mut bus, &q).unwrap();
    for (at, value) in [(0x1200, 0x020500), (0x1204, 0x020505), (0x1208, 0x02050a),
        (0x120c, 0x02050f), (0x1220, 0x010414), (0x1224, 0x010418), (0x1228, 0x01041c)] {
        assert_eq!(bus.get(at, 4), value);
    }
    let base = bus.writes.iter().position(|(at, _, _)| *at == fwcmd::BASE_LOW).unwrap();
    let reset = bus.writes.iter().position(|(at, _, value)| *at == fwcmd::INIT_CONFIG && value & 8 != 0).unwrap();
    assert!(base < reset); assert_eq!(q.state(), fwcmd::State::Installed); bus.stopped();
}

#[test]
fn ring_finish_refuses_changed_state_without_writes() {
    for case in 0..5 {
        let region = Region::new(); let mut bus = Device::new(); let prepared = pcie::prepare(&mut bus).unwrap();
        let mut q = region.queue(&mut bus); if case != 0 { q.install(&mut bus).unwrap(); }
        match case { 1 => bus.command = 6, 2 => bus.put(fwcmd::BASE_LOW, 4, 0x300000),
            3 => bus.put(R_AX_PLATFORM_ENABLE, 4, 2), 4 => bus.put(R_AX_IC_PWR_STATE, 4, 0), _ => () }
        let writes = bus.writes.len(); let err = prepared.finish(&mut bus, &q).unwrap_err();
        assert_eq!(err.stage, pcie::Stage::Ring); assert_eq!(err.stop, None); assert_eq!(bus.writes.len(), writes);
    }
}

#[test]
fn stuck_bdram_reset_keeps_the_queue_and_dma_memory_owned() {
    let region = Region::new(); let mut bus = Device::new(); let prepared = pcie::prepare(&mut bus).unwrap();
    let mut q = region.queue(&mut bus); q.install(&mut bus).unwrap(); bus.stuck_reset = true;
    let err = prepared.finish(&mut bus, &q).unwrap_err();
    assert_eq!(err.stage, pcie::Stage::Bdram); assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(q.state(), fwcmd::State::Installed); assert_eq!(q.physical(), 0x200000);
    assert_eq!(bus.now, 100); bus.stopped();
}

#[test]
fn every_bdram_write_failure_attempts_stop_without_releasing_the_queue() {
    for relative in 0..8 {
        let region = Region::new(); let mut bus = Device::new(); let prepared = pcie::prepare(&mut bus).unwrap();
        let mut q = region.queue(&mut bus); q.install(&mut bus).unwrap(); bus.fail_write = Some(bus.writes.len() + relative);
        let err = prepared.finish(&mut bus, &q).unwrap_err();
        assert_eq!(err.stage, pcie::Stage::Bdram); assert_eq!(err.cause.kind, FaultKind::Access);
        assert_eq!(q.state(), fwcmd::State::Installed); assert_eq!(err.stop, None); bus.stopped();
    }
}
