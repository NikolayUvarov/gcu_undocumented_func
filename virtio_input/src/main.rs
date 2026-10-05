#![no_std]
#![no_main]
// Ring 3 VirtIO input driver (issues 160, 202): up to two devices, a tablet (an absolute pointer, so the emulator needs
// no pointer grab) and a keyboard. The modern interface only (the device has no legacy one): one event virtqueue of
// 8-byte Linux input events per device in its half of the DMA region. A tablet report (EV_SYN) becomes one pointer
// event with the position, the buttons and the wheel; a key becomes its PS/2 set 1 scan code for the shared decoder
// (mind::keys: modifiers, layouts, Ctrl+Z), so the keyboard service (idl/keyboard.wit) works as with ps2_kbd. Events
// are injected with the input privilege. Holds nothing but its devices and that privilege.
use core::sync::atomic::{fence, Ordering};
use mind::abi::{pointer_at_event, BootInfo, CAP_KIND_IRQ, CAP_KIND_MMIO, POINTER_LEFT, POINTER_MIDDLE, POINTER_RIGHT, POINTER_SCALE, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, input_key, Dma, Irq, Mmio};
use mind::idl::{keyboard, wire};
use mind::ipc::Endpoint;
use mind::keys::Ps2;
use mind::virtio::{Layout, Modern, NO_VECTOR};

const SLOT_IRQ1: usize = 7; // the second device's interrupt
const RECEIVED_CAP: usize = 9;
const EVENTS: usize = 64;
const SHARE: usize = 16 * 1024; // each device's part of the DMA region: the virtqueue, then the event buffers
const QUEUE_BYTES: usize = 8192;
const DESC_WRITE: u16 = 2;
const POLL_MS: u32 = 20; // the queues are also looked at without an interrupt (a shared or lost line)
// Linux input event types and codes.
const EV_SYN: u16 = 0; const EV_KEY: u16 = 1; const EV_REL: u16 = 2; const EV_ABS: u16 = 3;
const ABS_X: u16 = 0; const ABS_Y: u16 = 1; const REL_WHEEL: u16 = 8;
const BTN_LEFT: u16 = 0x110; const BTN_RIGHT: u16 = 0x111; const BTN_MIDDLE: u16 = 0x112;
// virtio_input_config: select, subsel, size, then the answer at 8; an axis's min and max are its first two words.
const CFG_EV_BITS: u8 = 0x11; const CFG_ABS_INFO: u8 = 0x12;

const fn align(value: usize) -> usize { (value + 4095) & !4095 }

/// PS/2 set 1 bytes of a Linux key code: the codes up to 88 are the set 1 make codes; the rest need the E0 prefix.
fn scancode(code: u16) -> Option<(bool, u8)> {
    Some(match code {
        1..=88 => (false, code as u8),
        96 => (true, 0x1C), 97 => (true, 0x1D), 98 => (true, 0x35), 100 => (true, 0x38), // KP Enter, right Ctrl, KP /, right Alt
        102 => (true, 0x47), 103 => (true, 0x48), 104 => (true, 0x49), 105 => (true, 0x4B), 106 => (true, 0x4D),
        107 => (true, 0x4F), 108 => (true, 0x50), 109 => (true, 0x51), 110 => (true, 0x52), 111 => (true, 0x53),
        125 => (true, 0x5B), 126 => (true, 0x5C), 127 => (true, 0x5D), // the Windows keys and Menu
        _ => return None,
    })
}

enum Kind { Tablet { range: [(i64, i64); 2], at: [u32; 2], buttons: u8, wheel: i32, changed: bool }, Keyboard }

struct Device { modern: Modern, base: usize, size: usize, avail: usize, used: usize, last_used: u16, notify: usize, msix: bool, irq: Irq, kind: Kind }

