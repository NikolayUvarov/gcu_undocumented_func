#![no_std]
#![no_main]

use core::mem::ManuallyDrop;
use mind::abi::{BootInfo, CAP_KIND_DMA, CAP_KIND_ENDPOINT, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM, SLOT_VFS};
use mind::dev::{cap_info, device_config, Dma, Mmio};
use mind::mem::Pages;
use rtw89_wifi::{board, btc_session, download, fwcmd, initial_phy, mac, native_dma, pcie, phycap, power, post_mac, runtime_mac, rx, io::{self, Fault, FaultKind, RegisterIo, TimedIo}};
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

impl btc_session::DeviceIo for Native {
    fn pci_id(&mut self) -> io::Result<u32> {
        device_config(SLOT_DEV0, 0).map_err(|_| Fault::new(FaultKind::Access, 0, 0))
    }
    fn bar_len(&self) -> usize { self.bar.len() }
}

enum Error {
    Grant, Length, State, System(mind::sys::Error), File(mind::fs::Error), Verify(download::VerifyError),
    Io(Fault), Mac(mac::Failure), Pcie(pcie::Failure), Queue(fwcmd::Error), Download(download::Failure),
    PhyCap(phycap::Failure), Runtime(runtime_mac::Failure), Receive(rx::Error), PostMac(post_mac::Failure), Stop(Stopped),
    InitialPhy(initial_phy::Failure), NativeDma(native_dma::Error),
}

