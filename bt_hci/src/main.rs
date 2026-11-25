#![no_std]
#![no_main]

//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. It claims the controller usb_host gives its badge
//! (`BADGE_BLUETOOTH`, 700-DRV-0043), resets it, patches a Realtek RTL8821C or RTL8852B (700-BLT-0004) from
//! data/firmware/rtl_bt/, says its version and address, then runs the host stack (bt_host, linked in for the testing
//! stage, 700-BLT-0005 and on): events from the interrupt endpoint, ACL data both ways on the bulk pair, commands as
//! class requests. The stack pages, pairs and serves only the devices `data/bluetooth/devices.txt` names, or a list
//! delivered to `data/inbox/bt-devices.txt`; without one the radio stays quiet but for the listening rounds
//! `data/bt-listen.txt` (or `data/inbox/bt-listen.txt`) asks for (700-BLT-0003): a controller that shares its radio
//! with the machine's Wi-Fi (the Ryzen's RTL8852BE) left the Wi-Fi deaf after its inquiries.

use bt_hci::{hci, rtl};
use bt_host::acl;
use bt_host::stack::{Stack, Up};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, CAP_KIND_INPUT, SLOT_DEV0, SLOT_FILE, SLOT_PRIV, SLOT_VFS};
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
/// The RTL8852B's patch, as scripts/firmware/rtl8852b_bt.py fetches it; it has no configuration file (700-BLT-0004).
const PATCH_8852B: (&str, usize, [u8; 32]) = ("data/firmware/rtl_bt/rtl8852bu_fw.bin", 127_301, [
    0xfa, 0x1a, 0x98, 0xb9, 0x62, 0x3e, 0xe1, 0xef, 0x90, 0x75, 0x41, 0x42, 0x9a, 0x48, 0x6c, 0x52,
    0xca, 0x12, 0x40, 0x62, 0xa6, 0x46, 0x55, 0x00, 0x55, 0x74, 0x95, 0xa6, 0xb3, 0xf1, 0x4c, 0xba]);
/// The largest patch image: the RTL8852B's for ROM version 0 is 67,133 bytes.
const IMAGE_MAX: usize = 128 * 1024;
/// How long a controller may take to enumerate, and a command to be answered.
const CLAIM_MS: u64 = 30_000;
const COMMAND_MS: u64 = 2_000;
/// The file that turns the listening rounds on (700-BLT-0003).
const LISTEN: &str = "data/bt-listen.txt";
/// The devices the stack may page, accept and pair, and the keys it keeps for them (bt_host::devices): the testing
/// stage's place, a directory of the boot volume's data area. They are written only through a writable client of that
/// directory, which init does not grant yet (asked of KRN); without it the keys of one boot are kept in memory alone.
const BLUETOOTH_DIR: &str = "data/bluetooth";
const DEVICES: &str = "data/bluetooth/devices.txt";
const KEYS: &str = "data/bluetooth/keys.txt";
/// The same put in Effector's delivery root, so that a bench names its devices and asks for rounds from afar.
const INBOX_DEVICES: &str = "data/inbox/bt-devices.txt";
const INBOX_LISTEN: &str = "data/inbox/bt-listen.txt";
/// Where bulk OUT data sit in the USB buffer: past the reports and control data at its start.
const BULK_OUT_AT: usize = 0x8000;
/// How often the loop runs while a link is up, and while none is.
const BUSY_MS: usize = 2;
const IDLE_MS: usize = 20;

fn say(args: core::fmt::Arguments) { mind::println!("[BT] {}", args); }
fn now() -> u64 { mind::time::uptime_ms() as u64 }

