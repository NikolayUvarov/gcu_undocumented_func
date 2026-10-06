#![no_std]
#![no_main]
// Ring 3 VirtIO input driver (issues 161, 202): up to two devices on the modern interface — configuration structures
// in a memory BAR, MSI-X or the legacy line — each with its event queue in its half of the DMA region. A tablet reports
// where the host's pointer is, so in a virtual machine the system's pointer follows it exactly (a relative PS/2 mouse
// drifts from it and the host takes its pointer back at the window's edge); its reports become pointer events for the
// focused task, as the PS/2 driver's do. A keyboard's keys become PS/2 set 1 scan codes for the shared decoder
// (mind::keys: modifiers, layouts, Ctrl+Z), and the driver serves the keyboard service (idl/keyboard.wit) as ps2_kbd
// does. The driver holds nothing but its devices and the input privilege.
mod events;

use core::sync::atomic::{fence, Ordering};
use events::{scancode, Axis, Pointer, ABS_X, ABS_Y, EV_ABS, EV_KEY, EV_REL};
use mind::abi::{BootInfo, CAP_KIND_IRQ, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, input_key, Dma, Irq, Mmio};
use mind::idl::{keyboard, wire};
use mind::ipc::Endpoint;
use mind::keys::Ps2;
use mind::virtio::{Layout, Modern, NO_VECTOR};
pub use mind::abi;

const DESC_WRITE: u16 = 2;
const QUEUE_MAX: u16 = 64;
const EVENT: usize = 8; // struct virtio_input_event { le16 type; le16 code; le32 value; }
const POLL_MS: u32 = 20; // the queue is also looked at without an interrupt (a shared or lost line)
const RECEIVED_CAP: usize = 9;
const SLOT_IRQ1: usize = 7; // the second device's interrupt
const SHARE: usize = 12 * 1024; // each device's part of the DMA region

// Device configuration (virtio_input_config): select and subsel choose what the union at offset 8 shows.
const CFG_SELECT: usize = 0; const CFG_SUBSEL: usize = 1; const CFG_SIZE: usize = 2; const CFG_DATA: usize = 8;
const CFG_ID_NAME: u8 = 0x01; const CFG_EV_BITS: u8 = 0x11; const CFG_ABS_INFO: u8 = 0x12;

const fn align(value: usize) -> usize { (value + 4095) & !4095 }

// What a device is: a pointer (a tablet or a mouse) or a keyboard.
enum Kind { Pointer(Pointer), Keyboard }

// The queue's descriptors, available and used rings, then one event buffer per descriptor at `buffers`, all in the
// device's share of the DMA region from `memory` (physical `physical`).
struct Device { modern: Modern, notify: usize, msix: bool, irq: Irq, memory: *mut u8, physical: u64, size: usize, avail: usize, used: usize, buffers: usize, last_used: u16, kind: Kind, name: [u8; 32], name_len: usize }

impl Device {
    fn memory(&self) -> *mut u8 { self.memory }
    fn read16(&self, at: usize) -> u16 { unsafe { core::ptr::read_volatile(self.memory().add(at).cast::<u16>()) } }
    fn read32(&self, at: usize) -> u32 { unsafe { core::ptr::read_volatile(self.memory().add(at).cast::<u32>()) } }
    fn write16(&self, at: usize, value: u16) { unsafe { core::ptr::write_volatile(self.memory().add(at).cast::<u16>(), value) } }
    fn write32(&self, at: usize, value: u32) { unsafe { core::ptr::write_volatile(self.memory().add(at).cast::<u32>(), value) } }
    fn write64(&self, at: usize, value: u64) { unsafe { core::ptr::write_volatile(self.memory().add(at).cast::<u64>(), value) } }

    // Configuration `select`/`subsel`: the size of what it shows, then its bytes through `read`.
    fn config(modern: &Modern, select: u8, subsel: u8) -> usize {
        let base = modern.layout.device.offset as usize;
        modern.bar.write8(base + CFG_SELECT, select);
        modern.bar.write8(base + CFG_SUBSEL, subsel);
        modern.bar.read8(base + CFG_SIZE) as usize
    }
    fn config32(modern: &Modern, offset: usize) -> u32 { modern.bar.read32(modern.layout.device.offset as usize + CFG_DATA + offset) }

