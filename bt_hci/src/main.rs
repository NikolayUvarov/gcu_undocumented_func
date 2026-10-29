#![no_std]
#![no_main]

//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. It claims the controller usb_host gives its badge
//! (`BADGE_BLUETOOTH`, 700-DRV-0043), resets it, patches a Realtek RTL8821C from data/firmware/rtl_bt/, says its
//! version and address, then listens: an LE scan and a BR/EDR inquiry at start and every five minutes, each device
//! heard logged once a round. The stack (bt_host) and its devices come after.

use bt_hci::{hci, rtl};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_DEV0, SLOT_VFS};
use mind::ipc::Endpoint;
use mind::mem::Pages;
use mind::usb::{Host, Interface};

/// The controller's files, as scripts/firmware/rtl8821c_bt.py fetches them and proprietary.sh puts them on the disk.
const PATCH: (&str, usize, [u8; 32]) = ("data/firmware/rtl_bt/rtl8821c_fw.bin", 54_636, [
    0xdf, 0xe7, 0xef, 0x77, 0x2d, 0x07, 0x8e, 0xc0, 0x94, 0x90, 0xc0, 0x21, 0x90, 0xa3, 0x2f, 0xcd,
    0x40, 0xb3, 0xea, 0xf3, 0xb0, 0x40, 0x04, 0x9d, 0x89, 0x30, 0xeb, 0x80, 0xe6, 0xd5, 0x74, 0x3c]);
const CONFIG: (&str, usize, [u8; 32]) = ("data/firmware/rtl_bt/rtl8821c_config.bin", 10, [
    0x29, 0x27, 0x17, 0xb0, 0x61, 0x0c, 0xe9, 0x20, 0xf6, 0x6b, 0x73, 0x4c, 0x29, 0x9d, 0x34, 0x9e,
    0xa5, 0x9a, 0x4c, 0x44, 0x00, 0x04, 0x40, 0x78, 0xd8, 0xfc, 0x65, 0x62, 0x5f, 0x17, 0xe3, 0x82]);
/// How long a controller may take to enumerate, and a command to be answered.
const CLAIM_MS: u64 = 30_000;
const COMMAND_MS: u64 = 2_000;
const SCAN_MS: u64 = 10_000;
const ROUND_MS: u64 = 300_000;
/// The default event mask with Extended Inquiry Result (bit 46) and the LE meta event (bit 61).
const EVENT_MASK: u64 = 0x0000_1FFF_FFFF_FFFF | 1 << 46 | 1 << 61;
/// The general inquiry access code, 0x9E8B33, and 8 × 1.28 s.
const INQUIRY: [u8; 5] = [0x33, 0x8B, 0x9E, 8, 0];
const HEARD_MAX: usize = 64;

fn say(args: core::fmt::Arguments) { mind::println!("[BT] {}", args); }
fn now() -> u64 { mind::time::uptime_ms() as u64 }

