#![allow(dead_code)]
extern crate self as rtw89_wifi;
#[path = "../rtw89_wifi/src/io.rs"] pub mod io;
#[path = "../rtw89_wifi/src/fwcmd.rs"] pub mod fwcmd;
#[path = "../rtw89_wifi/src/startup.rs"] mod startup;

use startup::{Diagnostic, Stage};

const STEPS: [Stage; 10] = [Stage::Verify, Stage::Dma, Stage::Power, Stage::Mac, Stage::Pcie, Stage::Install, Stage::Finish, Stage::Resume, Stage::Start, Stage::Download];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event { Step(Stage), Stop, Registers, PowerOff, Quiesce }

struct Attempt { events: Vec<Event>, fail: Option<Stage>, cleanup: [bool; 4] }
impl Attempt {
    fn new() -> Self { Self { events: Vec::new(), fail: None, cleanup: [false; 4] } }
    fn clean(&mut self, event: Event, index: usize) -> Result<(), usize> {
        self.events.push(event);
        if self.cleanup[index] { Err(index) } else { Ok(()) }
    }
}
impl Diagnostic for Attempt {
    type Error = usize;
    fn step(&mut self, stage: Stage) -> Result<(), usize> {
        self.events.push(Event::Step(stage));
        if self.fail == Some(stage) { Err(99) } else { Ok(()) }
    }
    fn stop(&mut self) -> Result<(), usize> { self.clean(Event::Stop, 0) }
    fn registers_only(&mut self) -> Result<(), usize> { self.clean(Event::Registers, 1) }
    fn power_off(&mut self) -> Result<(), usize> { self.clean(Event::PowerOff, 2) }
    fn quiesce(&mut self) -> Result<(), usize> { self.clean(Event::Quiesce, 3) }
}

#[test]
fn private_file_and_dma_failures_cannot_reach_power_or_mastering() {
    for (index, stage) in STEPS[..2].iter().copied().enumerate() {
        let mut attempt = Attempt::new(); attempt.fail = Some(stage);
        let report = startup::run(&mut attempt);
        assert_eq!(report.failure, Some((stage, 99)));
        assert_eq!(attempt.events, STEPS[..=index].iter().copied().map(Event::Step).collect::<Vec<_>>());
        assert!(report.stop.is_none() && report.registers.is_none() && report.power_off.is_none() && report.quiesce.is_none());
    }
}

#[test]
fn successful_diagnostic_always_stops_after_observing_firmware_readiness() {
    let mut attempt = Attempt::new();
    let report = startup::run(&mut attempt);
    let mut expected = STEPS.map(Event::Step).to_vec();
    expected.extend([Event::Stop, Event::Registers, Event::PowerOff, Event::Quiesce]);
    assert_eq!(attempt.events, expected);
    assert!(report.failure.is_none());
    assert_eq!([report.stop, report.registers, report.power_off, report.quiesce], [Some(Ok(())); 4]);
}

#[test]
fn every_modified_failure_stops_independently_and_preserves_its_primary_cause() {
    for (index, stage) in STEPS.iter().copied().enumerate().skip(2) {
        let mut attempt = Attempt::new(); attempt.fail = Some(stage); attempt.cleanup[0] = true;
        let report = startup::run(&mut attempt);
        let mut expected = STEPS[..=index].iter().copied().map(Event::Step).collect::<Vec<_>>();
        expected.extend([Event::Stop, Event::Registers, Event::PowerOff, Event::Quiesce]);
        assert_eq!(attempt.events, expected);
        assert_eq!(report.failure, Some((stage, 99)));
        assert_eq!(report.stop, Some(Err(0)));
        assert_eq!(report.registers, Some(Ok(())));
        assert_eq!(report.power_off, Some(Ok(())));
    }
}

#[test]
fn unverified_bus_master_stop_prevents_all_further_mmio_and_retries() {
    let mut attempt = Attempt::new(); attempt.cleanup = [true, true, true, true];
    let report = startup::run(&mut attempt);
    assert_eq!(&attempt.events[STEPS.len()..], [Event::Stop, Event::Registers]);
    assert_eq!(report.stop, Some(Err(0)));
    assert_eq!(report.registers, Some(Err(1)));
    assert!(report.power_off.is_none() && report.quiesce.is_none());
}