    fn probe(slot: usize, irq_slot: usize, dma: &mut Dma, base: usize) -> Option<Self> {
        if cap_info(slot).0 != mind::abi::CAP_KIND_MMIO { return None; }
        let memory = dma.bytes(base, 1).as_mut_ptr();
        let layout = Layout::read(slot)?;
        layout.single_bar()?;
        let modern = Modern { bar: Mmio::map(slot).ok()?, layout };
        modern.negotiate(0)?;
        // What it is: its name, and the range of each absolute axis (QEMU's tablet: 0 to 32767).
        let mut name = [0u8; 32];
        let len = Self::config(&modern, CFG_ID_NAME, 0).min(name.len());
        for (i, byte) in name[..len].iter_mut().enumerate() { *byte = modern.bar.read8(modern.layout.device.offset as usize + CFG_DATA + i); }
        let axis = |code: u16| {
            if Self::config(&modern, CFG_ABS_INFO, code as u8) < 8 { return Axis { min: 0, max: 32767 }; }
            Axis { min: Self::config32(&modern, 0) as i32, max: Self::config32(&modern, 4) as i32 }
        };
        let (absolute, relative) = (Self::config(&modern, CFG_EV_BITS, EV_ABS as u8) > 0, Self::config(&modern, CFG_EV_BITS, EV_REL as u8) > 0);
        let kind = if absolute || relative { Kind::Pointer(Pointer::new([axis(ABS_X), axis(ABS_Y)])) }
                   else if Self::config(&modern, CFG_EV_BITS, EV_KEY as u8) > 0 { Kind::Keyboard }
                   else { mind::println!("[VIRTIO_INPUT] {}: NEITHER POINTER NOR KEYBOARD, LEFT ALONE", core::str::from_utf8(&name[..len]).unwrap_or("?")); return None };
        // MSI-X entry 0 for the event queue if init granted a vector, else the legacy line.
        let (irq_kind, line, _) = cap_info(irq_slot);
        let mut msix = irq_kind == CAP_KIND_IRQ && line >= 16;
        modern.set16(mind::virtio::MSIX_CONFIG, NO_VECTOR);
        modern.set16(mind::virtio::QUEUE_SELECT, 0);
        let size = (modern.common16(mind::virtio::QUEUE_SIZE).min(QUEUE_MAX) as usize).max(1);
        let (avail, used) = (16 * size, align(16 * size + 6 + 2 * size));
        let buffers = used + align(6 + 8 * size);
        if buffers + size * EVENT > SHARE || base + SHARE > dma.len() { mind::println!("[VIRTIO_INPUT] DMA REGION TOO SMALL"); return None; }
        let physical = dma.physical(base);
        let addresses = (physical, physical + avail as u64, physical + used as u64);
        let set = |vector| modern.queue(0, size as u16, addresses.0, addresses.1, addresses.2, vector);
        let (size, notify) = match set(if msix { 0 } else { NO_VECTOR }) { Some(done) => done, None if msix => { msix = false; set(NO_VECTOR)? } None => return None };
        let device = Self { modern, notify, msix, irq: Irq(irq_slot), memory, physical, size: size as usize, avail, used, buffers, last_used: 0, kind, name, name_len: len };
        for id in 0..device.size { device.offer(id as u16); }
        device.modern.ready();
        device.modern.notify(device.notify, 0);
        Some(device)
    }

    // Gives event buffer `id` to the device.
    fn offer(&self, id: u16) {
        let desc = 16 * id as usize;
        self.write64(desc, self.physical + (self.buffers + id as usize * EVENT) as u64); self.write32(desc + 8, EVENT as u32);
        self.write16(desc + 12, DESC_WRITE); self.write16(desc + 14, 0);
        let index = self.read16(self.avail + 2);
        self.write16(self.avail + 4 + 2 * (index as usize % self.size), id);
        fence(Ordering::SeqCst); // the descriptor and ring entry are visible before the index moves
        self.write16(self.avail + 2, index.wrapping_add(1));
        fence(Ordering::SeqCst);
    }

    // The events the device wrote, in order; each buffer goes back to it.
    fn take(&mut self, decoder: &mut Ps2) -> usize {
        let mut count = 0;
        loop {
            fence(Ordering::SeqCst);
            if self.read16(self.used + 2) == self.last_used { break; }
            let entry = self.used + 4 + 8 * (self.last_used as usize % self.size);
            let id = self.read32(entry) as usize % self.size;
            let at = self.buffers + id * EVENT;
            let (kind, code, value) = (self.read16(at), self.read16(at + 2), self.read32(at + 4) as i32);
            match &mut self.kind {
                Kind::Pointer(pointer) => pointer.feed(kind, code, value, &mut |event| { let _ = input_key(event, event, false); }),
                // Press (1) and autorepeat (2) send the make code, release (0) the break code.
                Kind::Keyboard => if let (EV_KEY, Some((extended, make))) = (kind, scancode(code)) {
                    if extended { let _ = decoder.feed(0xE0); }
                    if let Some(event) = decoder.feed(if value == 0 { make | 0x80 } else { make }) { mind::keyboard::deliver("VIRTIO_INPUT", decoder, event); }
                },
            }
            self.last_used = self.last_used.wrapping_add(1);
            self.offer(id as u16);
            count += 1;
        }
        if count > 0 { self.modern.notify(self.notify, 0); }
        count
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(mut dma) = Dma::map(SLOT_MEM) else { mind::println!("[VIRTIO_INPUT] NO DMA REGION"); return };
    let mut devices = [Device::probe(SLOT_DEV0, SLOT_IRQ, &mut dma, 0), Device::probe(SLOT_DEV1, SLOT_IRQ1, &mut dma, SHARE)];
    if devices.iter().all(Option::is_none) { mind::println!("[VIRTIO_INPUT] NO DEVICE"); return; }
    for device in devices.iter().flatten() {
        let wired = device.irq.bind(Endpoint::SERVICE).is_ok();
        let line = if device.msix { "MSI-X" } else if wired { "INTX" } else { "POLLED" };
        let name = core::str::from_utf8(&device.name[..device.name_len]).unwrap_or("?");
        match &device.kind {
            Kind::Pointer(p) => mind::println!("[VIRTIO_INPUT] {} X={}..{} Y={}..{} QUEUE={} {}", name, p.axes[0].min, p.axes[0].max, p.axes[1].min, p.axes[1].max, device.size, line),
            Kind::Keyboard => mind::println!("[VIRTIO_INPUT] {} KEYBOARD QUEUE={} {}", name, device.size, line),
        }
    }
    let mut decoder = Ps2::new();
    loop {
        // Interrupts arrive on the service endpoint between keyboard requests; every queue is also looked at on a timeout.
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        for device in devices.iter_mut().flatten() {
            if !device.msix { let _ = device.modern.isr(); } // reading the ISR lowers the line
            device.take(&mut decoder);
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
