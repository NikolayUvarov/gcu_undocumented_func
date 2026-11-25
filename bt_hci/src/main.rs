#![no_std]
#![no_main]

//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. It claims the controller usb_host gives its badge
//! (`BADGE_BLUETOOTH`, 700-DRV-0043) and runs bt_host's driver (bt_host::driver, host-tested whole against a scripted
//! controller) through this port: the bring-up (reset, a Realtek RTL8821C or RTL8852B patched from
//! data/firmware/rtl_bt/, 700-BLT-0004, the address), then one of two paths.
//! - With no device listed and no listening asked: main's bt_hci's last two commands, then idle, no more usb_host calls
//!   (a controller that shares its radio with the machine's Wi-Fi, the Ryzen's RTL8852BE, left the Wi-Fi deaf after
//!   inquiries, 700-BLT-0003; and a usb_host kept busy on the boot stick's xHCI looks like a hung machine).
//! - Else the host stack (bt_host, linked in for the testing stage, 700-BLT-0005 and on): it pages, pairs and serves
//!   only the devices `data/bluetooth/devices.txt` or a list delivered to `data/inbox/bt-devices.txt` names, and runs
//!   the listening rounds `data/bt-listen.txt` (or `data/inbox/bt-listen.txt`) asks for.

use bt_host::driver::{self, Bring, Firmware, Next, Port, PortError};
use bt_host::stack::{Stack, Up};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, CAP_KIND_INPUT, SLOT_AUDIO, SLOT_DEV0, SLOT_FILE, SLOT_PRIV, SLOT_VFS};
use mind::ipc::Endpoint;
use mind::mem::Pages;
use mind::sys::Error;
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
/// How long a controller may take to enumerate.
const CLAIM_MS: u64 = 30_000;
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
/// The machine's output read from the audio gateway's sink a tenth of a second ahead, at least 10 ms at a time.
const OUTPUT_FRAMES: usize = 4_800;
const OUTPUT_LEAST: usize = 480;
/// Where bulk OUT data sit in the USB buffer: past the reports and control data at its start.
const BULK_OUT_AT: usize = 0x8000;

fn say(args: core::fmt::Arguments) { mind::println!("[BT] {}", args); }
fn now() -> u64 { mind::time::uptime_ms() as u64 }
fn idle() -> ! { loop { let _ = mind::time::sleep(60_000); } }

/// The controller as claimed, as bt_host's driver sees it.
struct Usb { host: Host, handle: u32, number: u8, events_ep: u8, bulk_in: u8, bulk_out: u8 }

fn port_error(e: Error) -> PortError { if matches!(e, Error::NotFound | Error::Peer) { PortError::Gone } else { PortError::Failed } }

