//! Native diagnostic ordering and retention boundaries; no radio endpoint or retry.

use rtw89_wifi::{fwcmd, io::{self, Fault, FaultKind, RegisterIo, TimedIo}};

pub const DMA_GRANT_BYTES: usize = 8 * 1024 * 1024;
pub const COMMAND_REGION_BYTES: usize = 128 * 1024;
pub const RECEIVE_REGION_BYTES: usize = DMA_GRANT_BYTES - COMMAND_REGION_BYTES;
const STOP_ALL: u32 = 0x001f_0f03;
const STOP_DATA: u32 = 0x000b_0f03;
const HCI: u32 = 1 << 11 | 1 << 13;

pub fn valid_dma(address: usize, length: usize, physical: u64) -> bool {
    address != 0 && address & 4095 == 0 && physical != 0 && physical & 4095 == 0
        && length >= DMA_GRANT_BYTES && length <= isize::MAX as usize
        && address.checked_add(length).is_some()
        && physical.checked_add(length as u64).is_some_and(|end| end <= 1u64 << 32)
}

fn require(bus: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let actual = io::read32(bus, at)?;
    if actual & mask != expected { return Err(Fault::new(FaultKind::PowerState, at, actual)); }
    Ok(())
}

/// Only after queue installation, PCIe finish and kernel mastering; all ordinary DMA stays stopped.
pub fn command_dma(bus: &mut impl fwcmd::Bus) -> io::Result<()> {
    let command = bus.pci_command()?;
    if command == u16::MAX || command & 6 != 6 { return Err(Fault::new(FaultKind::Access, 4, command as u32)); }
    let mut timed = TimedIo::new(bus, 1_000);
    require(&mut timed, fwcmd::DMA_STOP, STOP_ALL, STOP_ALL)?;
    require(&mut timed, fwcmd::INIT_CONFIG, HCI, 0)?;
    io::modify32(&mut timed, fwcmd::DMA_STOP, 1 << 18 | 1 << 20, 0)?;
    require(&mut timed, fwcmd::DMA_STOP, STOP_ALL, STOP_DATA)?;
    io::modify32(&mut timed, fwcmd::INIT_CONFIG, 1 << 13, 1 << 11)?;
    require(&mut timed, fwcmd::INIT_CONFIG, HCI, 1 << 11)?;
    timed.delay_us(0)
}

/// Do not replace an uncertain indirect command during the later MAC power-off sequence.
pub fn power_off_ready(bus: &mut impl RegisterIo) -> io::Result<()> {
    io::poll32(bus, 0x270, 1 << 31, 0, 50, 50_000)?;
    bus.delay_us(0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Verify, Dma, Power, Mac, Pcie, Install, Finish, InstallReceive, Resume, Start, Download, RuntimeMac, Receive }

pub trait Diagnostic {
    type Error;
    fn step(&mut self, stage: Stage) -> Result<(), Self::Error>;
    fn stop(&mut self) -> Result<(), Self::Error>;
    fn registers_only(&mut self) -> Result<(), Self::Error>;
    fn power_off(&mut self) -> Result<(), Self::Error>;
    fn quiesce(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub struct Report<E> {
    pub failure: Option<(Stage, E)>,
    pub stop: Option<Result<(), E>>,
    pub registers: Option<Result<(), E>>,
    pub power_off: Option<Result<(), E>>,
    pub quiesce: Option<Result<(), E>>,
}

/// Every modified attempt stops independently, verifies kernel BM-off/MEM-on, then powers off; DMA stays retained.
pub fn run<D: Diagnostic>(device: &mut D) -> Report<D::Error> {
    let mut report = Report { failure: None, stop: None, registers: None, power_off: None, quiesce: None };
    for stage in [Stage::Verify, Stage::Dma] {
        if let Err(error) = device.step(stage) { report.failure = Some((stage, error)); return report; }
    }
    for stage in [Stage::Power, Stage::Mac, Stage::Pcie, Stage::Install, Stage::Finish, Stage::InstallReceive,
        Stage::Resume, Stage::Start, Stage::Download, Stage::RuntimeMac, Stage::Receive] {
        if let Err(error) = device.step(stage) { report.failure = Some((stage, error)); break; }
    }
    report.stop = Some(device.stop());
    report.registers = Some(device.registers_only());
    if report.registers.as_ref().is_some_and(Result::is_ok) {
        report.power_off = Some(device.power_off());
        report.quiesce = Some(device.quiesce());
    }
    report
}