impl Device {
    fn probe(slot: usize, irq_slot: usize, dma: &mut Dma, base: usize) -> Option<Self> {
        if cap_info(slot).0 != CAP_KIND_MMIO { return None; }
        let layout = Layout::read(slot)?;
        layout.single_bar()?;
        let modern = Modern { bar: Mmio::map(slot).ok()?, layout };
        let kind = if bits(&modern, EV_ABS) { Kind::Tablet { range: [(0, 0x7FFF); 2], at: [0; 2], buttons: 0, wheel: 0, changed: false } }
                   else if bits(&modern, EV_KEY) && !bits(&modern, EV_REL) { Kind::Keyboard } else { return None };
        modern.negotiate(0)?;
        let (irq_kind, line, _) = cap_info(irq_slot);
        let mut msix = irq_kind == CAP_KIND_IRQ && line >= 16;
        modern.set16(mind::virtio::QUEUE_SELECT, 0);
        let size = (modern.common16(mind::virtio::QUEUE_SIZE) as usize).min(EVENTS);
        let (avail, used) = (base + 16 * size, base + align(16 * size + 6 + 2 * size));
        if size == 0 || used - base + align(6 + 8 * size) > QUEUE_BYTES || dma.len() < base + QUEUE_BYTES + 8 * size { return None; }
        let addresses = (dma.physical(base), dma.physical(avail), dma.physical(used));
        let set = |vector| modern.queue(0, size as u16, addresses.0, addresses.1, addresses.2, vector);
        let (_, notify) = match set(if msix { 0 } else { NO_VECTOR }) { Some(done) => done, None if msix => { msix = false; set(NO_VECTOR)? } None => return None };
        let mut device = Self { modern, base, size, avail, used, last_used: 0, notify, msix, irq: Irq(irq_slot), kind };
        if let Kind::Tablet { range, .. } = &mut device.kind { for (axis, slot) in range.iter_mut().enumerate() { if let Some(r) = axis_range(&device.modern, axis as u8) { *slot = r; } } }
        for id in 0..size { device.offer(dma, id as u16); }
        device.modern.ready();
        device.modern.notify(device.notify, 0);
        Some(device)
    }

    // Gives buffer `id` back to the device.
    fn offer(&mut self, dma: &mut Dma, id: u16) {
        let (desc, buffer) = (self.base + 16 * id as usize, self.base + QUEUE_BYTES + 8 * id as usize);
        let physical = dma.physical(buffer);
        dma.bytes(desc, 8).copy_from_slice(&physical.to_le_bytes());
        dma.bytes(desc + 8, 4).copy_from_slice(&8u32.to_le_bytes());
        set16(dma, desc + 12, DESC_WRITE); set16(dma, desc + 14, 0);
        let index = word16(dma, self.avail + 2);
        set16(dma, self.avail + 4 + 2 * (index as usize % self.size), id);
        fence(Ordering::SeqCst); // the descriptor and ring entry are visible before the index moves
        set16(dma, self.avail + 2, index.wrapping_add(1));
    }

    // Takes every event the device wrote; returns how many.
    fn drain(&mut self, dma: &mut Dma, decoder: &mut Ps2) -> usize {
        let mut taken = 0;
        loop {
            fence(Ordering::SeqCst);
            let index = word16(dma, self.used + 2);
            if index == self.last_used || taken > 4 * self.size { break; }
            let entry = self.used + 4 + 8 * (self.last_used as usize % self.size);
            let id = u32::from_le_bytes(dma.bytes(entry, 4).try_into().unwrap()) as usize;
            self.last_used = self.last_used.wrapping_add(1);
            if id >= self.size { continue; } // a device that names a buffer it does not have
            let event: [u8; 8] = dma.bytes(self.base + QUEUE_BYTES + 8 * id, 8).try_into().unwrap();
            self.event(decoder, u16::from_le_bytes([event[0], event[1]]), u16::from_le_bytes([event[2], event[3]]), u32::from_le_bytes([event[4], event[5], event[6], event[7]]) as i32);
            self.offer(dma, id as u16);
            taken += 1;
        }
        if taken > 0 { self.modern.notify(self.notify, 0); }
        taken
    }