/// A file read whole into `out` and checked by its size and SHA-256.
fn pinned((path, size, sha): (&'static str, usize, [u8; 32]), out: &mut [u8]) -> Option<usize> {
    if mind::dev::cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT || size > out.len() { return None; }
    mind::idl::wire::with_timeout(2_000, || -> Result<usize, ()> {
        let mut file = mind::fs::File::open(path).map_err(|_| ())?;
        if file.size() != size { return Err(()); }
        let mut at = 0;
        while at < size { let n = file.read(&mut out[at..size]).map_err(|_| ())?; if n == 0 { return Err(()); } at += n; }
        if mind::sha256::digest(&out[..size]) != sha { return Err(()); }
        Ok(size)
    }).ok()
}

impl Port for Usb {
    fn now(&mut self) -> u64 { now() }
    fn sleep(&mut self, ms: u64) { let _ = mind::time::sleep(ms.min(60_000) as usize); }
    fn command(&mut self, packet: &[u8]) -> Result<(), PortError> {
        self.host.buffer_mut()[..packet.len()].copy_from_slice(packet);
        self.host.control(self.handle, 0x20, 0, 0, self.number as u16, packet.len() as u16).map(drop).map_err(port_error)
    }
    fn events(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.host.reports(self.handle, self.events_ep, |p| each(p)).map(drop).map_err(port_error) }
    fn acl_in(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError> { self.host.reports(self.handle, self.bulk_in, |p| each(p)).map(drop).map_err(port_error) }
    fn acl_out(&mut self, packet: &[u8]) -> Result<(), PortError> {
        self.host.buffer_mut()[BULK_OUT_AT..BULK_OUT_AT + packet.len()].copy_from_slice(packet);
        self.host.bulk(self.handle, self.bulk_out, BULK_OUT_AT, packet.len()).map(drop).map_err(port_error)
    }
    fn say(&mut self, line: core::fmt::Arguments) { say(line) }
    fn firmware(&mut self, which: Firmware, out: &mut [u8]) -> Option<usize> {
        pinned(match which { Firmware::Rtl8821cPatch => PATCH, Firmware::Rtl8821cConfig => CONFIG, Firmware::Rtl8852bPatch => PATCH_8852B }, out)
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
    let endpoint = |info: &Interface, f: fn(&mind::usb::EndpointInfo) -> bool| info.endpoints().iter().find(|e| f(e)).copied();
    let Some(events_ep) = endpoint(&info, |e| e.is_interrupt() && e.is_in()) else { say(format_args!("{:04X}:{:04X}: NO EVENT ENDPOINT", info.vendor, info.product)); idle() };
    say(format_args!("CONTROLLER {:04X}:{:04X}, INTERFACE {}, EVENTS ON {:02X}", info.vendor, info.product, info.number, events_ep.address));
    let (bulk_in, bulk_out) = (endpoint(&info, |e| e.is_bulk() && e.is_in()), endpoint(&info, |e| e.is_bulk() && !e.is_in()));
    let mut usb = Usb { host, handle, number: info.number, events_ep: events_ep.address, bulk_in: bulk_in.map_or(0, |e| e.address), bulk_out: bulk_out.map_or(0, |e| e.address) };
    let mut bring = Bring::new();
    let local = {
        let Some(mut scratch) = Pages::new(driver::SCRATCH) else { say(format_args!("NO MEMORY FOR THE PATCH")); idle() };
        match bring.up(&mut usb, scratch.as_mut_slice()) {
            Ok(a) => a,
            Err(e) => { say(format_args!("BRING-UP STOPPED: {:?}", e)); idle() }
        }
    };
    // What the stack is asked for: devices listed, listening. Without either, main's bt_hci's path: two commands, idle.
    let listen = mind::fs::File::open(LISTEN).is_ok() || mind::fs::File::open(INBOX_LISTEN).is_ok();
    let mut stack = Stack::new();
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
    load(&mut stack, DEVICES, &mut text);
    // Nothing asked: main's bt_hci's last two commands, then no more usb_host calls.
    if stack.devices.is_empty() && !listen { let _ = driver::start(&mut usb, &mut bring, stack, false, 0).map_err(|e| say(format_args!("BRING-UP STOPPED: {:?}", e))); idle(); }
    let (Some(bulk_in), Some(_)) = (bulk_in, bulk_out) else { say(format_args!("NO BULK ENDPOINTS: NO ACL DATA")); idle() };
    let input = mind::dev::cap_info(SLOT_PRIV).0 == CAP_KIND_INPUT;
    let mut keys = [0u8; 4096];
    let kept = read_text(KEYS, &mut keys).map_or(0, |t| stack.devices.load_keys(t));
    let listed = stack.devices.list.iter().flatten().count();
    // The audio gateway's client badged for its sink (audio.wit 1.3, 700-DRV-0110), when init grants one (asked of KRN).
    let gateway = mind::dev::cap_info(SLOT_AUDIO).0 == CAP_KIND_ENDPOINT;
    say(format_args!("ACL ON {:02X} AND {:02X}; {} DEVICES LISTED ({} OF {} AND {}), {} KEYS; KEYS {}; HID INPUT {}; HEADPHONES PLAY {}", usb.bulk_in, usb.bulk_out, listed, lists, DEVICES, INBOX_DEVICES, kept,
        if writable { "KEPT IN data/bluetooth" } else { "KEPT FOR THIS BOOT ONLY (NO WRITABLE data/bluetooth)" }, if input { "DELIVERED" } else { "NOT DELIVERED (NO INPUT PRIVILEGE)" },
        if gateway { "THE MACHINE'S OUTPUT (OR A TONE FOR `audio tone`)" } else { "A TONE (NO CLIENT OF THE AUDIO GATEWAY'S SINK)" }));
    for d in stack.devices.list.iter().flatten() { say(format_args!("DEVICE {} {}{}", d.addr, d.kind.name(), if d.key.is_some() || d.ltk.is_some() { ", PAIRED" } else { "" })); }
    // LE pairing's random values: the stack's generator seeded from the processor's (without one, LE does not pair).
    stack.le.local = local;
    let mut seed = [0u8; 32];
    if mind::random::fill(&mut seed) { stack.le.rng.seed(&seed); } else { say(format_args!("NO {}: LE DEVICES ARE NOT PAIRED", mind::random::SOURCE)); }
    seed.fill(0);
    let mut d = match driver::start(&mut usb, &mut bring, stack, listen, bulk_in.packet as usize) {
        Ok(Some(d)) => d,
        Ok(None) => idle(),
        Err(e) => { say(format_args!("BRING-UP STOPPED: {:?}", e)); idle() }
    };
    let mut keyboard = mind::hid::Keyboard::new();
    let mut decoder = mind::keys::Ps2::new();
    let pointer = mind::hid::Pointer::boot();
    // A HID over GATT device's layout, from its report map.
    let mut mapped: Option<mind::hid::Pointer> = None;
    // The gateway's sink while held, and its refusal said once.
    let (mut held, mut refused) = (false, false);
    let mut output = [0i16; 2 * OUTPUT_FRAMES];
    loop {
        let t = now();
        let next = d.step(&mut usb, &mut |u| match u {
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
            Up::Keys => {}
        });
        // A stream to headphones listed for the machine's output takes the gateway's sink; its end gives it back.
        let wanted = gateway && d.stack.wants_output();
        if wanted != held {
            match mind::audio::sink(wanted) {
                Ok(_) => { held = wanted; d.stack.output(held); }
                Err(e) => if !refused { refused = true; say(format_args!("THE AUDIO GATEWAY'S SINK {}: {:?}", if wanted { "NOT TAKEN" } else { "NOT GIVEN BACK" }, e)); },
            }
        }
        if held {
            if let Some(frames) = d.stack.audio_wanted().filter(|&f| f >= OUTPUT_LEAST) {
                match mind::audio::sink_read(&mut output[..2 * frames.min(OUTPUT_FRAMES)]) {
                    Ok(n) => d.stack.audio(&output[..n]),
                    Err(e) => { held = false; d.stack.output(false); say(format_args!("THE AUDIO GATEWAY'S SINK NOT READ: {:?}", e)); }
                }
            }
        }
        if d.keys_changed { d.keys_changed = false; if writable && !write_keys(&d.stack) { say(format_args!("{} NOT WRITTEN", KEYS)); } }
        keyboard.repeat(t, &mut |byte| if let Some(e) = decoder.feed(byte) { mind::keyboard::deliver("BT", &decoder, e); });
        match next { Next::Sleep(ms) => usb.sleep(ms), Next::Idle => idle() }
    }
}
