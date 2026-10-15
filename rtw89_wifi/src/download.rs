// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! One bounded, hash-pinned RTL8852B WCPU download; no signature verification, RF or native activation.

use crate::board::Board;
use crate::firmware::{self, Image, SelectedImage};
use crate::fwcmd::{self, Bus, Queue};
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::mac;
use crate::registers::*;

pub const FILE_BYTES: usize = 1_245_944;
pub const FILE_SHA256: [u8; 32] = [0x5b,0x68,0x41,0x5e,0x3b,0xfe,0x72,0x71,0x5d,0x63,0xa7,0x07,0x03,0xd4,0x47,0x1b,0x04,0xb5,0x47,0x5b,0x8f,0xf6,0x9c,0xfd,0xc8,0xcd,0xf4,0x82,0x33,0xcd,0x5d,0x3a];
pub const FW_CONTROL: usize = 0x1e0;
const SEC_CTRL: usize = 0xc00;
const SYS_CLOCK: usize = 0x8;
const CPU_CLOCK: u32 = 1 << 14;
const WCPU: u32 = 1 << 1;
const DATA_STOP: u32 = 0x000b_0f03;
const ALL_STOP: u32 = 0x001f_0f03;
const STAGE_US: u64 = 2_000_000;
const PATH_US: u32 = 400_000;
const VERSION: [u8; 4] = [0, 29, 29, 15];
const SECTIONS: [(u8, u32, usize); 3] = [(2, 0x1897_0000, 310_992), (1, 0x18e1_2400, 13_432), (9, 0x18e1_1928, 2_048)];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyError { Board, Security, Length, Hash, Format(firmware::Error), Image, Section }

/// The private file remains immutably borrowed; only the exact pinned cut-1 normal-CE image is admitted.
#[derive(Debug)]
pub struct Verified<'a> { selected: SelectedImage<'a>, image: Image<'a> }

impl<'a> Verified<'a> {
    /// Board maps must be the completed OTP read from this same exclusively owned function, not defaults.
    /// The digest pins local bytes; it is neither a vendor signature nor secure-boot support.
    pub fn verify(board: &Board, file: &'a [u8]) -> core::result::Result<Self, VerifyError> {
        if board.identity.cut != 1 || board.identity.rfe != 1 { return Err(VerifyError::Board); }
        let at = 0x5ec - crate::efuse::PHYCAP_AT;
        if board.maps.phycap[at..at + 2] != [0xff, 0xff] { return Err(VerifyError::Security); }
        if file.len() != FILE_BYTES { return Err(VerifyError::Length); }
        if mind::sha256::digest(file) != FILE_SHA256 { return Err(VerifyError::Hash); }
        Self::parse_pinned(file)
    }

    fn parse_pinned(file: &'a [u8]) -> core::result::Result<Self, VerifyError> {
        let selected = firmware::select(file, 1, firmware::NORMAL_CE).map_err(VerifyError::Format)?;
        let image = firmware::parse(selected.bytes).map_err(VerifyError::Format)?;
        if selected.cut != 1 || selected.container_offset != 96 || selected.bytes.len() != 326_616
            || image.version != VERSION || image.commit_id != 0x6fb3_ec41 || image.header_len != 144
            || image.dynamic_header.len() != 64 || image.idmem_share_mode != 0 || image.sections().len() != 3 {
            return Err(VerifyError::Image);
        }
        for (section, &(kind, address, length)) in image.sections().iter().zip(SECTIONS.iter()) {
            if section.kind != kind || section.download_address != address || section.bytes.len() != length
                || section.checksum_present || section.redownload { return Err(VerifyError::Section); }
        }
        Ok(Self { selected, image })
    }

