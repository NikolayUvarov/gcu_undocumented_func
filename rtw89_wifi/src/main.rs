#![no_std]
#![no_main]

use core::mem::ManuallyDrop;
use mind::abi::{BootInfo, CAP_KIND_DMA, CAP_KIND_ENDPOINT, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM, SLOT_VFS};
use mind::dev::{cap_info, device_config, Dma, Mmio};
use mind::mem::Pages;
use rtw89_wifi::{board, download, fwcmd, mac, pcie, power, io::{self, Fault, FaultKind, RegisterIo, TimedIo}};
mod startup;

const FIRMWARE_PATH: &str = "data/firmware/rtw89/rtw8852b_fw-1.bin";

struct Native { bar: Mmio }

impl RegisterIo for Native {
    fn read8(&mut self, at: usize) -> io::Result<u8> { io::check_offset(at, 1, self.bar.len())?; Ok(self.bar.read8(at)) }
    fn read16(&mut self, at: usize) -> io::Result<u16> { io::check_offset(at, 2, self.bar.len())?; Ok(self.bar.read16(at)) }
    fn read32(&mut self, at: usize) -> io::Result<u32> { io::check_offset(at, 4, self.bar.len())?; Ok(self.bar.read32(at)) }
    fn write8(&mut self, at: usize, value: u8) -> io::Result<()> { io::check_offset(at, 1, self.bar.len())?; self.bar.write8(at, value); Ok(()) }
    fn write16(&mut self, at: usize, value: u16) -> io::Result<()> { io::check_offset(at, 2, self.bar.len())?; self.bar.write16(at, value); Ok(()) }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> { io::check_offset(at, 4, self.bar.len())?; self.bar.write32(at, value); Ok(()) }
    fn now_us(&mut self) -> u64 { mind::time::monotonic_ns() / 1_000 }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { io::short_delay(mind::time::monotonic_ns, us) }
}

impl fwcmd::Bus for Native {
    fn pci_command(&mut self) -> io::Result<u16> {
        device_config(SLOT_DEV0, 4).map(|value| value as u16).map_err(|_| Fault::new(FaultKind::Access, 4, 0))
    }
}

enum Error {
    Grant, Length, State, System(mind::sys::Error), File(mind::fs::Error), Verify(download::VerifyError),
    Io(Fault), Mac(mac::Failure), Pcie(pcie::Failure), Queue(fwcmd::Error), Download(download::Failure), Stop(download::Stop),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (name, detail): (&str, Option<&dyn core::fmt::Debug>) = match self {
            Self::Grant => ("Grant", None), Self::Length => ("Length", None), Self::State => ("State", None),
            Self::System(e) => ("System", Some(e)), Self::File(e) => ("File", Some(e)), Self::Verify(e) => ("Verify", Some(e)),
            Self::Io(e) => ("Io", Some(e)), Self::Mac(e) => ("Mac", Some(e)), Self::Pcie(e) => ("Pcie", Some(e)),
            Self::Queue(e) => ("Queue", Some(e)), Self::Download(e) => ("Download", Some(e)), Self::Stop(e) => ("Stop", Some(e)),
        };
        match detail { Some(value) => f.debug_tuple(name).field(value).finish(), None => f.write_str(name) }
    }
}

// Open/close have finite IPC waits; each data chunk consumes the remainder of one five-second read budget.
fn firmware_file() -> Result<Pages, Error> {
    if cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT { return Err(Error::Grant); }
    mind::idl::wire::with_timeout(2_000, || {
        let mut file = mind::fs::File::open(FIRMWARE_PATH).map_err(Error::File)?;
        if file.size() != download::FILE_BYTES { return Err(Error::Length); }
        let mut bytes = Pages::new(download::FILE_BYTES).ok_or(Error::System(mind::sys::Error::NoMemory))?;
        let deadline = mind::deadline::Deadline::after(mind::time::monotonic_ns() / 1_000_000, 5_000);
        for chunk in bytes.as_mut_slice().chunks_mut(mind::fs::CHUNK) {
            let count = deadline.call(|| file.read(chunk).map_err(Into::into)).map_err(Error::System)?;
            if count != chunk.len() { return Err(Error::Length); }
        }
        if mind::sha256::digest(bytes.as_slice()) != download::FILE_SHA256 { return Err(Error::Verify(download::VerifyError::Hash)); }
        Ok(bytes)
    })
}

