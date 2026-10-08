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

enum Kind { Keyboard(Keyboard), Pointer(Pointer) }
struct Device { handle: u32, endpoint: u8, kind: Kind, complained: bool }

// Sets up a claimed HID interface: the boot protocol for a boot keyboard; for anything else the report descriptor
// says where X and Y are (the boot protocol for a boot mouse whose descriptor cannot be read).
fn setup(host: &mut Host, handle: u32, info: &Interface) -> Option<Device> {
    let endpoint = info.endpoints().iter().find(|e| e.is_interrupt() && e.is_in())?.address;
    let number = info.number as u16;
    // Each interface claimed, for a machine whose keys do not arrive (211-DRV-0003).
    mind::println!("[USB_HID] {:04X}:{:04X} INTERFACE {}: SUBCLASS {} PROTOCOL {}, ENDPOINT {:02X}", info.vendor, info.product, number, info.subclass, info.protocol, endpoint);
    let _ = host.control(handle, 0x21, 0x0A, 0, number, 0); // SET_IDLE 0: reports only on change
    if info.subclass == 1 && info.protocol == 1 {
        host.control(handle, 0x21, 0x0B, 0, number, 0).ok()?; // SET_PROTOCOL boot
        mind::println!("[USB_HID] {:04X}:{:04X} KEYBOARD", info.vendor, info.product);
        return Some(Device { handle, endpoint, kind: Kind::Keyboard(Keyboard::new()), complained: false });
    }
    let length = host.control(handle, 0x81, 6, 0x2200, number, 1024).ok(); // the report descriptor
    let pointer = length.and_then(|n| Pointer::parse(&host.buffer()[..n]));
    let pointer = match pointer {
        Some(pointer) => pointer,
        None if info.subclass == 1 && info.protocol == 2 => { host.control(handle, 0x21, 0x0B, 0, number, 0).ok()?; Pointer::boot() }
        None => { mind::println!("[USB_HID] {:04X}:{:04X} NEITHER KEYBOARD NOR POINTER, LEFT ALONE", info.vendor, info.product); return None }
    };
    mind::println!("[USB_HID] {:04X}:{:04X} {}", info.vendor, info.product, if pointer.absolute { "TABLET" } else { "MOUSE" });
    Some(Device { handle, endpoint, kind: Kind::Pointer(pointer), complained: false })
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(mut host) = Host::new(Endpoint(SLOT_DEV0)) else { mind::println!("[USB_HID] NO MEMORY"); return };
    let mut devices: [Option<Device>; MAX_DEVICES] = [const { None }; MAX_DEVICES];
    let mut decoder = Ps2::new();
    let mut claimed_at = 0u64;
    let deliver = |decoder: &mut Ps2, byte: u8| { if let Some(event) = decoder.feed(byte) { mind::keyboard::deliver("USB_HID", decoder, event); } };
    loop {
        let now = mind::time::uptime_ms() as u64;
        // New interfaces: claimed until there are none; one that cannot be used is given back.
        if now >= claimed_at + CLAIM_MS || claimed_at == 0 {
            claimed_at = now.max(1);
            while let Some(free) = devices.iter().position(Option::is_none) {
                let Ok((handle, info)) = host.claim() else { break };
                match setup(&mut host, handle, &info) { Some(device) => devices[free] = Some(device), None => { let _ = host.release(handle); } }
            }
        }
        for entry in devices.iter_mut() {
            let Some(device) = entry.as_mut() else { continue };
            let mut events = [[0u8; 64]; 8]; let mut lengths = [0usize; 8]; let mut count = 0;
            let result = host.reports(device.handle, device.endpoint, |report| {
                if count < 8 { let n = report.len().min(64); events[count][..n].copy_from_slice(&report[..n]); lengths[count] = n; count += 1; }
            });
            for (report, &n) in events.iter().zip(&lengths).take(count) {
                match &mut device.kind {
                    Kind::Keyboard(keyboard) => keyboard.feed(&report[..n], now, &mut |byte| deliver(&mut decoder, byte)),
                    Kind::Pointer(pointer) => pointer.events(&report[..n], &mut |event| { let _ = mind::dev::input_key(event, event, false); }),
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