/// Why a step failed. The fields are read through Debug, in the log.
#[allow(dead_code)]
#[derive(Debug)]
enum Fail { Usb(mind::sys::Error), Timeout(u16), Status(u16, u8), File(&'static str), Patch(rtl::Error) }

/// The controller as claimed: its HCI interface, its event endpoint, the events joined, the last command's answer.
struct Hci { host: Host, handle: u32, number: u8, events_ep: u8, events: hci::Events, ret: [u8; hci::EVENT_MAX], ret_len: usize, heard: Heard }

/// Devices heard in a round, each logged once.
struct Heard { seen: [([u8; 6], bool); HEARD_MAX], count: usize, le: u32, bredr: u32 }

impl Heard {
    fn note(&mut self, h: hci::Heard) {
        if self.seen[..self.count].iter().any(|&(a, le)| a == h.addr && le == h.le) { return; }
        if self.count < HEARD_MAX { self.seen[self.count] = (h.addr, h.le); self.count += 1; }
        if h.le { self.le += 1 } else { self.bredr += 1 }
        let a = h.addr;
        let mut name = [b' '; 34];
        let shown = h.name.map_or(0, |n| { let n = &n[..n.len().min(32)]; name[0] = b'"'; for (i, &c) in n.iter().enumerate() { name[1 + i] = if (0x20..0x7F).contains(&c) { c } else { b'?' }; } name[1 + n.len()] = b'"'; n.len() + 2 });
        let name = core::str::from_utf8(&name[..shown]).unwrap_or("");
        if h.le {
            say(format_args!("LE {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} RSSI {} APPEARANCE {:04X} {}", a[0], a[1], a[2], a[3], a[4], a[5], h.rssi, h.kind, name));
        } else {
            say(format_args!("BR/EDR {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} RSSI {} CLASS {:06X} {} {}", a[0], a[1], a[2], a[3], a[4], a[5], h.rssi, h.kind, hci::major_class(h.kind), name));
        }
    }
}

impl Hci {
    /// The events that came since the last call; a device heard is noted, a command's answer kept.
    fn poll(&mut self, mut each: impl FnMut(u8, &[u8])) -> Result<(), Fail> {
        let (events, heard) = (&mut self.events, &mut self.heard);
        self.host.reports(self.handle, self.events_ep, |packet| events.feed(packet, |code, params| {
            match code {
                hci::EV_LE_META if params.first() == Some(&hci::LE_ADVERTISING_REPORT) => hci::le_reports(&params[1..], |h| heard.note(h)),
                hci::EV_EXTENDED_INQUIRY_RESULT => if let Some(h) = hci::extended_inquiry(params) { heard.note(h) },
                hci::EV_INQUIRY_RESULT_RSSI => hci::inquiry_rssi(params, |h| heard.note(h)),
                _ => {}
            }
            each(code, params);
        })).map_err(Fail::Usb)?;
        Ok(())
    }

    /// Sends a command and waits for its Command Complete (its return parameters in `ret`) or, for one answered by a
    /// Command Status, that status.
    fn command(&mut self, opcode: u16, params: &[u8]) -> Result<&[u8], Fail> {
        let n = hci::command(opcode, params, self.host.buffer_mut());
        self.host.control(self.handle, 0x20, 0, 0, self.number as u16, n as u16).map_err(Fail::Usb)?;
        let deadline = now() + COMMAND_MS;
        let mut done: Option<Result<usize, Fail>> = None;
        let mut ret = [0u8; hci::EVENT_MAX];
        while done.is_none() {
            self.poll(|code, p| match code {
                hci::EV_COMMAND_COMPLETE => if let Some((op, r)) = hci::complete(p) { if op == opcode { ret[..r.len()].copy_from_slice(r); done = Some(Ok(r.len())); } },
                hci::EV_COMMAND_STATUS => if let Some((status, op)) = hci::status(p) { if op == opcode { done = Some(if status == 0 { Ok(0) } else { Err(Fail::Status(opcode, status)) }); } },
                _ => {}
            })?;
            if done.is_none() {
                if now() > deadline { return Err(Fail::Timeout(opcode)); }
                let _ = mind::time::sleep(10);
            }
        }
        let len = done.unwrap()?;
        self.ret[..len].copy_from_slice(&ret[..len]);
        self.ret_len = len;
        // A Command Complete's first return byte is its status.
        if len > 0 && self.ret[0] != 0 { return Err(Fail::Status(opcode, self.ret[0])); }
        Ok(&self.ret[..len])
    }

    fn version(&mut self) -> Result<hci::Version, Fail> {
        let ret = self.command(hci::READ_LOCAL_VERSION, &[])?;
        hci::Version::parse(ret).ok_or(Fail::Status(hci::READ_LOCAL_VERSION, 0xFF))
    }

    /// Listens for `ms`, noting the devices heard; stops early when `until` sees the event it waits for.
    fn listen(&mut self, ms: u64, mut until: impl FnMut(u8) -> bool) -> Result<(), Fail> {
        let deadline = now() + ms;
        let mut over = false;
        while !over && now() < deadline {
            self.poll(|code, _| if until(code) { over = true })?;
            let _ = mind::time::sleep(10);
        }
        Ok(())
    }

    /// One round: an active LE scan, then a general inquiry; each device heard is logged once.
    fn round(&mut self) -> Result<(), Fail> {
        (self.heard.count, self.heard.le, self.heard.bredr) = (0, 0, 0);
        // Active, every 60 ms for 30 ms, the public address, every advertiser.
        self.command(hci::LE_SET_SCAN_PARAMETERS, &[1, 0x60, 0, 0x30, 0, 0, 0])?;
        self.command(hci::LE_SET_SCAN_ENABLE, &[1, 1])?;
        self.listen(SCAN_MS, |_| false)?;
        self.command(hci::LE_SET_SCAN_ENABLE, &[0, 0])?;
        self.command(hci::INQUIRY, &INQUIRY)?;
        self.listen(SCAN_MS + 2_000, |code| code == hci::EV_INQUIRY_COMPLETE)?;
        say(format_args!("ROUND: {} LE AND {} BR/EDR DEVICES HEARD; {} EVENT PACKETS DROPPED", self.heard.le, self.heard.bredr, self.events.dropped));
        Ok(())
    }
}

/// A file read whole and checked by its size and SHA-256.
fn read((path, size, sha): (&'static str, usize, [u8; 32])) -> Result<Pages, Fail> {
    mind::idl::wire::with_timeout(2_000, || {
        let mut file = mind::fs::File::open(path).map_err(|_| Fail::File(path))?;
        if file.size() != size { return Err(Fail::File(path)); }
        let mut bytes = Pages::new(size).ok_or(Fail::File(path))?;
        let mut at = 0;
        while at < size {
            let n = file.read(&mut bytes.as_mut_slice()[at..size]).map_err(|_| Fail::File(path))?;
            if n == 0 { return Err(Fail::File(path)); }
            at += n;
        }
        if mind::sha256::digest(&bytes.as_slice()[..size]) != sha { return Err(Fail::File(path)); }
        Ok(bytes)
    })
}

/// The RTL8821C's patch: its ROM version, the image from the files, the fragments downloaded, a reset.
fn patch(hci: &mut Hci) -> Result<(), Fail> {
    if mind::dev::cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT { return Err(Fail::File("no file client")); }
    let rom = *hci.command(rtl::READ_ROM_VERSION, &[])?.get(1).ok_or(Fail::Status(rtl::READ_ROM_VERSION, 0xFF))?;
    let (fw, config) = (read(PATCH)?, read(CONFIG)?);
    let e = rtl::parse(&fw.as_slice()[..PATCH.1]).map_err(Fail::Patch)?;
    let mut image = Pages::new(64 * 1024).ok_or(Fail::File("no memory"))?;
    let len = rtl::image(&fw.as_slice()[..PATCH.1], &config.as_slice()[..CONFIG.1], &rtl::RTL8821C, rom, image.as_mut_slice()).map_err(Fail::Patch)?;
    say(format_args!("ROM VERSION {}; PATCH {:08X} FOR PROJECT {}: {} BYTES IN {} FRAGMENTS", rom, e.version, e.project, len, rtl::fragments(len)));
    let started = now();
    let mut params = [0u8; 1 + rtl::FRAGMENT];
    for i in 0..rtl::fragments(len) {
        let (index, range) = rtl::fragment(len, i);
        let n = range.len();
        params[0] = index;
        params[1..1 + n].copy_from_slice(&image.as_slice()[range]);
        hci.command(rtl::DOWNLOAD, &params[..1 + n])?;
    }
    say(format_args!("PATCH DOWNLOADED IN {} MS", now() - started));
    hci.command(hci::RESET, &[])?;
    Ok(())
}

fn idle() -> ! { loop { let _ = mind::time::sleep(60_000); } }

/// The HCI interface's endpoints: events on the interrupt IN one.
fn events_endpoint(info: &Interface) -> Option<u8> { info.endpoints().iter().find(|e| e.is_interrupt() && e.is_in()).map(|e| e.address) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    if mind::dev::cap_info(SLOT_DEV0).0 != CAP_KIND_ENDPOINT { say(format_args!("NO USB HOST CLIENT")); idle(); }
    let Ok(mut host) = Host::new(Endpoint(SLOT_DEV0)) else { say(format_args!("NO MEMORY FOR THE USB BUFFER")); idle() };
    let started = now();
    let (handle, info) = loop {
        match host.claim() {
            Ok(claimed) => break claimed,
            Err(e) if now() - started > CLAIM_MS => { say(format_args!("NO BLUETOOTH CONTROLLER IN {} S ({:?})", CLAIM_MS / 1000, e)); idle() }
            Err(_) => { let _ = mind::time::sleep(500); }
        }
    };
    let Some(events_ep) = events_endpoint(&info) else { say(format_args!("{:04X}:{:04X}: NO EVENT ENDPOINT", info.vendor, info.product)); idle() };
    say(format_args!("CONTROLLER {:04X}:{:04X}, INTERFACE {}, EVENTS ON {:02X}", info.vendor, info.product, info.number, events_ep));
    let mut hci = Hci { host, handle, number: info.number, events_ep, events: hci::Events::new(), ret: [0; hci::EVENT_MAX], ret_len: 0,
                        heard: Heard { seen: [([0; 6], false); HEARD_MAX], count: 0, le: 0, bredr: 0 } };
    let up = (|| -> Result<(), Fail> {
        // The first call starts the event endpoint's polling.
        hci.poll(|_, _| {})?;
        hci.command(hci::RESET, &[])?;
        let v = hci.version()?;
        say(format_args!("VERSION: HCI {} REVISION {:04X}, LMP {} SUBVERSION {:04X}, MANUFACTURER {:04X}", v.hci, v.hci_rev, v.lmp, v.lmp_subver, v.manufacturer));
        let c = rtl::RTL8821C;
        if (v.manufacturer, v.lmp_subver, v.hci_rev) == (rtl::MANUFACTURER, c.lmp_subver, c.hci_rev) {
            patch(&mut hci)?;
            let v = hci.version()?;
            say(format_args!("{} PATCHED: LMP SUBVERSION {:04X}, HCI REVISION {:04X}", c.name, v.lmp_subver, v.hci_rev));
        } else {
            say(format_args!("NOT AN UNPATCHED {}: NO PATCH", c.name));
        }
        let a = hci::bd_addr(hci.command(hci::READ_BD_ADDR, &[])?).ok_or(Fail::Status(hci::READ_BD_ADDR, 0xFF))?;
        say(format_args!("ADDRESS {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[0], a[1], a[2], a[3], a[4], a[5]));
        hci.command(hci::SET_EVENT_MASK, &EVENT_MASK.to_le_bytes())?;
        // Inquiry results with RSSI or extended data.
        hci.command(hci::WRITE_INQUIRY_MODE, &[2])?;
        Ok(())
    })();
    if let Err(e) = up { say(format_args!("BRING-UP STOPPED: {:?}", e)); idle(); }
    say(format_args!("UP: LISTENING EVERY {} S", ROUND_MS / 1000));
    loop {
        if let Err(e) = hci.round() { say(format_args!("ROUND FAILED: {:?}", e)); }
        let _ = mind::time::sleep(ROUND_MS as usize);
    }
}