struct Attempt<'a> {
    native: Native, index: usize, board: &'a board::Board, file: &'a [u8], verified: Option<download::Verified<'a>>,
    dma: Option<ManuallyDrop<Dma>>, queue: Option<fwcmd::Queue<'a>>, prepared: Option<pcie::Prepared>, version: Option<[u8; 4]>,
}

impl Attempt<'_> {
    fn command(&self, low_bits: u16) -> Result<(), Error> {
        let value = device_config(SLOT_DEV0, 4).map_err(Error::System)?;
        if value == u32::MAX || value as u16 & 7 != low_bits { return Err(Error::Io(Fault::new(FaultKind::Access, 4, value))); }
        Ok(())
    }
}

impl startup::Diagnostic for Attempt<'_> {
    type Error = Error;
    fn step(&mut self, stage: startup::Stage) -> Result<(), Error> {
        use startup::Stage;
        match stage {
            Stage::Verify => { self.verified = Some(download::Verified::verify(self.board, self.file).map_err(Error::Verify)?); }
            Stage::Dma => {
                self.command(2)?;
                if cap_info(SLOT_MEM).0 != CAP_KIND_DMA { return Err(Error::Grant); }
                self.dma = Some(ManuallyDrop::new(Dma::map(SLOT_MEM).map_err(Error::System)?));
                let dma = self.dma.as_mut().ok_or(Error::State)?;
                let (length, physical) = (dma.len(), dma.physical(0));
                let address = dma.bytes(0, length).as_mut_ptr();
                if !startup::valid_dma(address as usize, length, physical) { return Err(Error::Grant); }
                // Kernel DMA maps are coherent WB and BAR maps UC on x86; this retained mapping has no later CPU alias.
                self.queue = Some(unsafe { fwcmd::Queue::new(&mut self.native, address, length, physical) }.map_err(Error::Queue)?);
                mind::println!("[RTW89] VERIFIED FILE; RETAINED DMA {:X}, {} BYTES, PCI MEM ONLY", physical, length);
            }
            Stage::Power => {
                let mut timed = TimedIo::new(&mut self.native, 1_000_000);
                power::on_for_board_read(&mut timed).map_err(Error::Io)?;
                timed.delay_us(0).map_err(Error::Io)?;
            }
            Stage::Mac => mac::prepare_download(&mut self.native).map_err(Error::Mac)?,
            Stage::Pcie => self.prepared = Some(pcie::prepare(&mut self.native).map_err(Error::Pcie)?),
            Stage::Install => self.queue.as_mut().ok_or(Error::State)?.install(&mut self.native).map_err(Error::Queue)?,
            Stage::Finish => self.prepared.take().ok_or(Error::State)?.finish(&mut self.native, self.queue.as_ref().ok_or(Error::State)?).map_err(Error::Pcie)?,
            Stage::Resume => mind::platform::resume(self.index).map_err(Error::System)?,
            Stage::Start => {
                startup::command_dma(&mut self.native).map_err(Error::Io)?;
                // Verified image, installed rings and completed MAC/PCIe precede command-only activation; DMA remains retained.
                unsafe { self.queue.as_mut().ok_or(Error::State)?.start(&mut self.native) }.map_err(Error::Queue)?;
            }
            Stage::Download => {
                let ready = download::run(&mut self.native, self.queue.as_mut().ok_or(Error::State)?, self.verified.take().ok_or(Error::State)?).map_err(Error::Download)?;
                self.version = Some(ready.version());
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), Error> {
        let command = device_config(SLOT_DEV0, 4).map_err(Error::System)?;
        if command == u32::MAX || command & 2 == 0 { return Err(Error::Io(Fault::new(FaultKind::Access, 4, command))); }
        let stopped = download::stop(&mut self.native, self.queue.as_mut().ok_or(Error::State)?);
        if stopped.dma.is_some() || stopped.cpu.is_some() { Err(Error::Stop(stopped)) } else { Ok(()) }
    }
    fn registers_only(&mut self) -> Result<(), Error> {
        mind::platform::registers_only(self.index).map_err(Error::System)?;
        self.command(2)
    }
    fn power_off(&mut self) -> Result<(), Error> {
        let mut timed = TimedIo::new(&mut self.native, 1_000_000);
        power::off(&mut timed, Some(self.board.identity.rfe)).map_err(Error::Io)?;
        timed.delay_us(0).map_err(Error::Io)
    }
    fn quiesce(&mut self) -> Result<(), Error> {
        mind::platform::quiesce(self.index).map_err(Error::System)?;
        self.command(0)
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::println!("[RTW89] BOUNDED FIRMWARE DIAGNOSTIC: NO WLAN RADIO OR NETWORK LINK");
    if !cfg!(target_arch = "x86_64") { mind::println!("[RTW89] UNSUPPORTED DMA ARCHITECTURE"); idle(); }
    let (_, resolution, frequency) = mind::time::clock_info();
    if let Err(error) = io::validate_clock(resolution, frequency) {
        mind::println!("[RTW89] PRECISE CLOCK REQUIRED: {:?}", error);
        idle();
    }
    let (kind, _, len) = cap_info(SLOT_DEV0);
    if kind != CAP_KIND_MMIO { mind::println!("[RTW89] NO MMIO BAR2 GRANT"); idle(); }
    let values = [0, 4, 0x18, 0x1c].map(|offset| device_config(SLOT_DEV0, offset));
    let [Ok(id), Ok(command), Ok(low), Ok(high)] = values else { mind::println!("[RTW89] PCI CONFIG UNAVAILABLE"); idle(); };
    if let Err(error) = io::validate_grant(id, command as u16, low, high, len) {
        mind::println!("[RTW89] REJECTED GRANT {:?}; PCI {:08X}, COMMAND {:04X}", error, id, command as u16);
        idle();
    }
    let Ok(index) = mind::process::args_str().parse::<usize>() else { mind::println!("[RTW89] NO PCI DEVICE INDEX"); idle(); };
    let bytes = match firmware_file() {
        Ok(bytes) => bytes,
        Err(error) => { mind::println!("[RTW89] PRIVATE FILE {} UNAVAILABLE/REJECTED: {:?}; NO DEVICE START", FIRMWARE_PATH, error); idle(); }
    };
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { mind::println!("[RTW89] BAR2 MAP FAILED"); idle(); };
    if bar.len() != len { mind::println!("[RTW89] BAR2 SIZE CHANGED"); idle(); }
    let mut native = Native { bar };
    let board = match board::read(&mut native) {
        Ok(board) => {
            let b = board.identity;
            mind::println!("[RTW89] BOARD READ: CUT {}, RFE {}, XTAL {:02X}, COUNTRY {:02X}{:02X}, PLAN {:02X}", b.cut, b.rfe, b.xtal, b.country[0], b.country[1], b.channel_plan);
            mind::println!("[RTW89] MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}; PHY CAP {} BYTES; MAC POWERED OFF", b.mac[0], b.mac[1], b.mac[2], b.mac[3], b.mac[4], b.mac[5], board.maps.phycap.len());
            board
        }
        Err(error) => { mind::println!("[RTW89] BOARD ACCESS FAILED {:?}; NO NETWORK LINK", error); idle(); }
    };
    let mut attempt = Attempt { native, index, board: &board, file: bytes.as_slice(), verified: None,
        dma: None, queue: None, prepared: None, version: None };
    let report = startup::run(&mut attempt);
    if let Some(version) = attempt.version { mind::println!("[RTW89] FW STATUS 7 OBSERVED: {}.{}.{}.{}", version[0], version[1], version[2], version[3]); }
    mind::println!("[RTW89] DIAGNOSTIC {:?}", report);
    mind::println!("[RTW89] DMA RETAINED={}; NO RETRY, CALIBRATED RF, WLAN ENDPOINT OR NETWORK LINK", attempt.dma.is_some());
    idle()
}

fn idle() -> ! { loop { mind::time::sleep(60_000); } }