#[test]
fn failed_poweroff_still_requests_final_kernel_stop_and_reports_both_errors() {
    let mut attempt = Attempt::new(); attempt.cleanup = [false, false, true, true];
    let report = startup::run(&mut attempt);
    assert_eq!(report.power_off, Some(Err(2)));
    assert_eq!(report.quiesce, Some(Err(3)));
    assert_eq!(attempt.events.last(), Some(&Event::Quiesce));
}

#[test]
fn dma_requires_full_retained_aligned_region_below_four_gib() {
    let size = startup::DMA_GRANT_BYTES;
    assert!(size >= fwcmd::DMA_BYTES);
    assert!(startup::valid_dma(0x10000, size, 0x20000));
    assert!(startup::valid_dma(0x10000, size, (1u64 << 32) - size as u64));
    for (address, length, physical) in [(0, size, 0x20000), (0x10001, size, 0x20000), (0x10000, fwcmd::DMA_BYTES - 1, 0x20000),
        (0x10000, size, 0), (0x10000, size, 0x20001), (0x10000, size, (1u64 << 32) - 4096),
        (usize::MAX - 4095, size, 0x20000), (0x10000, usize::MAX, 0x20000)] {
        assert!(!startup::valid_dma(address, length, physical));
    }
}

struct Bus { command: u16, stop: u32, config: u32, writes: Vec<(usize, u32)>, ignore: bool }
impl Bus { fn new(command: u16) -> Self { Self { command, stop: 0x801f_0f03, config: 0x80, writes: Vec::new(), ignore: false } } }
impl io::RegisterIo for Bus {
    fn read8(&mut self, _: usize) -> io::Result<u8> { panic!("unexpected width") }
    fn read16(&mut self, _: usize) -> io::Result<u16> { panic!("unexpected width") }
    fn read32(&mut self, at: usize) -> io::Result<u32> {
        Ok(match at { fwcmd::DMA_STOP => self.stop, fwcmd::INIT_CONFIG => self.config, _ => panic!("unexpected register") })
    }
    fn write8(&mut self, _: usize, _: u8) -> io::Result<()> { panic!("unexpected width") }
    fn write16(&mut self, _: usize, _: u16) -> io::Result<()> { panic!("unexpected width") }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> {
        self.writes.push((at, value));
        if !self.ignore { match at { fwcmd::DMA_STOP => self.stop = value, fwcmd::INIT_CONFIG => self.config = value, _ => panic!("unexpected register") } }
        Ok(())
    }
    fn now_us(&mut self) -> u64 { 0 }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { assert_eq!(us, 0); Ok(()) }
}
impl fwcmd::Bus for Bus { fn pci_command(&mut self) -> io::Result<u16> { Ok(self.command) } }

#[test]
fn native_activation_opens_only_command_channel_and_tx_hci() {
    for command in [6, 7] {
        let mut bus = Bus::new(command);
        startup::command_dma(&mut bus).unwrap();
        assert_eq!(bus.stop, 0x800b_0f03);
        assert_eq!(bus.config, 0x880);
        assert_eq!(bus.writes, [(fwcmd::DMA_STOP, 0x800b_0f03), (fwcmd::INIT_CONFIG, 0x880)]);
    }
}

#[test]
fn missing_mastering_or_inaccessible_pci_cannot_activate_any_channel() {
    for command in [0, 2, 3, u16::MAX] {
        let mut bus = Bus::new(command);
        assert!(startup::command_dma(&mut bus).is_err());
        assert!(bus.writes.is_empty());
    }
}

#[test]
fn unexpected_active_packet_channel_or_hci_is_rejected_before_writes() {
    for change in 0..2 {
        let mut bus = Bus::new(6);
        if change == 0 { bus.stop &= !1; } else { bus.config |= 1 << 13; }
        assert!(startup::command_dma(&mut bus).is_err());
        assert!(bus.writes.is_empty());
    }
}

#[test]
fn ignored_command_channel_write_does_not_enable_hci() {
    let mut bus = Bus::new(6); bus.ignore = true;
    assert!(startup::command_dma(&mut bus).is_err());
    assert_eq!(bus.writes.len(), 1);
    assert_eq!(bus.config, 0x80);
}
