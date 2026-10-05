#![no_std]
#![no_main]
// Ring 3 VirtIO input driver (issue 160): a tablet, an absolute pointer, so the emulator needs no pointer grab. The
// modern interface only (the device has no legacy one): one event virtqueue of 8-byte Linux input events in its own
// DMA region; each report (EV_SYN) becomes one pointer event with the position, the buttons and the wheel, injected
// with the input privilege like ps2_kbd's events. Holds nothing but its device and that privilege.
use core::sync::atomic::{fence, Ordering};
use mind::abi::{pointer_at_event, BootInfo, CAP_KIND_IRQ, POINTER_LEFT, POINTER_MIDDLE, POINTER_RIGHT, POINTER_SCALE, SLOT_DEV0, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, input_key, Dma, Irq, Mmio};
use mind::ipc::Endpoint;
use mind::virtio::{Layout, Modern, NO_VECTOR};

const EVENTS: usize = 64;
const QUEUE_BYTES: usize = 8192; // the virtqueue; the event buffers follow
const DESC_WRITE: u16 = 2;
const POLL_MS: u32 = 20; // the queue is also looked at without an interrupt (a shared or lost line)
// Linux input event types and codes.
const EV_SYN: u16 = 0; const EV_KEY: u16 = 1; const EV_REL: u16 = 2; const EV_ABS: u16 = 3;
const ABS_X: u16 = 0; const ABS_Y: u16 = 1; const REL_WHEEL: u16 = 8;
const BTN_LEFT: u16 = 0x110; const BTN_RIGHT: u16 = 0x111; const BTN_MIDDLE: u16 = 0x112;
// virtio_input_config: select, subsel, size, then the answer at 8; an axis's min and max are its first two words.
const CFG_ABS_INFO: u8 = 0x12;

const fn align(value: usize) -> usize { (value + 4095) & !4095 }

struct Tablet { modern: Modern, dma: Dma, size: usize, avail: usize, used: usize, last_used: u16, notify: usize, msix: bool,
                range: [(i64, i64); 2], at: [u32; 2], buttons: u8, wheel: i32, changed: bool }

impl Tablet {
    fn probe() -> Option<Self> {
        if cap_info(SLOT_DEV0).0 != mind::abi::CAP_KIND_MMIO { return None; }
        let layout = Layout::read(SLOT_DEV0)?;
        layout.single_bar()?;
        let modern = Modern { bar: Mmio::map(SLOT_DEV0).ok()?, layout };
        modern.negotiate(0)?;
        let dma = Dma::map(SLOT_MEM).ok()?;
        let (kind, line, _) = cap_info(SLOT_IRQ);
        let mut msix = kind == CAP_KIND_IRQ && line >= 16;
        modern.set16(mind::virtio::QUEUE_SELECT, 0);
        let size = (modern.common16(mind::virtio::QUEUE_SIZE) as usize).min(EVENTS);
        let (avail, used) = (16 * size, align(16 * size + 6 + 2 * size));
        if size == 0 || used + align(6 + 8 * size) > QUEUE_BYTES || dma.len() < QUEUE_BYTES + 8 * size { return None; }
        let addresses = (dma.physical(0), dma.physical(avail), dma.physical(used));
        let set = |vector| modern.queue(0, size as u16, addresses.0, addresses.1, addresses.2, vector);
        let (_, notify) = match set(if msix { 0 } else { NO_VECTOR }) { Some(done) => done, None if msix => { msix = false; set(NO_VECTOR)? } None => return None };
        let mut tablet = Self { modern, dma, size, avail, used, last_used: 0, notify, msix, range: [(0, 0x7FFF); 2], at: [0; 2], buttons: 0, wheel: 0, changed: false };
        for axis in 0..2 { if let Some(range) = tablet.axis(axis as u8) { tablet.range[axis] = range; } }
        for id in 0..size { tablet.offer(id as u16); }
        tablet.modern.ready();
        tablet.modern.notify(tablet.notify, 0);
        Some(tablet)
    }

    // An axis's minimum and maximum from the device configuration.
    fn axis(&self, code: u8) -> Option<(i64, i64)> {
        let base = self.modern.layout.device.offset as usize;
        self.modern.bar.write8(base, CFG_ABS_INFO); self.modern.bar.write8(base + 1, code);
        if self.modern.bar.read8(base + 2) < 8 { return None; }
        let (min, max) = (self.modern.bar.read32(base + 8) as i32 as i64, self.modern.bar.read32(base + 12) as i32 as i64);
        (max > min).then_some((min, max))
    }