    fn header(&self) -> [u8; 88] {
        let mut packet = [0u8; 88];
        // FWDL uses sequence zero without ACK flags and leaves the ordinary H2C sequence unadvanced.
        packet[..4].copy_from_slice(&13u32.to_le_bytes());
        packet[4..8].copy_from_slice(&88u32.to_le_bytes());
        packet[8..].copy_from_slice(&self.selected.bytes[..80]);
        packet[36..38].copy_from_slice(&(firmware::DOWNLOAD_PART_SIZE as u16).to_le_bytes());
        packet
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Guard, CpuReset, CpuStart, H2cReady, Header, DownloadReady, Sections, Drain, FirmwareReady }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cause { Io(Fault), Transport(fwcmd::Error), FirmwareStatus(u8) }
impl From<Fault> for Cause { fn from(value: Fault) -> Self { Self::Io(value) } }
impl From<fwcmd::Error> for Cause { fn from(value: fwcmd::Error) -> Self { Self::Transport(value) } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure { pub stage: Stage, pub cause: Cause, pub dma_stop: Option<Fault>, pub cpu_stop: Option<Fault> }

/// Status 7 observed for this command session; full MAC initialization, C2H, RF calibration and WLAN remain separate.
#[derive(Debug)]
pub struct Ready { queue_session: u64 }
impl Ready {
    pub fn version(&self) -> [u8; 4] { VERSION }
    pub(crate) fn session(&self) -> u64 { self.queue_session }
}

fn require(bus: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let value = io::read32(bus, at)?;
    if value & mask != expected { return Err(Fault::new(FaultKind::PowerState, at, value)); }
    Ok(())
}

fn modify(bus: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    io::modify32(bus, at, clear, set)?;
    require(bus, at, clear | set, set)
}

fn guard(bus: &mut impl Bus, queue: &Queue<'_>) -> core::result::Result<(), Cause> {
    let command = bus.pci_command()?;
    if command == u16::MAX || command & 6 != 6 { return Err(Fault::new(FaultKind::Access, 4, command as u32).into()); }
    if !queue.fresh_running() { return Err(fwcmd::Error::State.into()); }
    require(bus, SYS_CFG1, CHIP_VERSION_MASK, 1 << 12)?;
    require(bus, R_AX_IC_PWR_STATE, B_AX_WLMAC_PWR_STE_MASK, 1 << 8)?;
    require(bus, R_AX_PLATFORM_ENABLE, 3, 1)?;
    let bb = bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb & 3 != 0 { return Err(Fault::new(FaultKind::PowerState, R_AX_SYS_FUNC_EN, bb as u32).into()); }
    require(bus, R_AX_DMAC_FUNC_EN, 0x64c4_0000, 0x64c4_0000)?;
    require(bus, mac::DMAC_CLOCK, 0x0484_0000, 0x0484_0000)?;
    require(bus, mac::WDE_CONFIG, 0x1fff_3f03, 0)?;
    require(bus, mac::PLE_CONFIG, 0x1fff_3f03, 0x0040_0801)?;
    require(bus, mac::WDE_READY, 3, 3)?;
    require(bus, mac::PLE_READY, 3, 3)?;
    require(bus, mac::HFC_CONTROL, 0xc09, 8)?;
    require(bus, mac::CHANNEL_PAGE, u32::MAX, 40 << 16)?;
    require(bus, fwcmd::INIT_CONFIG, 1 << 11 | 1 << 13, 1 << 11)?;
    require(bus, fwcmd::DMA_STOP, ALL_STOP, DATA_STOP)?;
    require(bus, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0)?;
    require(bus, fwcmd::BASE_LOW, u32::MAX, queue.physical() as u32)?;
    require(bus, fwcmd::BASE_HIGH, u32::MAX, 0)?;
    require(bus, fwcmd::INDEX, 0x0fff_0fff, 0)?;
    require(bus, fwcmd::BDRAM, u32::MAX, 0x0104_1c)?;
    let count = bus.read16(fwcmd::NUMBER)?;
    if count != fwcmd::DESCRIPTORS as u16 { return Err(fwcmd::Error::State.into()); }
    Ok(())
}

fn disable_cpu(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify(bus, R_AX_PLATFORM_ENABLE, WCPU, 0)?;
    modify(bus, FW_CONTROL, 7, 0)?;
    modify(bus, SYS_CLOCK, CPU_CLOCK, 0)?;
    modify(bus, R_AX_PLATFORM_ENABLE, 4, 0)?;
    modify(bus, R_AX_PLATFORM_ENABLE, 0, 4)?;
    modify(bus, R_AX_PLATFORM_ENABLE, 1, 0)?;
    modify(bus, R_AX_PLATFORM_ENABLE, 0, 1)?;
    bus.delay_us(0)
}

fn enable_cpu(bus: &mut impl RegisterIo) -> io::Result<()> {
    require(bus, R_AX_PLATFORM_ENABLE, WCPU, 0)?;
    for at in [0x1f4, 0x1f8, 0x160, 0x164, 0x168, 0x16c] { bus.write32(at, 0)?; }
    modify(bus, SYS_CLOCK, 0, CPU_CLOCK)?;
    modify(bus, FW_CONTROL, 0xe7, 1)?;
    modify(bus, SEC_CTRL, 3 << 16, 2 << 16)?;
    io::modify16(bus, 0x1e6, 7, 0)?;
    let reason = bus.read16(0x1e6)?;
    if reason & 7 != 0 { return Err(Fault::new(FaultKind::PowerState, 0x1e6, reason as u32)); }
    // WCPU can change FW_CONTROL as soon as it starts; only check PLATFORM_ENABLE after this write.
    modify(bus, R_AX_PLATFORM_ENABLE, 0, WCPU)
}

fn stop_dma(bus: &mut impl RegisterIo) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 1_000);
    modify(&mut timed, fwcmd::DMA_STOP, 0, ALL_STOP)?;
    modify(&mut timed, fwcmd::INIT_CONFIG, 1 << 11 | 1 << 13, 0)?;
    io::poll32(&mut timed, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0, 1, 100)?;
    timed.delay_us(0)
}

