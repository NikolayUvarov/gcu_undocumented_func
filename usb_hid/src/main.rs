#![no_std]
#![no_main]
// Ring 3 USB HID class driver (issue 164): keyboards, mice and tablets through usb_host (idl/usb.wit; its client
// with the HID badge in SLOT_DEV0 can claim only HID interfaces). A keyboard is put in the boot protocol and its
// reports become PS/2 set 1 scan codes for the shared decoder (mind::keys), with key repeat done here, as a USB
// keyboard leaves it to the host; the driver serves the keyboard service (idl/keyboard.wit) as ps2_kbd does. A pointing
// device's reports, laid out by its report descriptor, become pointer events for the focused task. Devices are
// claimed as they come; one that goes away is let go.
use mind::abi::{BootInfo, SLOT_DEV0};
use mind::hid::{Keyboard, Pointer};
use mind::idl::{keyboard, wire};
use mind::ipc::Endpoint;
use mind::keys::Ps2;
use mind::sys::Error;
use mind::usb::{Host, Interface};

const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 10;
const CLAIM_MS: u64 = 500; // how often new devices are looked for
const MAX_DEVICES: usize = 4;
const RETRY_MS: u64 = 5000; // an interface that could not be set up is tried again this often
const RETRIES: u8 = 5;
const SHOWN: u8 = 3; // a pointer's first reports are logged, for a mouse that does not move the cursor (211-DRV-0003)
const DECLINED: usize = 8; // interfaces this driver does not serve, kept claimed so that usb_host does not offer them again

enum Kind { Keyboard(Keyboard), Pointer(Pointer) }
struct Device { handle: u32, info: Interface, endpoint: u8, kind: Kind, complained: bool, shown: u8, ids: [u64; 4], misses: u8 }
// An interface kept claimed after its setup failed, so that the others are claimed meanwhile.
struct Failed { handle: u32, info: Interface, at: u64, tries: u8 }

// The report IDs a report descriptor declares, as a set.
fn report_ids(descriptor: &[u8]) -> [u64; 4] {
    let (mut ids, mut at) = ([0u64; 4], 0);
    while at < descriptor.len() {
        let prefix = descriptor[at];
        if prefix == 0xFE { at += 3 + descriptor.get(at + 1).copied().unwrap_or(0) as usize; continue; }
        if prefix & 0xFC == 0x84 { if let Some(&id) = descriptor.get(at + 1) { ids[id as usize / 64] |= 1 << (id % 64); } }
        at += 1 + [0, 1, 2, 4][(prefix & 3) as usize];
    }
    ids
}

