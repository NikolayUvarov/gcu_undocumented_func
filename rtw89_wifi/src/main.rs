#![no_std]
#![no_main]

use core::mem::ManuallyDrop;
use mind::abi::{BootInfo, CAP_KIND_DMA, CAP_KIND_ENDPOINT, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM, SLOT_SERVICE, SLOT_VFS};
use mind::dev::{cap_info, device_config, Dma, Mmio};
use mind::mem::Pages;
use rtw89_wifi::{args, board, btc_session, download, fwcmd, listen, mac, receive_service, native_dma, pcie, phycap, power, post_mac, runtime_mac, rx, io::{self, Fault, FaultKind, RegisterIo, TimedIo}};
mod startup;

const FIRMWARE_PATH: &str = "data/firmware/rtw89/rtw8852b_fw-1.bin";
// Channel 1 has the most access points near the Ryzen (Linux scan, 2026-11-04); receive only, for 20 s.
const LISTEN_MHZ: u16 = 2412;
const LISTEN_US: u64 = 20_000_000;

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
    fn pci_status(&mut self) -> io::Result<u16> {
        device_config(SLOT_DEV0, 4).map(|value| (value >> 16) as u16).map_err(|_| Fault::new(FaultKind::Access, 6, 0))
    }
}

enum Error {
    Grant, Length, State, System(mind::sys::Error), File(mind::fs::Error), Verify(download::VerifyError),
    Io(Fault), Mac(mac::Failure), Pcie(pcie::Failure), Queue(fwcmd::Error), Download(download::Failure),
    PhyCap(phycap::Failure), Runtime(runtime_mac::Failure), Receive(rx::Error), PostMac(post_mac::Failure), Stop(Stopped),
    Listen(listen::Failure), Served(receive_service::Stop), NativeDma(native_dma::Error),
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
            Self::Listen(e) => ("Listen", Some(e)), Self::Served(e) => ("Served", Some(e)), Self::NativeDma(e) => ("NativeDma", Some(e)),
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
    platform: args::Platform<'a>,
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
            Stage::Mac => {
                if let Some(found) = mac::quiesce(&mut self.native).map_err(Error::Io)? {
                    mind::println!("[RTW89] HCI LEFT ON (INIT_CONFIG {:08X}): STOPPED WITH PCI MASTERING OFF", found);
                }
                let masks = mac::mask_host(&mut self.native).map_err(Error::Io)?;
                if masks != [0; 3] { mind::println!("[RTW89] HOST INTERRUPTS LEFT ENABLED (HIMR {:08X?}): MASKED", masks); }
                mac::prepare_download(&mut self.native).map_err(Error::Mac)?
            }
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
                // The firmware enables its halt interrupt (HIMR0 bit 21); this polled driver keeps host interrupts masked.
                let masks = mac::mask_host(&mut self.native).map_err(Error::Io)?;
                if masks != [0; 3] { mind::println!("[RTW89] THE FIRMWARE ENABLED HOST INTERRUPTS (HIMR {:08X?}): MASKED", masks); }
                // 213-DRV-0091: the device's interrupt state; with HALT_INTERRUPT_AS_LINUX, the halt bit left on where it reaches no one.
                let state = mac::pci_interrupt(|at| device_config(SLOT_DEV0, at).ok());
                match state {
                    Some(s) => mind::println!("[RTW89] PCI INTERRUPT STATE: COMMAND {:04X} (INTX {}), STATUS {:04X} (INTERRUPT {}), LINE {}, PIN {}, MSI CONTROL {:04X?}, MSI-X CONTROL {:04X?} (213-DRV-0091)",
                        s.command, if s.command & 1 << 10 != 0 { "DISABLED" } else { "ENABLED" }, s.status, s.status >> 3 & 1, s.line, s.pin, s.msi, s.msix),
                    None => mind::println!("[RTW89] PCI INTERRUPT STATE UNREADABLE (213-DRV-0091)"),
                }
                if mac::HALT_INTERRUPT_AS_LINUX {
                    match state.map(|s| mac::halt_interrupt_refusal(&s)) {
                        Some(None) => {
                            mac::halt_interrupt_on(&mut self.native).map_err(Error::Io)?;
                            mind::println!("[RTW89] HIMR0 HALT_C2H LEFT ON AS LINUX LEAVES IT; NO HANDLER, HISR0 POLLED (DEBUG, 213-DRV-0091)");
                        }
                        Some(Some(why)) => mind::println!("[RTW89] THE HALT INTERRUPT LEFT MASKED: {} (213-DRV-0091)", why),
                        None => mind::println!("[RTW89] THE HALT INTERRUPT LEFT MASKED: THE PCI STATE UNREAD (213-DRV-0091)"),
                    }
                }
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
            Stage::InitialPhy if serving() => {
                let mut scratch = [0; rx::BUFFER_BYTES];
                let queues = self.queues.as_mut().ok_or(Error::State)?;
                // The data mode's packet inbox, in BSS: four receive buffers are too large for the stack.
                static mut INBOX: btc_session::Inbox = btc_session::Inbox::new();
                let inbox = unsafe { &mut *core::ptr::addr_of_mut!(INBOX) };
                let stop = receive_service::serve(&mut self.native, queues,
                    self.receiving.as_ref().ok_or(Error::State)?, self.ready.take().ok_or(Error::State)?, self.board,
                    self.capabilities.as_ref().ok_or(Error::State)?, &mut scratch, inbox, LISTEN_MHZ, self.platform, served);
                pieces("WLANHW SERVICE STOPPED", &stop);
                return Err(Error::Served(stop));
            }
            Stage::InitialPhy => {
                let mut scratch = [0; rx::BUFFER_BYTES];
                let queues = self.queues.as_mut().ok_or(Error::State)?;
                let listened = listen::listen(&mut self.native, &mut queues.command, &mut queues.receive,
                    self.receiving.as_ref().ok_or(Error::State)?, self.ready.take().ok_or(Error::State)?, self.board,
                    self.capabilities.as_ref().ok_or(Error::State)?, &mut scratch, LISTEN_MHZ, LISTEN_US, heard);
                let listened = match listened {
                    Ok(listened) => listened,
                    Err(failure) => {
                        mind::println!("[RTW89] LISTEN FAILED: {:?}", failure.cause);
                        mind::println!("[RTW89] BEFORE THE FAILURE: {:?}", failure.tally);
                        return Err(Error::Listen(failure));
                    }
                };
                let (i, t) = (&listened.initial, listened.tally);
                mind::println!("[RTW89] INITIAL PHY: THERMAL {:?}, RX PATH {}, RX NSS {}, SUBMITTED {}", i.thermal, i.rx_path, i.rx_nss, i.submitted);
                mind::println!("[RTW89] LISTENED {} MS ON CHANNEL {}, RECEIVE ONLY: {} FRAMES, {} BEACONS, {} PROBE RESPONSES, {} BSSIDS",
                    listened.elapsed_us / 1_000, listened.channel, t.frames, t.beacons, t.probe_responses, listened.heard);
                mind::println!("[RTW89] ALSO {} OTHER FRAMES, {} REJECTED, {} UNLISTED, {} OTHER PACKETS, {} BTC EVENTS, {} PPDU REPORTS",
                    t.other_frames, t.rejected, t.unlisted, t.other_packets, t.btc_events, listened.counts.ppdu_reports);
                mind::println!("[RTW89] CALIBRATION, PACKET TX AND WLAN PENDING; SHUTTING DOWN");
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

// 213-KRN-0123: init lends an endpoint in SLOT_SERVICE when wlan may drive this radio.
fn serving() -> bool { cap_info(SLOT_SERVICE).0 == CAP_KIND_ENDPOINT }

// The agreed lines of 213-KRN-0123's check, and what a country's limits left.
fn served(event: receive_service::Event) {
    match event {
        receive_service::Event::Serving { channel } => {
            mind::println!("[RTW89] RECEIVE-ONLY CHANNEL {}: SCHEDULER ZERO, DATA DMA STOPPED", channel);
            mind::println!("[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED");
        }
        receive_service::Event::CountryTaken { code: [b'0', b'0'], channels, listen_only, applied: true } =>
            mind::println!("[RTW89] COUNTRY 00: {} CHANNELS, {} LISTEN ONLY; APPLIED: A CHANNEL IS CALIBRATED WHEN THE STATION FIRST SENDS THERE; COUNTRY UNKNOWN: POWER CAPPED AT 75 % (801-DRV-0071)",
                channels, listen_only),
        receive_service::Event::CountryTaken { code, channels, listen_only, applied: true } =>
            mind::println!("[RTW89] COUNTRY {}{}: {} CHANNELS, {} LISTEN ONLY; APPLIED: A CHANNEL IS CALIBRATED WHEN THE STATION FIRST SENDS THERE",
                code[0] as char, code[1] as char, channels, listen_only),
        receive_service::Event::CountryTaken { code, channels, listen_only, applied: false } =>
            mind::println!("[RTW89] COUNTRY {}{}: {} CHANNELS, {} LISTEN ONLY; NOT APPLIED: THE PLATFORM'S POWER POLICY IS UNKNOWN, RECEIVE ONLY",
                code[0] as char, code[1] as char, channels, listen_only),
        receive_service::Event::NoCountry { channels, listen_only } =>
            mind::println!("[RTW89] NO COUNTRY FROM THE STATION BEFORE ITS FIRST SEND: THE WORLD RULES (00), {} CHANNELS, {} LISTEN ONLY; POWER CAPPED AT 75 % OF THE MAXIMUM (801-DRV-0071)",
                channels, listen_only),
        receive_service::Event::Capped { channel, maximum, limit } =>
            mind::println!("[RTW89] COUNTRY 00, UNKNOWN: POWER CAPPED AT 75 % OF THE MAXIMUM ON CHANNEL {} (-1.25 DB ROUNDED DOWN TO THE MAC'S HALF-DB STEP): 1 TX AT MOST {} HALF-DBM OF ITS {}, 2 TX {} OF {}",
                channel, limit[0], maximum[0], limit[1], maximum[1]),
        receive_service::Event::Stalled { channel, quiet_ms, frames, sent, acked } =>
            mind::println!("[RTW89] RECEIVE STALLED ON CHANNEL {}: NO FRAME FOR {} MS ({} BEFORE; {} SENT, {} ACKNOWLEDGED SINCE THE LAST): THE DATA MODE STOPS (801-DRV-0071)",
                channel, quiet_ms, frames, sent, acked),
        receive_service::Event::Unheard { channel, count, ms, heard_ms } =>
            mind::println!("[RTW89] {} UNICAST SENDS IN A ROW NOT ACKNOWLEDGED ON CHANNEL {} OVER {} MS; THE LAST FRAME HEARD {} MS AGO (801-DRV-0071)",
                count, channel, ms, heard_ms),
        receive_service::Event::Admitted { channel, registers } => {
            mind::println!("[RTW89] DATAPATH ADMITTED ON CHANNEL {}", channel);
            mind::println!("[RTW89] REGISTERS AT ADMISSION: {:?}", registers);
        }
        receive_service::Event::Scanned { tunes, longest_us, kept, dropped, measured, late, unmatched } =>
            mind::println!("[RTW89] SCAN: {} PASSIVE TUNES, THE LONGEST {} US; {} FRAMES KEPT, {} DROPPED; SIGNAL ON {} OF THEM, {} REPORTS AFTER THE STATION TOOK THE FRAME, {} UNMATCHED (213-DRV-0087)",
                tunes, longest_us, kept, dropped, measured, late, unmatched),
        receive_service::Event::Recovered { count, ser, cmac, bb_us, restored, waited_us, after_reconcile } =>
            mind::println!("[RTW89] FIRMWARE RECOVERY {} {} (SER_DBG_INFO {:08X}, CMAC_ERR_ISR {:08X}): BB BACK AFTER {} US, THE SCHEDULER {} AFTER {} US; DEBUG MODE GOES ON",
                count, if after_reconcile { "AFTER A COEXISTENCE RECONCILE" } else { "BETWEEN OPERATIONS" }, ser, cmac, bb_us,
                if restored { "RESTORED BY THE DRIVER" } else { "BACK FROM THE FIRMWARE" }, waited_us),
        receive_service::Event::LateRelease { ms, attempts, acked } =>
            mind::println!("[RTW89] THE LATE FRAME WAS RELEASED {} MS AFTER ITS SUBMISSION ({} TRIES, {})", ms, attempts, if acked { "ACKNOWLEDGED" } else { "NOT ACKNOWLEDGED" }),
        receive_service::Event::LateFrame { count, finished, fetched, released, length, age_us } =>
            mind::println!("[RTW89] A FRAME OUTRAN ITS TIME ({}): {} BYTES, {} SUBMISSIONS DONE, THE NEXT {} AND {} AFTER {} US; ANSWERED NOT ACKNOWLEDGED, ITS RELEASE AWAITED 5 S MORE (DEBUG)",
                count, length, finished, if fetched { "FETCHED" } else { "NOT FETCHED" }, if released { "RELEASED" } else { "NOT RELEASED" }, age_us),
        receive_service::Event::Heard5 { channel, bssid: b } =>
            mind::println!("[RTW89] HEARD {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} ON 5 GHZ CHANNEL {} (213-DRV-0087)",
                b[0], b[1], b[2], b[3], b[4], b[5], channel),
        receive_service::Event::Signals { channel, kept, measured, late, unmatched } =>
            mind::println!("[RTW89] SIGNAL ON CHANNEL {} SINCE THE ADMISSION: ON {} OF {} FRAMES KEPT; {} REPORTS AFTER THE FRAME WAS HANDED ON, {} UNMATCHED (213-DRV-0087)",
                channel, measured, kept, late, unmatched),
        receive_service::Event::Traffic { channel, wifi, kept, filtered, invalid, pressure, ppdu, beacons, bssid: b, other_beacons, refused, sent, acked, late, holes, longest_ms, probe, errors: e, polls: [q, p], tx, floor } => {
            if let Some(r) = refused { for (n, (length, head)) in r.frames[..r.count].iter().enumerate() {
                mind::println!("[RTW89] REFUSED FRAME {} OF THE FIRST {}: {} BYTES, {:02X?}", n + 1, r.count, length, head);
            } }
            mind::println!("[RTW89] TRAFFIC ON {}: {} FRAMES, {} KEPT, {} FILTERED, {} INVALID, {} INBOX FULL; {} PPDU, {} BEACONS; {} SENT, {} ACKED, {} LATE; {} HOLES, LONGEST {} MS",
                channel, wifi, kept, filtered, invalid, pressure, ppdu, beacons, sent, acked, late, holes, longest_ms);
            mind::println!("[RTW89] TRAFFIC ON {}: THE BEACONS ABOVE ARE {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}'S; {} BEACONS OF OTHERS FILTERED OUT (213-DRV-0090)",
                channel, b[0], b[1], b[2], b[3], b[4], b[5], other_beacons);
            mind::println!("[RTW89] TRAFFIC ON {}: THE FIRMWARE'S ERROR CODES {} L0, {} L1, {} L2, {} UNKNOWN; HISR0 HALT_C2H {}, WDT_TIMEOUT {} (213-DRV-0091)",
                channel, e.l0, e.l1, e.l2, e.unknown, e.hisr_halt, e.hisr_wdt);
            mind::println!("[RTW89] TRAFFIC ON {}: RX POLLS: RXQ LONGEST GAP {} US, MOST WAITING {}; RPQ LONGEST GAP {} US, MOST WAITING {}; OF {} (213-DRV-0091)",
                channel, q.longest_gap_us, q.most_waiting, p.longest_gap_us, p.most_waiting, rx::DESCRIPTORS);
            mind::println!("[RTW89] TRAFFIC ON {}: LATE FRAMES RELEASED {}: {} ACKNOWLEDGED AT THE FIRST TRY, {} NEVER; THEIR MEDIAN HOLD {} MS; THE FRAME BUFFER FULL {} TIMES, THE LONGEST {} MS (213-DRV-0091)",
                channel, tx.late, tx.first_try, tx.not_acked, tx.median_hold_ms, tx.full_times, tx.full_longest_ms);
            // 213-DRV-0104: the minute's sends answered late, refused busy and parked, and what this minute held.
            // 213-DRV-0104: how long sends took to their answer when in time, by size, and how long the next took to come.
            for (class, bins) in ["UNDER 200 B", "200 TO 1000 B", "OVER 1000 B"].iter().zip(tx.answer_ms.iter()) {
                mind::println!("[RTW89] TRAFFIC ON {}: SENDS ANSWERED IN TIME BY MS (2 TO 30, 10 TO 90), {}: {} (213-DRV-0104)", channel, class, Spaced(bins));
            }
            mind::println!("[RTW89] TRAFFIC ON {}: FROM AN ANSWER TO THE NEXT SEND BY MS (2 TO 30, 10 TO 90, MORE): {}; PEAK {} SENDS IN A SECOND (213-DRV-0104)",
                channel, Spaced(&tx.gap_ms), tx.peak_sends);
            // 213-DRV-0104: sends to one station by the rate they went at and by tries, and what the access point sent us.
            const TX_RATES: [&str; 12] = ["1M", "2M", "5.5M", "11M", "6M", "9M", "12M", "18M", "24M", "36M", "48M", "54M"];
            mind::println!("[RTW89] TRAFFIC ON {}: SENT TO ONE STATION BY RATE, SENDS/ACKED/TRIES, 1 TO 9 MB/S:{} (213-DRV-0104)", channel, ByRate(&TX_RATES[..6], &tx.by_rate[..6]));
            mind::println!("[RTW89] TRAFFIC ON {}: SENT TO ONE STATION BY RATE, SENDS/ACKED/TRIES, 12 TO 54 MB/S:{} (213-DRV-0104)", channel, ByRate(&TX_RATES[6..], &tx.by_rate[6..]));
            mind::println!("[RTW89] TRAFFIC ON {}: SENT TO ONE STATION BY TRIES, 1 TO 15: {} (213-DRV-0104)", channel, Spaced(&tx.by_tries[1..]));
            let r = tx.rx_rate;
            mind::println!("[RTW89] TRAFFIC ON {}: RECEIVED FOR US {}, {} WITH THE RETRY BIT; BY LEGACY RATE:{} (213-DRV-0104)",
                channel, r.iter().sum::<u32>(), tx.rx_retry, Counted(&TX_RATES_RX, &r[..12]));
            mind::println!("[RTW89] TRAFFIC ON {}: RECEIVED FOR US BY HT RATE:{}; OTHER {} (213-DRV-0104)", channel, Counted(&MCS, &r[12..28]), r[28]);
            let behind = tx.behind_label();
            mind::println!("[RTW89] TRAFFIC ON {}: SENDS {} ANSWERED LATE, {} REFUSED BUSY, {} PARKED; PARKED RELEASED {}, {} ACKNOWLEDGED; {} (213-DRV-0104)",
                channel, tx.answered_late, tx.busy, tx.parked, tx.parked_released, tx.parked_acked, behind);
            mind::println!("[RTW89] TRAFFIC ON {}: RELEASES BY THE BEACON INTERVAL'S PHASE, 10 MS BINS: BY OUR CLOCK {:?}; BY THE TSF {:?} (213-DRV-0091)",
                channel, tx.clock, tx.tsf);
            let [pd, cck, limit, edcca] = floor.registers;
            let held = if floor.on() { "THIS MINUTE FLOOR ON" } else if floor.off() { "THIS MINUTE FLOOR OFF" }
                else if floor.set { "SET AS LINUX" } else { "AS FOUND" };
            mind::println!("[RTW89] TRAFFIC ON {}: DETECTION FLOOR 4860={:08X} 4B64={:08X} 4B74={:08X} 4884={:08X}; THE AP AT {:?} DBM OVER {} BEACONS; {}{} (213-DRV-0102)",
                channel, pd, cck, limit, edcca, floor.bss_dbm, floor.measured, held,
                if floor.switch_failures != 0 { "; A WRITE FAULTED" } else { "" });
            // Every field is written; one not read back is named, apart from the switch (213-DRV-0102, boot 93).
            let w = floor.written;
            if w.unread != 0 {
                let names = |m: u8| [(1, " 4860"), (2, " 4B64"), (4, " 4B74"), (8, " 4884")].map(|(b, n)| if m & b != 0 { n } else { "" });
                let (same, other) = (names(w.unchanged), names(w.unread & !w.unchanged));
                mind::println!("[RTW89] TRAFFIC ON {}: DETECTION FLOOR: EVERY FIELD WRITTEN; READ AS BEFORE THE WRITE:{}{}{}{}{}; READ OTHERWISE:{}{}{}{}{} (213-DRV-0102)",
                    channel, same[0], same[1], same[2], same[3], if w.unchanged == 0 { " NONE" } else { "" },
                    other[0], other[1], other[2], other[3], if w.unread & !w.unchanged == 0 { " NONE" } else { "" });
            }
            let r = floor.ppdu_rssi;
            mind::println!("[RTW89] TRAFFIC ON {}: PPDU BY SIGNAL, 5 DB BINS FROM -20 DBM: {:?} (213-DRV-0102)", channel, &r[..9]);
            let n = floor.no_rpl;
            mind::println!("[RTW89] TRAFFIC ON {}: PPDU BY SIGNAL, ON FROM -65 DBM: {:?}, THE LAST BELOW -100; UNPARSED {}; NO RPL {} (213-DRV-0102)",
                channel, &r[9..], floor.unparsed, n.iter().sum::<u32>());
            mind::println!("[RTW89] TRAFFIC ON {}: NO RPL BY THE HEADER, 5 DB BINS FROM -20 DBM: {:?} (213-DRV-0102)", channel, &n[..9]);
            mind::println!("[RTW89] TRAFFIC ON {}: NO RPL BY THE HEADER, ON FROM -65 DBM: {:?}, THE LAST BELOW -100 (213-DRV-0102)", channel, &n[9..]);
            let (sum, count) = floor.header_over;
            let tenths = if count == 0 { 0 } else { (sum * 5 + if sum < 0 { -(i64::from(count) / 2) } else { i64::from(count) / 2 }) / i64::from(count) };
            mind::println!("[RTW89] TRAFFIC ON {}: OFDM TO US: THE HEADER'S AVERAGE {}{}.{} DB OVER THE RPL, ON {} REPORTS (213-DRV-0102)",
                channel, if tenths < 0 { "-" } else { "" }, tenths.abs() / 10, tenths.abs() % 10, count);
            let ago = |at: u64| (mind::time::monotonic_ns() / 1000).saturating_sub(at) / 1000;
            mind::println!("[RTW89] TRAFFIC ON {}: RX POLL GAPS OVER 5 / 10 MS: RXQ {} / {}, RPQ {} / {}; THE LONGEST ENDED {} / {} MS AGO (213-DRV-0091)",
                channel, q.over_5ms, q.over_10ms, p.over_5ms, p.over_10ms, ago(q.longest_at_us), ago(p.longest_at_us));
            if let Some(probe) = probe { pieces("GOOD LINK", &probe); }
        }
        receive_service::Event::Hole { n, channel, bssid: b, by_beacons: true, ms, wifi, kept, ppdu, pressure, sent, acked, pending, probe } => {
            mind::println!("[RTW89] HOLE {} ON {}: NO BEACON OF {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} FOR {} MS; SINCE IT {} FRAMES, {} KEPT, {} PPDU, {} INBOX FULL; {} SENT, {} ACKED{}",
                n, channel, b[0], b[1], b[2], b[3], b[4], b[5], ms, wifi, kept, ppdu, pressure, sent, acked, if pending { ", ONE PENDING" } else { "" });
            if let Some(probe) = probe { pieces("HOLE START", &probe); }
        }
        receive_service::Event::Hole { n, channel, by_beacons: false, ms, kept, ppdu, pressure, sent, acked, pending, probe, .. } => {
            mind::println!("[RTW89] HOLE {} ON {}: NO WI-FI FRAME AT ALL FOR {} MS (NO BEACON OF THE BSS REACHES THE HOST); SINCE IT {} KEPT, {} PPDU, {} INBOX FULL; {} SENT, {} ACKED{}",
                n, channel, ms, kept, ppdu, pressure, sent, acked, if pending { ", ONE PENDING" } else { "" });
            if let Some(probe) = probe { pieces("HOLE START", &probe); }
        }
        receive_service::Event::HoleEnded { n, ms, wifi, kept, ppdu, pressure, sent, acked, probe } => {
            mind::println!("[RTW89] HOLE {} ENDED: ITS BEACON AFTER {} MS; MEANWHILE {} FRAMES, {} KEPT, {} PPDU, {} INBOX FULL; {} SENT, {} ACKED (213-DRV-0090)",
                n, ms, wifi, kept, ppdu, pressure, sent, acked);
            if let Some(probe) = probe { pieces("HOLE END", &probe); }
        }
        receive_service::Event::Linked { channel, bssid: b, aid, was, before, after } => {
            match aid {
                Some(aid) => mind::println!("[RTW89] THE FIRMWARE TOLD: CONNECTED TO {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} ON CHANNEL {} AS AID {} (JOIN INFO AND ADDRESS CAM AS INFRA; 213-DRV-0091)",
                    b[0], b[1], b[2], b[3], b[4], b[5], channel, aid),
                None => mind::println!("[RTW89] THE FIRMWARE TOLD: NO LINK ON CHANNEL {} (WAS AID {}; 213-DRV-0091)", channel, was.unwrap_or(0)),
            }
            if let Some(probe) = before { pieces("LINK BEFORE", &probe); }
            if let Some(probe) = after { pieces("LINK AFTER", &probe); }
        }
        receive_service::Event::Port { channel, linked, after } => {
            mind::println!("[RTW89] PORT 0 SET FOR {} ON CHANNEL {} (NET TYPE, BSSID FIT, TSF UPDATE, TBTT PROHIBIT; 213-DRV-0091)",
                if linked { "INFRA" } else { "NO LINK" }, channel);
            if let Some(probe) = after { pieces("PORT AFTER", &probe); }
        }
        receive_service::Event::ListenHeard { channel, bssid: b, tune_ms, send_ms } =>
            mind::println!("[RTW89] LISTEN-ONLY CHANNEL {}: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} HEARD {} MS AFTER THE TUNE, {} MS AFTER THE REFUSED SEND (213-DRV-0087)",
                channel, b[0], b[1], b[2], b[3], b[4], b[5], tune_ms, send_ms),
        receive_service::Event::Held { n, held, ms, submitted, at, registers } => {
            let ms_of = |us: Option<u64>| us.map(|us| us / 1000);
            let (sc, st, si, ac, at_tsf) = (ms_of(submitted.clock_us), ms_of(submitted.tsf_us), submitted.interval_us / 1000, ms_of(at.clock_us), ms_of(at.tsf_us));
            if held {
                mind::println!("[RTW89] HELD FRAME (LATE {}): {} MS SINCE ITS SUBMISSION; SUBMITTED {:?} / {:?} MS INTO THE BEACON INTERVAL OF {} MS (OUR CLOCK / TSF), NOW {:?} / {:?} (213-DRV-0091)",
                    n, ms, sc, st, si, ac, at_tsf);
            } else {
                mind::println!("[RTW89] A FRAME RELEASED IN TIME, BESIDE THE HELD ONES: {} MS AFTER ITS SUBMISSION; SUBMITTED {:?} / {:?} MS INTO THE BEACON INTERVAL OF {} MS, RELEASED {:?} / {:?} (213-DRV-0091)",
                    ms, sc, st, si, ac, at_tsf);
            }
            // In pieces: a log record holds 200 bytes, and the whole set is about 430 (the Ryzen's boot 84 cut it).
            pieces(if held { "HELD FRAME'S REGISTERS" } else { "IN-TIME FRAME'S REGISTERS" }, &registers);
        }
        receive_service::Event::Calibrated { channel, dpk_gain_scale, paths } => for (path, c) in paths.iter().enumerate() {
            mind::println!("[RTW89] CALIBRATION ON {} PATH {}: IQK LOK IDAC {:05X} VBUF {:05X}{}, ONE-SHOT FAILS LOK {} TX {} RX {}{}, TX {:08X} RX {:08X}: {} BY LINUX'S CHECKS (213-DRV-0091)",
                channel, path, c.lok_idac, c.lok_vbuf, if c.lok_range_fail { " OUT OF RANGE" } else { "" }, c.lok_fails, c.tx_fails, c.rx_fails,
                if c.restore_fail { ", RESTORE FAILED" } else { "" }, c.iqk_tx, c.iqk_rx, if c.iqk_ok() { "PASS" } else { "FAIL" });
            mind::println!("[RTW89] CALIBRATION ON {} PATH {}: DPK TXAGC {:02X} THERMAL {} CORRELATION {} (LINUX: AT LEAST 170) DC {}/{} (AT MOST 200), GAIN SCALE {}: {}; TSSI ALIGNMENT {:08X?} REPORTS {:?} (213-DRV-0091)",
                channel, path, c.dpk_txagc, c.dpk_thermal, c.dpk_correlation, c.dpk_dc[0], c.dpk_dc[1], dpk_gain_scale,
                if c.dpk_ok() { "PASS" } else { "FAIL" }, c.tssi_alignment, c.tssi_reports);
        },
        receive_service::Event::FloorSet { channel, dbm, measured, ofdm_pd, cck_pd, edcca, written, registers: [pd, cck, limit, level] } =>
            mind::println!("[RTW89] DETECTION FLOOR ON {} AS LINUX FOR THE AP AT {} DBM ({} BEACONS): OFDM PD {} (-{} DBM), CCK PD {:02X} ({} DBM), EDCCA {} ({} DBM){}; 4860={:08X} 4B64={:08X} 4B74={:08X} 4884={:08X} (213-DRV-0102)",
                channel, dbm, measured, ofdm_pd, 102 - 2 * i16::from(ofdm_pd), cck_pd, cck_pd as i8, edcca, i16::from(edcca) - 128,
                if written { "" } else { ", NOT AS WRITTEN" }, pd, cck, limit, level),
        receive_service::Event::FloorAlternates { channel, dbm, measured, ofdm_pd, cck_pd, edcca, found: [pd, cck, limit, level] } =>
            mind::println!("[RTW89] DETECTION FLOOR ON {} TO ALTERNATE BY THE MINUTE, THE NEXT ONE OFF: LINUX'S OFDM PD {}, CCK PD {:02X}, EDCCA {} FOR THE AP AT {} DBM ({} BEACONS); FOUND 4860={:08X} 4B64={:08X} 4B74={:08X} 4884={:08X} (213-DRV-0102)",
                channel, ofdm_pd, cck_pd, edcca, dbm, measured, pd, cck, limit, level),
        receive_service::Event::FloorRefused { channel } =>
            mind::println!("[RTW89] DETECTION FLOOR NOT SET ON CHANNEL {}: ITS VALUES ARE KNOWN FOR 2.4 GHZ 20 MHZ ONLY (213-DRV-0102)", channel),
        receive_service::Event::SerStep { count, from, to, since_us, polls: [q, p] } =>
            mind::println!("[RTW89] SER_DBG_INFO STEP {}: {:08X} -> {:08X}, SEEN {} US AFTER THE CHECK BEFORE; THIS MINUTE'S RX POLLS SO FAR: RXQ LONGEST GAP {} US, MOST WAITING {}; RPQ LONGEST GAP {} US, MOST WAITING {} (213-DRV-0091)",
                count, from, to, since_us, q.longest_gap_us, q.most_waiting, p.longest_gap_us, p.most_waiting),
        receive_service::Event::HaltStatus { count, hisr, ctrl, ser, intx } =>
            mind::println!("[RTW89] HISR0 HALT BITS {}: {:08X} ({}{}) WITH HALT_C2H_CTRL {:08X}; SER_DBG_INFO {:08X}; PCI INTX {}; CLEARED AS LINUX DOES, NOTHING ANSWERED (213-DRV-0091)",
                count, hisr, if hisr & (1 << 21) != 0 { "HALT_C2H " } else { "" }, if hisr & (1 << 22) != 0 { "WDT_TIMEOUT" } else { "" }, ctrl, ser,
                match intx { Some(true) => "ASSERTED", Some(false) => "NOT ASSERTED", None => "UNREAD" }),
        receive_service::Event::FirmwareError { count, ctrl, raw, ser, code, name, level, event: ser_event } =>
            mind::println!("[RTW89] THE FIRMWARE'S ERROR CODE {}: HALT_C2H {:08X} (CTRL {:08X}) = MAC_AX_ERR_{} {:04X}, {:?} ({}); SER_DBG_INFO {:08X}; NOT ANSWERED (213-DRV-0091)",
                count, raw, ctrl, name, code, level, ser_event, ser),
        receive_service::Event::Unreadable { count, register, value, reads, waited_us, ser_before, ser_after, recovered } =>
            mind::println!("[RTW89] CHECK MET {} {}: {:X} READ {:08X}, READABLE AGAIN AFTER {} READS IN {} US; SER_DBG_INFO {:08X} -> {:08X} (213-DRV-0091)",
                if recovered { "A FIRMWARE RECOVERY" } else { "AN UNREADABLE CMAC" }, count, register, value, reads, waited_us, ser_before, ser_after),
        receive_service::Event::ListenOnly { channel, receiver: r, heard } =>
            mind::println!("[RTW89] LISTEN-ONLY CHANNEL {}: NOTHING SENT TO {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, NO BEACON OF IT HEARD HERE SINCE THE TUNE ({} ACCESS POINTS HEARD; 213-DRV-0087)",
                channel, r[0], r[1], r[2], r[3], r[4], r[5], heard),
        receive_service::Event::DataCountry { code: [b'0', b'0'], channel, kept: true, .. } =>
            mind::println!("[RTW89] COUNTRY 00 IN DATA MODE: CHANNEL {} KEPT AT THE POWER IT WAS ADMITTED AT (AN UNKNOWN COUNTRY'S 75 % CAP IS SET AT ADMISSION)", channel),
        receive_service::Event::DataCountry { code, channel, kept: true, .. } =>
            mind::println!("[RTW89] COUNTRY {}{} IN DATA MODE: CHANNEL {} KEPT", code[0] as char, code[1] as char, channel),
        receive_service::Event::DataCountry { code, channel, kept: false, enforced: true } =>
            mind::println!("[RTW89] COUNTRY {}{} IN DATA MODE: CHANNEL {} NOT ALLOWED AT ITS ADMITTED POWER; NOTHING SENT UNTIL A COUNTRY ALLOWS IT",
                code[0] as char, code[1] as char, channel),
        receive_service::Event::DataCountry { code, channel, kept: false, enforced: false } =>
            mind::println!("[RTW89] COUNTRY {}{} IN DATA MODE: CHANNEL {} NOT ALLOWED AT ITS ADMITTED POWER: SENDING SHOULD STOP; DEBUG MODE KEEPS SENDING",
                code[0] as char, code[1] as char, channel),
    }
}

// The system log keeps lines of mind::log::LINE bytes: a long value goes out in numbered pieces of a line each.
struct Pieces { label: &'static str, bytes: [u8; 150], len: usize, count: u32 }
impl Pieces {
    fn flush(&mut self) {
        if self.len == 0 { return }
        let text = core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("?");
        mind::println!("[RTW89] {} {}: {}", self.label, self.count, text);
        self.count += 1; self.len = 0;
    }
}
impl core::fmt::Write for Pieces {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for &byte in text.as_bytes() { if self.len == self.bytes.len() { self.flush(); } self.bytes[self.len] = byte; self.len += 1; }
        Ok(())
    }
}
fn pieces(label: &'static str, value: &dyn core::fmt::Debug) {
    use core::fmt::Write;
    let mut out = Pieces { label, bytes: [0; 150], len: 0, count: 1 };
    let _ = write!(out, "{:?}", value); out.flush();
}

// Printable ASCII only; other SSID bytes show as '?'.
fn heard(found: &listen::Heard) {
    let (ssid, b) = (found.ssid(), found.bssid);
    let mut text = [b'?'; 32];
    for (out, byte) in text.iter_mut().zip(ssid) { if (0x20..0x7f).contains(byte) { *out = *byte; } }
    let name = core::str::from_utf8(&text[..ssid.len()]).unwrap_or("");
    mind::println!("[RTW89] HEARD {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} DS {} SSID {} BYTES \"{}\"",
        b[0], b[1], b[2], b[3], b[4], b[5], found.ds_channel, ssid.len(), name);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::println!("[RTW89] FIRMWARE/MAC/RX/INITIAL PHY, THEN WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED AND A CHANNEL CALIBRATED");
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
    let Some((index, platform)) = args::parse(mind::process::args_str()) else { mind::println!("[RTW89] NO PCI DEVICE INDEX"); idle(); };
    // 213-KRN-0122's word: Unknown, or none, is held back (801 S5): the country's limits alone, as with no platform limit.
    match platform {
        args::Platform::Absent => mind::println!("[RTW89] PLATFORM POWER POLICY FROM INIT: ABSENT"),
        args::Platform::Limits { per_ntx, per_path } => mind::println!("[RTW89] PLATFORM POWER POLICY FROM INIT: LIMITS NTX {:?}, PATH {:?} HALF-DBM", per_ntx, per_path),
        args::Platform::Unknown(reason) => mind::println!("[RTW89] PLATFORM POWER POLICY FROM INIT: UNKNOWN ({}): HELD BACK, NOT ENFORCED: SENDING UNDER THE COUNTRY'S LIMITS ALONE (801)",
            reason.unwrap_or("none given")),
    }
    let bytes = match firmware_file() {
        Ok(bytes) => bytes,
        Err(error) => { mind::println!("[RTW89] PRIVATE FILE {} UNAVAILABLE/REJECTED: {:?}; NO DEVICE START", FIRMWARE_PATH, error); idle(); }
    };
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { mind::println!("[RTW89] BAR2 MAP FAILED"); idle(); };
    if bar.len() != len { mind::println!("[RTW89] BAR2 SIZE CHANGED"); idle(); }
    let mut native = Native { bar };
    match board::power_off_left_on(&mut native) {
        Ok(true) => mind::println!("[RTW89] THE MAC WAS LEFT ON (A REPLACED OR STOPPED INSTANCE): POWERED OFF FIRST, AS LINUX DOES"),
        Ok(false) => {}
        Err(cause) => { mind::println!("[RTW89] THE MAC WAS LEFT ON AND DID NOT POWER OFF: {:?}; NO NETWORK LINK", cause); idle(); }
    }
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
        dma: None, queues: None, prepared: None, ready: None, runtime: None, capabilities: None, receiving: None, radio_attempted: false, platform };
    let report = startup::run(&mut attempt);
    pieces("DIAGNOSTIC", &report);
    let clean = [&report.stop, &report.power_off, &report.quiesce].iter().all(|step| matches!(step, Some(Ok(()))));
    if RESTART_AFTER_FAILURE && report.failure.is_some() && clean {
        mind::println!("[RTW89] THE DEVICE STOPPED CLEANLY: EXITING, SO THAT INIT STARTS A FRESH DRIVER (DEBUG)");
        mind::process::exit_with(1)
    }
    mind::println!("[RTW89] DMA RETAINED={}; NO RETRY, CALIBRATED RF, WLAN ENDPOINT OR NETWORK LINK", attempt.dma.is_some());
    if serving() { receive_service::absent_forever(); }
    idle()
}

// DEBUG (the maintainer, 2026-11-08: everything runs first, hardening later): after a failure that stopped, powered off
// and quiesced the device, the driver exits; init restarts it within its budget, the device quiesced and the DMA region
// cleared. False: it serves `absent` for good.
const RESTART_AFTER_FAILURE: bool = true;

fn idle() -> ! { loop { mind::time::sleep(60_000); } }

// 213-DRV-0104: numbers apart by spaces, for the minute's histograms.
struct Spaced<'a>(&'a [u32]);
impl core::fmt::Display for Spaced<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, v) in self.0.iter().enumerate() { if i > 0 { f.write_str(" ")?; } write!(f, "{}", v)?; }
        Ok(())
    }
}
// 213-DRV-0104: each rate that was used, with its sends, acknowledged and tries; NONE when none was.
struct ByRate<'a>(&'a [&'a str], &'a [[u32; 3]]);
impl core::fmt::Display for ByRate<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut any = false;
        for (name, [n, a, t]) in self.0.iter().zip(self.1) { if *n != 0 { any = true; write!(f, " {} {}/{}/{}", name, n, a, t)?; } }
        if !any { f.write_str(" NONE")?; }
        Ok(())
    }
}
// 213-DRV-0104: each label whose count is not zero, with its count; NONE when all are.
struct Counted<'a>(&'a [&'a str], &'a [u32]);
impl core::fmt::Display for Counted<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut any = false;
        for (name, n) in self.0.iter().zip(self.1) { if *n != 0 { any = true; write!(f, " {} {}", name, n)?; } }
        if !any { f.write_str(" NONE")?; }
        Ok(())
    }
}
const TX_RATES_RX: [&str; 12] = ["1M", "2M", "5.5M", "11M", "6M", "9M", "12M", "18M", "24M", "36M", "48M", "54M"];
const MCS: [&str; 16] = ["MCS0", "MCS1", "MCS2", "MCS3", "MCS4", "MCS5", "MCS6", "MCS7", "MCS8", "MCS9", "MCS10", "MCS11", "MCS12", "MCS13", "MCS14", "MCS15"];
