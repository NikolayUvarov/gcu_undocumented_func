#[path = "../rtw89_wifi/src/io.rs"]
#[allow(dead_code)] mod io;
#[path = "../rtw89_wifi/src/registers.rs"]
#[allow(dead_code)] mod registers;
#[path = "../rtw89_wifi/src/fwcmd.rs"]
#[allow(dead_code)] mod fwcmd;
#[path = "../rtw89_wifi/src/mac.rs"]
#[allow(dead_code)] mod mac;

use io::{Fault, FaultKind, RegisterIo, Result};
use registers::*;

struct Device {
    registers: Vec<u8>, command: u16, now: u64, frozen: bool,
    writes: Vec<(usize, u32)>, reads: Vec<usize>, fail_write: Option<usize>,
    ignore_write: Option<usize>, cleanup_faults: bool, read_cost: u64,
    ready_after: [u64; 2], late_ready: Option<usize>, late_final_read: bool, hfc_reads: usize,
}
impl Device {
    fn new() -> Self {
        let mut d = Self { registers: vec![0; 0xc004], command: 2, now: 0, frozen: false,
            writes: Vec::new(), reads: Vec::new(), fail_write: None, ignore_write: None,
            cleanup_faults: false, read_cost: 0, ready_after: [0; 2], late_ready: None, late_final_read: false, hfc_reads: 0 };
        d.put(SYS_CFG1, 0x1000); d.put(R_AX_IC_PWR_STATE, 0x100); d
    }
    fn get(&self, at: usize) -> u32 { u32::from_le_bytes(self.registers[at..at + 4].try_into().unwrap()) }
    fn put(&mut self, at: usize, value: u32) { self.registers[at..at + 4].copy_from_slice(&value.to_le_bytes()); }
    fn no_host_dma_or_radio_writes(&self) {
        assert_eq!(self.command, 2);
        assert!(self.writes.iter().all(|(at, _)| ![fwcmd::INIT_CONFIG, fwcmd::DMA_STOP,
            R_AX_PLATFORM_ENABLE, R_AX_SYS_FUNC_EN, R_AX_WLRF_CTRL, R_AX_CMAC_FUNC_EN].contains(at)));
    }
    fn cleaned(&self) {
        assert_eq!(self.get(mac::HFC_CONTROL) & 9, 0);
        assert_eq!(self.get(R_AX_DMAC_FUNC_EN) & 0x0480_0000, 0);
        self.no_host_dma_or_radio_writes();
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> Result<u8> { Ok(self.registers[at]) }
    fn read16(&mut self, at: usize) -> Result<u16> { Ok(u16::from_le_bytes(self.registers[at..at + 2].try_into().unwrap())) }
    fn read32(&mut self, at: usize) -> Result<u32> {
        self.reads.push(at); self.now += self.read_cost;
        if at == mac::HFC_CONTROL {
            self.hfc_reads += 1;
            if self.late_final_read && self.hfc_reads == 6 { self.now += 10_000; }
        }
        if self.late_ready == Some(at) { self.now += 2_000; }
        if let Some(index) = [mac::WDE_READY, mac::PLE_READY].iter().position(|r| *r == at) {
            return Ok(if self.now >= self.ready_after[index] { 3 } else { 0 });
        }
        Ok(self.get(at))
    }
    fn write8(&mut self, _at: usize, _value: u8) -> Result<()> { panic!("unexpected byte write") }
    fn write16(&mut self, _at: usize, _value: u16) -> Result<()> { panic!("unexpected halfword write") }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> {
        let failure = self.fail_write == Some(self.writes.len()) || self.cleanup_faults &&
            (at == mac::HFC_CONTROL || at == R_AX_DMAC_FUNC_EN && self.reads.contains(&mac::WDE_READY));
        self.writes.push((at, value));
        if self.ignore_write != Some(at) { self.put(at, value); }
        if failure { return Err(Fault::new(FaultKind::Access, at, value)); }
        Ok(())
    }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> Result<()> { if !self.frozen { self.now += us as u64; } Ok(()) }
}
impl fwcmd::Bus for Device { fn pci_command(&mut self) -> Result<u16> { Ok(self.command) } }

#[test]
fn download_profile_keeps_scc_wcpu_minimum_and_only_command_flow_control() {
    let mut bus = Device::new();
    bus.put(mac::WDE_CONFIG, 0x4000_0040); bus.put(mac::PLE_CONFIG, 0x4000_0040);
    bus.put(mac::HFC_CONTROL, 0xfff);
    mac::prepare_download(&mut bus).unwrap();
    assert_eq!(bus.get(R_AX_DMAC_FUNC_EN), 0x64c4_0000);
    assert_eq!(bus.get(mac::DMAC_CLOCK), 0x0484_0000);
    assert_eq!(bus.get(mac::WDE_CONFIG), 0x4000_0040);
    assert_eq!(bus.get(mac::PLE_CONFIG), 0x4040_0841);
    assert_eq!((bus.get(mac::PLE_CONFIG) >> 8) & 63, 64 * 1024 / 8192);
    assert_eq!(64 * 1024 + 128 * (64 + 960), 196_608);
    assert_eq!(bus.get(0x8c44), 48);
    for at in [0x8c40, 0x8c4c, 0x8c50] { assert_eq!(bus.get(at), 0); }
    assert_eq!(bus.get(0x9048), 0x0010_0010); assert_eq!(bus.get(0x904c), 0x0030_0030);
    for at in [0x9040, 0x9044, 0x9050, 0x9054, 0x9058, 0x905c, 0x9060, 0x9064, 0x9068] {
        assert_eq!(bus.get(at), 0);
    }
    assert_eq!(bus.get(mac::CHANNEL_PAGE), 40 << 16);
    assert_eq!(bus.get(mac::HFC_CONTROL), 0x3fe);
    bus.no_host_dma_or_radio_writes();
}

#[test]
fn incompatible_or_active_device_is_refused_before_writes() {
    for case in 0..13 {
        let mut bus = Device::new();
        match case {
            0 => bus.command = 6, 1 => bus.command = u16::MAX,
            2 => bus.put(SYS_CFG1, 0), 3 => bus.put(SYS_CFG1, u32::MAX),
            4 => bus.put(R_AX_IC_PWR_STATE, 0), 5 => bus.put(R_AX_IC_PWR_STATE, 0x200),
            6 => bus.put(R_AX_IC_PWR_STATE, 0x300), 7 => bus.registers[R_AX_SYS_FUNC_EN] = 1,
            8 => bus.put(R_AX_PLATFORM_ENABLE, 2), 9 => bus.put(fwcmd::INIT_CONFIG, 1 << 11),
            10 => bus.put(fwcmd::INIT_CONFIG, 1 << 13), 11 => bus.put(fwcmd::DMA_BUSY, 1 << 18),
            _ => bus.put(fwcmd::DMA_BUSY, u32::MAX),
        }
        let err = mac::prepare_download(&mut bus).unwrap_err();
        assert_eq!(err.stage, mac::Stage::Guard); assert!(bus.writes.is_empty());
        assert_eq!(err.hfc_disable, None); assert_eq!(err.dle_disable, None);
    }
}

#[test]
fn both_memory_engines_must_be_ready_before_flow_control() {
    let mut bus = Device::new(); bus.ready_after = [80, 160];
    mac::prepare_download(&mut bus).unwrap();
    let wde = bus.reads.iter().position(|at| *at == mac::WDE_READY).unwrap();
    let ple = bus.reads.iter().position(|at| *at == mac::PLE_READY).unwrap();
    let hfc = bus.reads.iter().position(|at| *at == mac::HFC_CONTROL).unwrap();
    assert!(wde < ple && ple < hfc); assert_eq!(bus.now, 160);
}

#[test]
fn each_memory_engine_timeout_disables_both_engines_and_flow_control() {
    for engine in 0..2 {
        let mut bus = Device::new(); bus.ready_after[engine] = u64::MAX;
        let err = mac::prepare_download(&mut bus).unwrap_err();
        assert_eq!(err.stage, mac::Stage::Ready); assert_eq!(err.cause.kind, FaultKind::Timeout);
        assert_eq!(err.cause.register, [mac::WDE_READY, mac::PLE_READY][engine]);
        assert_eq!(bus.now, 2_000); assert_eq!(err.hfc_disable, None); assert_eq!(err.dle_disable, None);
        assert!(!bus.writes.iter().any(|(at, value)| *at == mac::HFC_CONTROL && value & 8 != 0));
        bus.cleaned();
    }
}

#[test]
fn frozen_clock_has_a_finite_poll_limit_and_still_attempts_cleanup() {
    let mut bus = Device::new(); bus.frozen = true; bus.ready_after[0] = u64::MAX;
    let err = mac::prepare_download(&mut bus).unwrap_err();
    assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert!(bus.reads.iter().filter(|at| **at == mac::WDE_READY).count() <= 2_002);
    assert_eq!(bus.now, 0); bus.cleaned();
}

#[test]
fn every_potentially_applied_write_failure_gets_independent_cleanup() {
    let mut reference = Device::new(); mac::prepare_download(&mut reference).unwrap();
    for index in 0..reference.writes.len() {
        let mut bus = Device::new(); bus.fail_write = Some(index);
        let err = mac::prepare_download(&mut bus).unwrap_err();
        assert_eq!(err.cause.kind, FaultKind::Access);
        assert_eq!(err.cause.register, reference.writes[index].0);
        assert_eq!(err.hfc_disable, None); assert_eq!(err.dle_disable, None);
        bus.cleaned();
    }
}

#[test]
fn silent_configuration_write_failure_is_not_reported_as_ready() {
    let mut bus = Device::new(); bus.ignore_write = Some(mac::PLE_CONFIG);
    let err = mac::prepare_download(&mut bus).unwrap_err();
    assert_eq!(err.stage, mac::Stage::Quotas); assert_eq!(err.cause.register, mac::PLE_CONFIG);
    assert!(!bus.reads.contains(&mac::WDE_READY)); bus.cleaned();
}

#[test]
fn both_cleanup_failures_are_reported_without_losing_primary_timeout() {
    let mut bus = Device::new(); bus.cleanup_faults = true; bus.ready_after[0] = u64::MAX;
    let err = mac::prepare_download(&mut bus).unwrap_err();
    assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(err.hfc_disable.unwrap().register, mac::HFC_CONTROL);
    assert_eq!(err.dle_disable.unwrap().register, R_AX_DMAC_FUNC_EN);
    bus.no_host_dma_or_radio_writes();
}

#[test]
fn late_ready_bits_do_not_erase_the_poll_deadline() {
    for at in [mac::WDE_READY, mac::PLE_READY] {
        let mut bus = Device::new(); bus.late_ready = Some(at);
        let err = mac::prepare_download(&mut bus).unwrap_err();
        assert_eq!(err.cause.kind, FaultKind::Timeout); assert_eq!(err.cause.register, at);
        bus.cleaned();
    }
}

#[test]
fn absolute_stage_timeout_does_not_consume_cleanup_budgets() {
    let mut bus = Device::new(); bus.read_cost = 300;
    let err = mac::prepare_download(&mut bus).unwrap_err();
    assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(err.hfc_disable, None); assert_eq!(err.dle_disable, None);
    assert!(bus.now < 15_000); bus.cleaned();
}

#[test]
fn final_successful_readback_must_finish_within_the_stage_deadline() {
    let mut bus = Device::new(); bus.late_final_read = true;
    let err = mac::prepare_download(&mut bus).unwrap_err();
    assert_eq!(err.stage, mac::Stage::FlowControl); assert_eq!(err.cause.kind, FaultKind::Timeout);
    assert_eq!(err.hfc_disable, None); assert_eq!(err.dle_disable, None);
    assert_eq!(bus.now, 10_000); bus.cleaned();
}