// Sets up a claimed HID interface: the boot protocol for a boot keyboard; for anything else the report descriptor
// says where X and Y are (the boot protocol for a boot mouse whose descriptor cannot be read). Err(None): not a device
// this driver serves; Err(Some): a request failed.
fn setup(host: &mut Host, handle: u32, info: &Interface, first: bool) -> Result<Device, Option<Error>> {
    let endpoint = info.endpoints().iter().find(|e| e.is_interrupt() && e.is_in()).ok_or(None)?.address;
    let (number, name) = (info.number as u16, (info.vendor, info.product));
    // Each interface claimed, for a machine whose keys do not arrive (211-DRV-0003).
    if first { mind::println!("[USB_HID] {:04X}:{:04X} INTERFACE {}: SUBCLASS {} PROTOCOL {}, ENDPOINT {:02X}", name.0, name.1, number, info.subclass, info.protocol, endpoint); }
    let device = |kind, ids| Device { handle, info: *info, endpoint, kind, complained: false, shown: 0, ids, misses: 0 };
    let _ = host.control(handle, 0x21, 0x0A, 0, number, 0); // SET_IDLE 0: reports only on change
    if info.subclass == 1 && info.protocol == 1 {
        host.control(handle, 0x21, 0x0B, 0, number, 0).map_err(Some)?; // SET_PROTOCOL boot
        mind::println!("[USB_HID] {:04X}:{:04X} KEYBOARD", name.0, name.1);
        return Ok(device(Kind::Keyboard(Keyboard::new()), [0; 4]));
    }
    let length = host.control(handle, 0x81, 6, 0x2200, number, 1024); // the report descriptor
    let (parsed, ids) = match length { Ok(n) => (Pointer::parse(&host.buffer()[..n]), report_ids(&host.buffer()[..n])), Err(_) => (None, [0; 4]) };
    let pointer = match (parsed, length) {
        // The firmware may have left a boot device in the boot protocol, whose reports the descriptor does not describe.
        (Some(pointer), _) => { if info.subclass == 1 { let _ = host.control(handle, 0x21, 0x0B, 1, number, 0); } pointer }
        (None, _) if info.subclass == 1 && info.protocol == 2 => { host.control(handle, 0x21, 0x0B, 0, number, 0).map_err(Some)?; Pointer::boot() }
        (None, Err(error)) => return Err(Some(error)),
        (None, Ok(_)) => { mind::println!("[USB_HID] {:04X}:{:04X} NEITHER KEYBOARD NOR POINTER, LEFT ALONE", name.0, name.1); return Err(None) }
    };
    mind::println!("[USB_HID] {:04X}:{:04X} {}: ID {}, X AT BIT {} ({} BITS), Y AT BIT {} ({} BITS), {} PROTOCOL", name.0, name.1,
                   if pointer.absolute { "TABLET" } else { "MOUSE" }, pointer.id, pointer.x.offset, pointer.x.size, pointer.y.offset,
                   pointer.y.size, if parsed.is_some() { "REPORT" } else { "BOOT" });
    Ok(device(Kind::Pointer(pointer), ids))
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(mut host) = Host::new(Endpoint(SLOT_DEV0)) else { mind::println!("[USB_HID] NO MEMORY"); return };
    let mut devices: [Option<Device>; MAX_DEVICES] = [const { None }; MAX_DEVICES];
    let mut failed: [Option<Failed>; MAX_DEVICES] = [const { None }; MAX_DEVICES];
    // Released, an interface comes straight back from claim: one not ours is kept here instead (211-DRV-0016, the
    // MacBook Pro's keyboard interface 1 was claimed and released without end).
    let mut declined = [0u32; DECLINED]; let mut declined_count = 0;
    let mut decoder = Ps2::new();
    let mut claimed_at = 0u64;
    let deliver = |decoder: &mut Ps2, byte: u8| { if let Some(event) = decoder.feed(byte) { mind::keyboard::deliver("USB_HID", decoder, event); } };
    loop {
        let now = mind::time::uptime_ms() as u64;
        // Interfaces that failed: tried again now and then; one whose device is gone, or that failed every time, is given back.
        for entry in failed.iter_mut() {
            let Some(retry) = entry.as_mut().filter(|f| now >= f.at) else { continue };
            let Some(free) = devices.iter().position(Option::is_none) else { break };
            match setup(&mut host, retry.handle, &retry.info, false) {
                Ok(device) => { devices[free] = Some(device); *entry = None; }
                Err(Some(error)) if retry.tries < RETRIES && !matches!(error, Error::NotFound | Error::Peer) => { retry.tries += 1; retry.at = now + RETRY_MS; }
                Err(error) => {
                    mind::println!("[USB_HID] {:04X}:{:04X} INTERFACE {} GIVEN UP: {:?}", retry.info.vendor, retry.info.product, retry.info.number, error);
                    // Kept claimed like one not ours, so that it is not set up again at the next look.
                    if declined_count < DECLINED && !matches!(error, Some(Error::NotFound | Error::Peer)) { declined[declined_count] = retry.handle; declined_count += 1; } else { let _ = host.release(retry.handle); }
                    *entry = None;
                }
            }
        }
        // New interfaces: claimed until there are none; one that cannot be used is given back, one that failed kept to retry.
        if now >= claimed_at + CLAIM_MS || claimed_at == 0 {
            claimed_at = now.max(1);
            while let Some(free) = devices.iter().position(Option::is_none) {
                let Ok((handle, info)) = host.claim() else { break };
                match setup(&mut host, handle, &info, true) {
                    Ok(device) => devices[free] = Some(device),
                    Err(Some(error)) if !matches!(error, Error::NotFound | Error::Peer) => {
                        mind::println!("[USB_HID] {:04X}:{:04X} INTERFACE {} NOT SET UP: {:?}, TRIED AGAIN IN {} S", info.vendor, info.product, info.number, error, RETRY_MS / 1000);
                        match failed.iter_mut().find(|f| f.is_none()) {
                            Some(slot) => *slot = Some(Failed { handle, info, at: now + RETRY_MS, tries: 1 }),
                            None => { let _ = host.release(handle); break } // given back, and not claimed again before the next look
                        }
                    }
                    Err(_) if declined_count < DECLINED => { declined[declined_count] = handle; declined_count += 1; }
                    Err(_) => { let _ = host.release(handle); break } // not claimed again before the next look
                }
            }
        }
        for entry in devices.iter_mut() {
            let Some(device) = entry.as_mut() else { continue };
            let mut events = [[0u8; 64]; 8]; let mut lengths = [0usize; 8]; let mut count = 0;
            let result = host.reports(device.handle, device.endpoint, |report| {
                if count < 8 { let n = report.len().min(64); events[count][..n].copy_from_slice(&report[..n]); lengths[count] = n; count += 1; }
            });
            for (report, &n) in events.iter().zip(&lengths).take(count) {
                let report = &report[..n];
                match &mut device.kind {
                    Kind::Keyboard(keyboard) => keyboard.feed(report, now, &mut |byte| deliver(&mut decoder, byte)),
                    Kind::Pointer(pointer) => {
                        if device.shown < SHOWN { device.shown += 1; mind::println!("[USB_HID] {:04X}:{:04X} REPORT {:02X?}", device.info.vendor, device.info.product, report); }
                        // A boot mouse still in the boot protocol: its reports start with no ID its descriptor declares.
                        let first = report.first().copied().unwrap_or(0);
                        if pointer.id != 0 && device.info.subclass == 1 && device.info.protocol == 2 && device.ids[first as usize / 64] >> (first % 64) & 1 == 0 {
                            device.misses += 1;
                            if device.misses == 2 {
                                let _ = host.control(device.handle, 0x21, 0x0B, 0, device.info.number as u16, 0); // SET_PROTOCOL boot
                                *pointer = Pointer::boot();
                                mind::println!("[USB_HID] {:04X}:{:04X} SENDS BOOT REPORTS: BOOT PROTOCOL USED", device.info.vendor, device.info.product);
                            }
                        }
                        pointer.events(report, &mut |event| { let _ = mind::dev::input_key(event, event, false); });
                    }
                }
            }
            if let Kind::Keyboard(keyboard) = &mut device.kind { keyboard.repeat(now, &mut |byte| deliver(&mut decoder, byte)); }
            // Gone (unplugged, or usb_host restarted): its keys are released and the handle forgotten.
            if let Err(error) = result { if !device.complained && !matches!(error, Error::NotFound | Error::Peer) { device.complained = true; mind::println!("[USB_HID] REPORTS: {:?}", error); } }
            if matches!(result, Err(Error::NotFound | Error::Peer)) {
                if let Kind::Keyboard(keyboard) = &mut device.kind { keyboard.release_all(&mut |byte| deliver(&mut decoder, byte)); }
                mind::println!("[USB_HID] DEVICE GONE");
                *entry = None;
                claimed_at = 0;
                if matches!(result, Err(Error::Peer)) { declined_count = 0; } // a new usb_host knows none of the old handles
            }
        }
        // Keyboard requests (the shell's keymap) between the polls; without a device, until the next look for one.
        let wait = if devices.iter().any(Option::is_some) { POLL_MS } else { CLAIM_MS as u32 };
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, wait) else { continue };
        match keyboard::decode(&request, RECEIVED_CAP) {
            Ok((request, call)) => mind::keyboard::serve("USB_HID", &mut decoder, request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