    fn event(&mut self, decoder: &mut Ps2, kind: u16, code: u16, value: i32) {
        match &mut self.kind {
            Kind::Keyboard => {
                // Press (1) and autorepeat (2) send the make code, release (0) the break code.
                if kind != EV_KEY { return; }
                let Some((extended, make)) = scancode(code) else { return };
                if extended { let _ = decoder.feed(0xE0); }
                if let Some(event) = decoder.feed(if value == 0 { make | 0x80 } else { make }) { mind::keyboard::deliver("VIRTIO_INPUT", decoder, event); }
            }
            Kind::Tablet { range, at, buttons, wheel, changed } => match (kind, code) {
                (EV_ABS, ABS_X | ABS_Y) => {
                    let axis = code as usize; let (min, max) = range[axis];
                    at[axis] = ((value as i64).clamp(min, max) - min) as u32 * (POINTER_SCALE - 1) / (max - min) as u32;
                    *changed = true;
                }
                (EV_KEY, BTN_LEFT | BTN_RIGHT | BTN_MIDDLE) => {
                    let bit = match code { BTN_LEFT => POINTER_LEFT, BTN_RIGHT => POINTER_RIGHT, _ => POINTER_MIDDLE };
                    if value != 0 { *buttons |= bit } else { *buttons &= !bit }
                    *changed = true;
                }
                (EV_REL, REL_WHEEL) => { *wheel -= value; *changed = true; } // up is positive here, towards the user is positive in events
                (EV_SYN, 0) if *changed => {
                    let word = pointer_at_event(*buttons, at[0], at[1], *wheel);
                    let _ = input_key(word, word, false);
                    *wheel = 0; *changed = false;
                }
                _ => {}
            },
        }
    }
}

fn word16(dma: &mut Dma, at: usize) -> u16 { u16::from_le_bytes(dma.bytes(at, 2).try_into().unwrap()) }
fn set16(dma: &mut Dma, at: usize, value: u16) { dma.bytes(at, 2).copy_from_slice(&value.to_le_bytes()) }

// Asks the device configuration `select`/`subsel`; returns the size of the answer.
fn config(modern: &Modern, select: u8, subsel: u8) -> u8 {
    let base = modern.layout.device.offset as usize;
    modern.bar.write8(base, select); modern.bar.write8(base + 1, subsel);
    modern.bar.read8(base + 2)
}
// Whether the device reports events of type `kind`.
fn bits(modern: &Modern, kind: u16) -> bool { config(modern, CFG_EV_BITS, kind as u8) > 0 }
// An axis's minimum and maximum.
fn axis_range(modern: &Modern, code: u8) -> Option<(i64, i64)> {
    if config(modern, CFG_ABS_INFO, code) < 8 { return None; }
    let base = modern.layout.device.offset as usize;
    let (min, max) = (modern.bar.read32(base + 8) as i32 as i64, modern.bar.read32(base + 12) as i32 as i64);
    (max > min).then_some((min, max))
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(mut dma) = Dma::map(SLOT_MEM) else { mind::println!("[VIRTIO_INPUT] NO DMA"); return };
    let mut devices = [Device::probe(SLOT_DEV0, SLOT_IRQ, &mut dma, 0), Device::probe(SLOT_DEV1, SLOT_IRQ1, &mut dma, SHARE)];
    if devices.iter().all(Option::is_none) { mind::println!("[VIRTIO_INPUT] NO DEVICE"); return; }
    let mut decoder = Ps2::new();
    for device in devices.iter().flatten() {
        let _ = device.irq.bind(Endpoint::SERVICE);
        let line = if device.msix { "MSI-X" } else { "LINE" };
        match &device.kind {
            Kind::Tablet { range, .. } => mind::println!("[VIRTIO_INPUT] TABLET READY: {} EVENTS, X {}..{}, Y {}..{}, {}", device.size, range[0].0, range[0].1, range[1].0, range[1].1, line),
            Kind::Keyboard => mind::println!("[VIRTIO_INPUT] KEYBOARD READY: {} EVENTS, {}", device.size, line),
        }
    }
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        for device in devices.iter_mut().flatten() {
            if !device.msix { let _ = device.modern.isr(); } // reading the ISR lowers the line
            device.drain(&mut dma, &mut decoder);
        }
        let Ok(request) = received else { continue };
        if let Some(line) = request.irq {
            for device in devices.iter().flatten().filter(|d| cap_info(d.irq.0).1 == line as usize) { let _ = device.irq.ack(); }
            continue;
        }
        match keyboard::decode(&request, RECEIVED_CAP) {
            Ok((request, call)) => mind::keyboard::serve("VIRTIO_INPUT", &mut decoder, request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