#[derive(Debug)]
struct Stopped { transmit: Option<native_dma::TransmitStop>, receive: Option<rx::Stop>, radio: Option<Fault>, command: download::Stop }

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (name, detail): (&str, Option<&dyn core::fmt::Debug>) = match self {
            Self::Grant => ("Grant", None), Self::Length => ("Length", None), Self::State => ("State", None),
            Self::System(e) => ("System", Some(e)), Self::File(e) => ("File", Some(e)), Self::Verify(e) => ("Verify", Some(e)),
            Self::Io(e) => ("Io", Some(e)), Self::Mac(e) => ("Mac", Some(e)), Self::Pcie(e) => ("Pcie", Some(e)),
            Self::Queue(e) => ("Queue", Some(e)), Self::Download(e) => ("Download", Some(e)), Self::Stop(e) => ("Stop", Some(e)),
            Self::PhyCap(e) => ("PhyCap", Some(e)),
            Self::InitialPhy(e) => ("InitialPhy", Some(e)), Self::NativeDma(e) => ("NativeDma", Some(e)),
            Self::Runtime(e) => ("Runtime", Some(e)), Self::Receive(e) => ("Receive", Some(e)), Self::PostMac(e) => ("PostMac", Some(e)),
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
    dma: Option<ManuallyDrop<Dma>>, queues: Option<native_dma::Queues<'a>>,
    prepared: Option<pcie::Prepared>, ready: Option<download::Ready>, runtime: Option<runtime_mac::Prepared>,
    capabilities: Option<phycap::Capabilities>, receiving: Option<post_mac::Receiving>, radio_attempted: bool,
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
                if !startup::valid_dma(address as usize, length, physical)
                    || startup::COMMAND_REGION_BYTES != native_dma::COMMAND_BYTES
                    || startup::RECEIVE_REGION_BYTES != native_dma::RECEIVE_BYTES
                    || startup::TRANSMIT_REGION_OFFSET != native_dma::TRANSMIT_OFFSET
                    || startup::TRANSMIT_REGION_BYTES != native_dma::TRANSMIT_BYTES { return Err(Error::Grant); }
                // This wrapper reserves three disjoint WB regions; the full mapping remains retained on every error.
                self.queues = Some(unsafe { native_dma::Queues::new(&mut self.native, address, length, physical) }.map_err(Error::NativeDma)?);
                mind::println!("[RTW89] VERIFIED FILE; RETAINED DMA {:X}, {} BYTES, PCI MEM ONLY", physical, length);
            }
            Stage::Power => {
                let mut timed = TimedIo::new(&mut self.native, 1_000_000);
                power::on_for_board_read(&mut timed).map_err(Error::Io)?;
                timed.delay_us(0).map_err(Error::Io)?;
            }
            Stage::Mac => mac::prepare_download(&mut self.native).map_err(Error::Mac)?,
            Stage::Pcie => self.prepared = Some(pcie::prepare(&mut self.native).map_err(Error::Pcie)?),
            Stage::Install => self.queues.as_mut().ok_or(Error::State)?.command.install(&mut self.native).map_err(Error::Queue)?,
            Stage::Finish => self.prepared.take().ok_or(Error::State)?.finish(&mut self.native, &self.queues.as_ref().ok_or(Error::State)?.command).map_err(Error::Pcie)?,
            Stage::InstallReceive => self.queues.as_mut().ok_or(Error::State)?.receive.install(&mut self.native).map_err(Error::Receive)?,
            Stage::InstallTransmit => self.queues.as_mut().ok_or(Error::State)?.install_transmit(&mut self.native).map_err(Error::NativeDma)?,
            Stage::Resume => mind::platform::resume(self.index).map_err(Error::System)?,
            Stage::Start => {
                startup::command_dma(&mut self.native).map_err(Error::Io)?;
                // Verified image, installed rings and completed MAC/PCIe precede command-only activation; DMA remains retained.
                unsafe { self.queues.as_mut().ok_or(Error::State)?.command.start(&mut self.native) }.map_err(Error::Queue)?;
            }
            Stage::Download => {
                let ready = download::run(&mut self.native, &mut self.queues.as_mut().ok_or(Error::State)?.command, self.verified.take().ok_or(Error::State)?).map_err(Error::Download)?;
                mind::println!("[RTW89] FW STATUS 7 OBSERVED: {:?}", ready.version());
                self.ready = Some(ready);
            }
            Stage::PhyCap => {
                let bar_bytes = self.native.bar.len();
                let capabilities = phycap::query(&mut self.native, &mut self.queues.as_mut().ok_or(Error::State)?.command, self.board,
                    bar_bytes, self.ready.as_ref().ok_or(Error::State)?, phycap::MAX_TIMEOUT_US).map_err(Error::PhyCap)?;
                mind::println!("[RTW89] FW PHY CAP RAW {:08X?}; TX/RX NSS {}/{}; ZERO-NSS DEFAULTS {}/{}",
                    capabilities.raw(), capabilities.tx_nss(), capabilities.rx_nss(), capabilities.tx_nss_defaulted(), capabilities.rx_nss_defaulted());
                self.capabilities = Some(capabilities);
            }
            Stage::RuntimeMac => {
                self.radio_attempted = true;
                let bar_bytes = self.native.bar.len();
                self.runtime = Some(runtime_mac::prepare(&mut self.native, &mut self.queues.as_mut().ok_or(Error::State)?.command, self.board,
                    bar_bytes, self.ready.as_ref().ok_or(Error::State)?).map_err(Error::Runtime)?);
            }
            Stage::Receive => {
                let bar_bytes = self.native.bar.len();
                let queues = self.queues.as_mut().ok_or(Error::State)?;
                self.receiving = Some(post_mac::activate(&mut self.native, &mut queues.command,
                    &mut queues.receive, self.board, bar_bytes, self.runtime.as_ref().ok_or(Error::State)?).map_err(Error::PostMac)?);
                mind::println!("[RTW89] MAC/RX DMA PREPARED; DATA TX STOPPED, RADIO CALIBRATION AND WLAN PENDING");
            }
            Stage::InitialPhy => {
                let mut scratch = [0; rx::BUFFER_BYTES];
                let queues = self.queues.as_mut().ok_or(Error::State)?;
                let observed = initial_phy::diagnose(&mut self.native, &mut queues.command,
                    &mut queues.receive, self.receiving.as_ref().ok_or(Error::State)?,
                    self.ready.take().ok_or(Error::State)?, self.board, self.capabilities.as_ref().ok_or(Error::State)?,
                    &mut scratch).map_err(Error::InitialPhy)?;
                mind::println!("[RTW89] INITIAL PHY/BTC DIAGNOSTIC: {:?}", observed);
                mind::println!("[RTW89] CHANNEL CALIBRATION AND WLAN PENDING; SHUTTING DOWN DIAGNOSTIC");
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), Error> {
        let command = device_config(SLOT_DEV0, 4).map_err(Error::System)?;
        if command as u16 == u16::MAX || command & 2 == 0 { return Err(Error::Io(Fault::new(FaultKind::Access, 4, command))); }
        let transmit = self.queues.as_mut().map(|queues| queues.stop_transmit(&mut self.native));
        let receive = self.queues.as_mut().map(|queues| queues.receive.stop(&mut self.native));
        let radio = if self.radio_attempted { runtime_mac::disable_radio(&mut self.native).err() } else { None };
        let command = download::stop(&mut self.native, &mut self.queues.as_mut().ok_or(Error::State)?.command);
        let stopped = Stopped { transmit, receive, radio, command };
        if stopped.transmit.as_ref().is_some_and(|stop| stop.request.is_some() || stop.idle.is_some())
            || stopped.receive.is_some_and(|stop| stop.request.is_some() || stop.hci.is_some() || stop.idle.is_some())
            || stopped.radio.is_some() || stopped.command.dma.is_some() || stopped.command.cpu.is_some() {
            Err(Error::Stop(stopped))
        } else { Ok(()) }
    }
    fn registers_only(&mut self) -> Result<(), Error> {
        mind::platform::registers_only(self.index).map_err(Error::System)?;
        self.command(2)
    }
    fn power_off(&mut self) -> Result<(), Error> {
        let mut timed = TimedIo::new(&mut self.native, 1_000_000);
        startup::power_off_ready(&mut timed).map_err(Error::Io)?;
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
    mind::println!("[RTW89] BOUNDED FIRMWARE/MAC/RX/INITIAL-PHY DIAGNOSTIC: NO WLAN NETWORK LINK");
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
        dma: None, queues: None, prepared: None, ready: None, runtime: None, capabilities: None, receiving: None, radio_attempted: false };
    let report = startup::run(&mut attempt);
    mind::println!("[RTW89] DIAGNOSTIC {:?}", report);
    mind::println!("[RTW89] DMA RETAINED={}; NO RETRY, CALIBRATED RF, WLAN ENDPOINT OR NETWORK LINK", attempt.dma.is_some());
    idle()
}

fn idle() -> ! { loop { mind::time::sleep(60_000); } }