/// Why a step failed. The fields are read through Debug, in the log.
#[allow(dead_code)]
#[derive(Debug)]
enum Fail { Usb(mind::sys::Error), Timeout(u16), Status(u16, u8), File(&'static str), Patch(rtl::Error) }

/// The controller as claimed: its HCI interface, its event endpoint, the events joined, the last command's answer.
struct Hci { host: Host, handle: u32, number: u8, events_ep: u8, events: hci::Events, ret: [u8; hci::EVENT_MAX], ret_len: usize }

impl Hci {
    /// The events that came since the last call.
    fn poll(&mut self, mut each: impl FnMut(u8, &[u8])) -> Result<(), Fail> {
        let events = &mut self.events;
        self.host.reports(self.handle, self.events_ep, |packet| events.feed(packet, |code, params| each(code, params))).map_err(Fail::Usb)?;
        Ok(())
    }

    /// Sends a command and waits for its Command Complete (its return parameters in `ret`) or, for one answered by a
    /// Command Status, that status. Used for the bring-up, before the stack runs.
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

/// A Realtek controller's patch: its ROM version, the image from its files, the fragments downloaded, a reset.
fn patch(hci: &mut Hci, chip: &rtl::Chip) -> Result<(), Fail> {
    if mind::dev::cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT { return Err(Fail::File("no file client")); }
    let rom = *hci.command(rtl::READ_ROM_VERSION, &[])?.get(1).ok_or(Fail::Status(rtl::READ_ROM_VERSION, 0xFF))?;
    let mut image = Pages::new(IMAGE_MAX).ok_or(Fail::File("no memory"))?;
    let len = if *chip == rtl::RTL8852B {
        let fw = read(PATCH_8852B)?;
        let fw = &fw.as_slice()[..PATCH_8852B.1];
        let e = rtl::parse_core(fw).map_err(Fail::Patch)?;
        let len = rtl::image_core(fw, chip, rom, image.as_mut_slice()).map_err(Fail::Patch)?;
        say(format_args!("ROM VERSION {}; PATCH {:016X} FOR PROJECT {} ({} OF {} SECTIONS): {} BYTES IN {} FRAGMENTS", rom, e.version, e.project, e.found, e.counted, len, rtl::fragments(len)));
        len
    } else {
        let (fw, config) = (read(PATCH)?, read(CONFIG)?);
        let e = rtl::parse(&fw.as_slice()[..PATCH.1]).map_err(Fail::Patch)?;
        let len = rtl::image(&fw.as_slice()[..PATCH.1], &config.as_slice()[..CONFIG.1], chip, rom, image.as_mut_slice()).map_err(Fail::Patch)?;
        say(format_args!("ROM VERSION {}; PATCH {:08X} FOR PROJECT {}: {} BYTES IN {} FRAGMENTS", rom, e.version, e.project, len, rtl::fragments(len)));
        len
    };
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
    let mut hci = Hci { host, handle, number: info.number, events_ep, events: hci::Events::new(), ret: [0; hci::EVENT_MAX], ret_len: 0 };
    let up = (|| -> Result<[u8; 6], Fail> {
        // The first call starts the event endpoint's polling.
        hci.poll(|_, _| {})?;
        hci.command(hci::RESET, &[])?;
        let v = hci.version()?;
        say(format_args!("VERSION: HCI {} REVISION {:04X}, LMP {} SUBVERSION {:04X}, MANUFACTURER {:04X}", v.hci, v.hci_rev, v.lmp, v.lmp_subver, v.manufacturer));
        let unpatched = [rtl::RTL8821C, rtl::RTL8852B].into_iter().find(|c| (v.manufacturer, v.lmp_subver, v.hci_rev) == (rtl::MANUFACTURER, c.lmp_subver, c.hci_rev));
        if let Some(c) = unpatched {
            patch(&mut hci, &c)?;
            let v = hci.version()?;
            say(format_args!("{} PATCHED: LMP SUBVERSION {:04X}, HCI REVISION {:04X}", c.name, v.lmp_subver, v.hci_rev));
        } else {
            say(format_args!("NOT AN UNPATCHED RTL8821C OR RTL8852B: NO PATCH"));
        }
        let a = hci::bd_addr(hci.command(hci::READ_BD_ADDR, &[])?).ok_or(Fail::Status(hci::READ_BD_ADDR, 0xFF))?;
        say(format_args!("ADDRESS {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[0], a[1], a[2], a[3], a[4], a[5]));
        Ok(a)
    })();
    match up {
        Ok(a) => run(hci, &info, bt_host::buf::Addr(core::array::from_fn(|i| a[5 - i]))),
        Err(e) => { say(format_args!("BRING-UP STOPPED: {:?}", e)); idle() }
    }
}

/// A text file whole into `buf`, if it is there and fits.
fn read_text<'a>(path: &str, buf: &'a mut [u8]) -> Option<&'a str> {
    let n = mind::idl::wire::with_timeout(2_000, || -> Result<usize, ()> {
        let mut file = mind::fs::File::open(path).map_err(|_| ())?;
        if file.size() > buf.len() { return Err(()); }
        let mut at = 0;
        while at < file.size() { let n = file.read(&mut buf[at..]).map_err(|_| ())?; if n == 0 { break; } at += n; }
        Ok(at)
    }).ok()?;
    core::str::from_utf8(&buf[..n]).ok()
}

/// The keys file written whole; false when it could not be.
fn write_keys(stack: &Stack) -> bool {
    let mut text = [0u8; 2048];
    let n = stack.devices.keys_text(&mut text);
    mind::idl::wire::with_timeout(2_000, || -> Result<(), ()> {
        let mut file = mind::fs::File::create(KEYS).map_err(|_| ())?;
        if file.write(&text[..n]).map_err(|_| ())? != n { return Err(()); }
        Ok(())
    }).is_ok()
}

/// The host stack on the controller brought up: events and ACL data in, commands and ACL data out, its lines logged,
/// its keys kept, its HID reports delivered as usb_hid delivers a USB keyboard's and mouse's.
fn run(mut hci: Hci, info: &Interface, local: bt_host::buf::Addr) -> ! {
    let bulk_in = info.endpoints().iter().find(|e| e.is_bulk() && e.is_in()).copied();
    let bulk_out = info.endpoints().iter().find(|e| e.is_bulk() && !e.is_in()).map(|e| e.address);
    let (Some(bulk_in), Some(bulk_out)) = (bulk_in, bulk_out) else { say(format_args!("NO BULK ENDPOINTS: NO ACL DATA")); idle() };
    let listen = mind::fs::File::open(LISTEN).is_ok() || mind::fs::File::open(INBOX_LISTEN).is_ok();
    let mut stack = Stack::new();
    // LE pairing's random values: the stack's generator seeded from the processor's (without one, LE does not pair).
    stack.le.local = local;
    let mut seed = [0u8; 32];
    if mind::random::fill(&mut seed) { stack.le.rng.seed(&seed); } else { say(format_args!("NO {}: LE DEVICES ARE NOT PAIRED", mind::random::SOURCE)); }
    seed.fill(0);
    let mut text = [0u8; 4096];
    let mut lists = 0;
    let mut load = |stack: &mut Stack, path: &str, text: &mut [u8]| if let Some(t) = read_text(path, text) {
        lists += 1;
        stack.devices.load(t, |n, line| say(format_args!("{} LINE {} NOT UNDERSTOOD: {}", path, n, line)));
    };
    load(&mut stack, INBOX_DEVICES, &mut text);
    // A writable client of data/bluetooth when init grants one (asked of KRN): from here every file is in it.
    let writable = mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT;
    if writable { mind::fs::use_scope(Endpoint(SLOT_FILE), BLUETOOTH_DIR); }
    let input = mind::dev::cap_info(SLOT_PRIV).0 == CAP_KIND_INPUT;
    load(&mut stack, DEVICES, &mut text);
    let mut keys = [0u8; 4096];
    let kept = read_text(KEYS, &mut keys).map_or(0, |t| stack.devices.load_keys(t));
    let listed = stack.devices.list.iter().flatten().count();
    say(format_args!("ACL ON {:02X} AND {:02X}; {} DEVICES LISTED ({} OF {} AND {}), {} KEYS; KEYS {}; HID INPUT {}", bulk_in.address, bulk_out, listed, lists, DEVICES, INBOX_DEVICES, kept,
        if writable { "KEPT IN data/bluetooth" } else { "KEPT FOR THIS BOOT ONLY (NO WRITABLE data/bluetooth)" }, if input { "DELIVERED" } else { "NOT DELIVERED (NO INPUT PRIVILEGE)" }));
    for d in stack.devices.list.iter().flatten() { say(format_args!("DEVICE {} {}{}", d.addr, d.kind.name(), if d.key.is_some() || d.ltk.is_some() { ", PAIRED" } else { "" })); }
    stack.scan.on = listen;
    say(format_args!("UP: {}", if listen { "LISTENING EVERY 300 S (data/bt-listen.txt)" } else { "NOT LISTENING (NO data/bt-listen.txt OR data/inbox/bt-listen.txt): A SCAN TAKES THE RADIO A WI-FI MAY SHARE" }));
    stack.start(now());
    let mut stream = acl::Stream::new(bulk_in.packet.clamp(1, 64) as usize);
    let mut keyboard = mind::hid::Keyboard::new();
    let mut decoder = mind::keys::Ps2::new();
    let pointer = mind::hid::Pointer::boot();
    // A HID over GATT device's layout, from its report map.
    let mut mapped: Option<mind::hid::Pointer> = None;
    let mut out = [0u8; acl::PACKET_MAX];
    let mut line = [0u8; bt_host::buf::LINE];
    let mut up = [0u8; bt_host::stack::UP_MAX];
    let mut complained = false;
    loop {
        let t = now();
        let events = hci.poll(|code, p| stack.event(code, p));
        let data = hci.host.reports(hci.handle, bulk_in.address, |packet| stream.feed(packet, |p| stack.acl(p)));
        // Said once: usb_host gone or the controller unplugged leaves this instance without a controller.
        if let (Some(e), false) = (events.err().or(data.err().map(Fail::Usb)), complained) { complained = true; say(format_args!("USB: {:?}", e)); }
        stack.tick(t);
        while let Some(n) = stack.ctl.next_command(&mut out) {
            hci.host.buffer_mut()[..n].copy_from_slice(&out[..n]);
            if let Err(e) = hci.host.control(hci.handle, 0x20, 0, 0, hci.number as u16, n as u16) { say(format_args!("COMMAND {:02X}{:02X} NOT SENT: {:?}", out[1], out[0], e)); }
        }
        while let Some(n) = stack.ctl.next_acl(&mut out) {
            hci.host.buffer_mut()[BULK_OUT_AT..BULK_OUT_AT + n].copy_from_slice(&out[..n]);
            if let Err(e) = hci.host.bulk(hci.handle, bulk_out, BULK_OUT_AT, n) { say(format_args!("ACL PACKET NOT SENT: {:?}", e)); }
        }
        while let Some(l) = stack.ctl.log.take(&mut line) { say(format_args!("{}", l)); }
        while let Some(u) = stack.ups.take(&mut up) {
            match u {
                Up::Keys if writable => if !write_keys(&stack) { say(format_args!("{} NOT WRITTEN", KEYS)); },
                Up::Keys => {}
                Up::Keyboard(r) => keyboard.feed(r, t, &mut |byte| if let Some(e) = decoder.feed(byte) { mind::keyboard::deliver("BT", &decoder, e); }),
                Up::Pointer(r) => pointer.events(r, &mut |e| { let _ = mind::dev::input_key(e, e, false); }),
                Up::Map(m) => {
                    mapped = mind::hid::Pointer::parse(m);
                    say(format_args!("REPORT MAP OF {} BYTES: {}", m.len(), if mapped.is_some() { "A POINTER IN IT" } else { "NO POINTER IN IT" }));
                }
                // The report ID goes first where the map names IDs.
                Up::Report(id, r) => if let Some(p) = mapped.as_ref() {
                    let mut with = [0u8; 64];
                    let n = r.len().min(63);
                    let report = if p.id != 0 { with[0] = id; with[1..1 + n].copy_from_slice(&r[..n]); &with[..1 + n] } else { r };
                    p.events(report, &mut |e| { let _ = mind::dev::input_key(e, e, false); });
                },
            }
        }
        keyboard.repeat(t, &mut |byte| if let Some(e) = decoder.feed(byte) { mind::keyboard::deliver("BT", &decoder, e); });
        let _ = mind::time::sleep(if stack.links() > 0 { BUSY_MS } else { IDLE_MS });
    }
}