fn wait_status(bus: &mut impl RegisterIo, mask: u8, expected: u8) -> core::result::Result<(), Cause> {
    let mut timed = TimedIo::new(bus, PATH_US as u64);
    for _ in 0..=PATH_US {
        let raw = timed.read8(FW_CONTROL)?;
        timed.delay_us(0)?;
        if raw == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, FW_CONTROL, raw as u32).into()); }
        let status = (raw >> 5) & 7;
        if matches!(status, 2 | 3 | 4) { return Err(Cause::FirmwareStatus(status)); }
        if raw & mask == expected { return Ok(()); }
        timed.delay_us(1)?;
    }
    Err(Fault::new(FaultKind::Timeout, FW_CONTROL, 0).into())
}

fn send(bus: &mut impl RegisterIo, queue: &mut Queue<'_>, bytes: &[u8], part: bool) -> core::result::Result<(), Cause> {
    let mut timed = TimedIo::new(bus, PATH_US as u64);
    for _ in 0..=PATH_US / 50 {
        match queue.submit(&mut timed, bytes, part) {
            Ok(()) => { timed.delay_us(0)?; return Ok(()); }
            Err(fwcmd::Error::Full) => timed.delay_us(50)?,
            Err(cause) => return Err(cause.into()),
        }
    }
    Err(Fault::new(FaultKind::Timeout, fwcmd::INDEX, queue.pending() as u32).into())
}

/// Independent device-stop results; even success does not clear PCI mastering or permit DMA-region release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stop { pub dma: Option<Fault>, pub cpu: Option<Fault> }

/// End this exclusively owned attempt, preserving both errors and all DMA memory until kernel quiescence.
pub fn stop(bus: &mut impl RegisterIo, queue: &mut Queue<'_>) -> Stop {
    queue.poison();
    let dma = stop_dma(bus).err();
    let cpu = disable_cpu(&mut TimedIo::new(bus, 1_000)).err();
    Stop { dma, cpu }
}

/// Run only after verified-file policy, PCIe finish and reviewed command-only DMA activation on this same device.
/// The caller keeps all ordinary channels stopped, exclusive MMIO/DMA ownership and the queue's allocation alive.
/// No retry, free, power-off or PCI Command change occurs here; failures still require kernel-verified quiescence.
pub fn run(bus: &mut impl Bus, queue: &mut Queue<'_>, firmware: Verified<'_>) -> core::result::Result<Ready, Failure> {
    let fail = |cause| Failure { stage: Stage::Guard, cause, dma_stop: None, cpu_stop: None };
    guard(bus, queue).map_err(fail)?;
    let mut stage = Stage::CpuReset;
    let result = (|| {
        let mut timed = TimedIo::new(bus, STAGE_US);
        disable_cpu(&mut timed)?;
        stage = Stage::CpuStart;
        enable_cpu(&mut timed)?;
        stage = Stage::H2cReady;
        wait_status(&mut timed, 2, 2)?;
        stage = Stage::Header;
        send(&mut timed, queue, &firmware.header(), false)?;
        stage = Stage::DownloadReady;
        wait_status(&mut timed, 4, 4)?;
        timed.write32(0x160, 0)?;
        timed.write32(0x164, 0)?;
        stage = Stage::Sections;
        for section in firmware.image.sections() {
            for part in section.bytes.chunks(firmware::DOWNLOAD_PART_SIZE) { send(&mut timed, queue, part, true)?; }
        }
        stage = Stage::Drain;
        queue.drain(&mut timed, PATH_US)?;
        timed.delay_us(0)?;
        for _ in 0..5 { timed.delay_us(1_000)?; }
        stage = Stage::FirmwareReady;
        wait_status(&mut timed, 0xe0, 0xe0)?;
        timed.delay_us(0)?;
        Ok::<(), Cause>(())
    })();
    if let Err(cause) = result {
        let stopped = stop(bus, queue);
        return Err(Failure { stage, cause, dma_stop: stopped.dma, cpu_stop: stopped.cpu });
    }
    Ok(Ready { queue_session: queue.session() })
}

#[cfg(test)]
#[path = "../../tests/rtw89_download_cases.rs"]
pub(crate) mod tests;