    fn word16(&mut self, at: usize) -> u16 { u16::from_le_bytes(self.dma.bytes(at, 2).try_into().unwrap()) }
    fn set16(&mut self, at: usize, value: u16) { self.dma.bytes(at, 2).copy_from_slice(&value.to_le_bytes()) }

    // Gives buffer `id` back to the device.
    fn offer(&mut self, id: u16) {
        let (desc, buffer) = (16 * id as usize, QUEUE_BYTES + 8 * id as usize);
        let physical = self.dma.physical(buffer);
        self.dma.bytes(desc, 8).copy_from_slice(&physical.to_le_bytes());
        self.dma.bytes(desc + 8, 4).copy_from_slice(&8u32.to_le_bytes());
        self.set16(desc + 12, DESC_WRITE); self.set16(desc + 14, 0);
        let index = self.word16(self.avail + 2);
        let slot = self.avail + 4 + 2 * (index as usize % self.size);
        self.set16(slot, id);
        fence(Ordering::SeqCst); // the descriptor and ring entry are visible before the index moves
        self.set16(self.avail + 2, index.wrapping_add(1));
    }

    // Takes every event the device wrote; returns how many.
    fn drain(&mut self) -> usize {
        let mut taken = 0;
        loop {
            fence(Ordering::SeqCst);
            let index = self.word16(self.used + 2);
            if index == self.last_used || taken > 4 * self.size { break; }
            let entry = self.used + 4 + 8 * (self.last_used as usize % self.size);
            let id = u32::from_le_bytes(self.dma.bytes(entry, 4).try_into().unwrap()) as usize;
            self.last_used = self.last_used.wrapping_add(1);
            if id >= self.size { continue; } // a device that names a buffer it does not have
            let event: [u8; 8] = self.dma.bytes(QUEUE_BYTES + 8 * id, 8).try_into().unwrap();
            self.event(u16::from_le_bytes([event[0], event[1]]), u16::from_le_bytes([event[2], event[3]]), u32::from_le_bytes([event[4], event[5], event[6], event[7]]) as i32);
            self.offer(id as u16);
            taken += 1;
        }
        if taken > 0 { self.modern.notify(self.notify, 0); }
        taken
    }

    fn event(&mut self, kind: u16, code: u16, value: i32) {
        match (kind, code) {
            (EV_ABS, ABS_X | ABS_Y) => {
                let axis = code as usize; let (min, max) = self.range[axis];
                self.at[axis] = ((value as i64).clamp(min, max) - min) as u32 * (POINTER_SCALE - 1) / (max - min) as u32;
                self.changed = true;
            }
            (EV_KEY, BTN_LEFT | BTN_RIGHT | BTN_MIDDLE) => {
                let bit = match code { BTN_LEFT => POINTER_LEFT, BTN_RIGHT => POINTER_RIGHT, _ => POINTER_MIDDLE };
                if value != 0 { self.buttons |= bit } else { self.buttons &= !bit }
                self.changed = true;
            }
            (EV_REL, REL_WHEEL) => { self.wheel -= value; self.changed = true; } // up is positive here, towards the user is positive in events
            (EV_SYN, 0) if self.changed => {
                let word = pointer_at_event(self.buttons, self.at[0], self.at[1], self.wheel);
                let _ = input_key(word, word, false);
                self.wheel = 0; self.changed = false;
            }
            _ => {}
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Some(mut tablet) = Tablet::probe() else { mind::println!("[VIRTIO_INPUT] NO DEVICE"); return };
    let irq = Irq(SLOT_IRQ);
    let _ = irq.bind(Endpoint::SERVICE);
    mind::println!("[VIRTIO_INPUT] TABLET READY: {} EVENTS, X {}..{}, Y {}..{}, {}", tablet.size, tablet.range[0].0, tablet.range[0].1,
                   tablet.range[1].0, tablet.range[1].1, if tablet.msix { "MSI-X" } else { "LINE" });
    loop {
        let received = Endpoint::SERVICE.recv_timeout(0, POLL_MS);
        if matches!(&received, Ok(request) if request.irq.is_some()) {
            if !tablet.msix { let _ = tablet.modern.isr(); } // reading the ISR lowers the line
            tablet.drain();
            let _ = irq.ack();
        } else {
            tablet.drain();
        }
    }
}
