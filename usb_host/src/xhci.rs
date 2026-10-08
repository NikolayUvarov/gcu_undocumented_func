// xHCI: command and event rings, a pool of DMA pages for device contexts and transfer rings, input contexts, synchronous
// control and bulk transfers, and interrupt IN endpoints kept polling with their reports queued (issue 164).
use core::sync::atomic::{fence, Ordering};
use mind::dev::{Dma, Mmio};

pub const TRB: usize = 16;
pub const PAGE: usize = 4096;
const RING_TRBS: usize = PAGE / TRB; // command, event, control and bulk rings: one page
const INTERRUPT_TRBS: usize = 64; // an interrupt ring: 1 KiB, then its report buffers in the same page
const REPORT_BUFFERS: usize = 16; const REPORT_BYTES: usize = 64; const REPORT_AREA: usize = 0x400;
pub const ARMED: usize = 8; // interrupt TRBs kept on the ring

// DMA region layout (512 KiB, 64 KiB aligned).
const DCBAA: usize = 0x0000; const SCRATCH_ARRAY: usize = 0x0800; const COMMAND_RING: usize = 0x1000; const EVENT_RING: usize = 0x2000;
const ERST: usize = 0x3000; const INPUT: usize = 0x4000; pub const SMALL: usize = 0x6000; const SCRATCH_PAGES: usize = 0x10000;
const MAX_SCRATCH: usize = 32; pub const DATA: usize = 0x30000; const POOL: usize = 0x40000; const POOL_PAGES: usize = 64;
pub const DMA_BYTES: usize = POOL + POOL_PAGES * PAGE;

pub const TYPE_NORMAL: u32 = 1; const TYPE_SETUP: u32 = 2; const TYPE_DATA: u32 = 3; const TYPE_STATUS: u32 = 4; const TYPE_LINK: u32 = 6;
const TYPE_ENABLE_SLOT: u32 = 9; const TYPE_DISABLE_SLOT: u32 = 10; const TYPE_ADDRESS: u32 = 11; const TYPE_CONFIGURE: u32 = 12;
const TYPE_EVALUATE: u32 = 13; const TYPE_RESET_ENDPOINT: u32 = 14; const TYPE_SET_DEQUEUE: u32 = 16;
const EVENT_TRANSFER: u32 = 32; const EVENT_COMMAND: u32 = 33;
pub const IOC: u32 = 1 << 5; const IDT: u32 = 1 << 6; pub const ISP: u32 = 1 << 2;
pub const SUCCESS: u32 = 1; pub const STALL: u32 = 6; pub const SHORT_PACKET: u32 = 13;
// PORTSC bits written back without side effects (no RW1C bits, no PED).
const PORT_NEUTRAL: u32 = 0x4E00_FFE9; const PORT_PED: u32 = 1 << 1; const PORT_PR: u32 = 1 << 4; const PORT_CHANGES: u32 = 0x7F << 17; const PORT_PRC: u32 = 1 << 21;

/// Endpoint context types.
pub const EP_BULK_OUT: u32 = 2; pub const EP_INTERRUPT_OUT: u32 = 3; pub const EP_CONTROL: u32 = 4; pub const EP_BULK_IN: u32 = 6; pub const EP_INTERRUPT_IN: u32 = 7;

#[derive(Clone, Copy, Default)]
pub struct Ring { pub page: usize, trbs: usize, index: usize, cycle: u32 }

// A synchronous transfer's completion: slot, endpoint (DCI), completion code, residue.
#[derive(Clone, Copy)]
struct Done { slot: u8, dci: u8, code: u32, residue: u32 }

const QUEUE: usize = 8;
/// An interrupt IN endpoint being polled and the reports it gave since they were last taken.
pub struct Interrupt { pub slot: u8, pub dci: u8, ring: Ring, length: u32, queue: [[u8; REPORT_BYTES]; QUEUE], lengths: [u8; QUEUE], head: usize, count: usize, pub failed: bool }

const MAX_INTERRUPTS: usize = 8; const MAX_DONE: usize = 8;

pub struct Xhci {
    mmio: Mmio, pub dma: Dma, op: usize, runtime: usize, doorbells: usize, context: usize, ports: usize, pub slots: usize,
    command: Ring, event: Ring, free: u64, completion: Option<(u32, u32)>, done: [Option<Done>; MAX_DONE], interrupts: [Option<Interrupt>; MAX_INTERRUPTS],
    pub last: u32, // completion code of the last command or transfer (0: no answer), for reports
}

/// Busy-poll, then sleep: QEMU completes commands at once, transfers asynchronously; about 30 s in all.
pub fn wait(mut done: impl FnMut() -> bool) -> bool {
    for attempt in 0..4_000 { if done() { return true; } if attempt > 1_000 { mind::time::sleep(10); } else { core::hint::spin_loop(); } }
    false
}

impl Xhci {
    pub fn init(mmio: Mmio, mut dma: Dma) -> Option<Self> {
        if dma.len() < DMA_BYTES { mind::println!("[USB] DMA REGION TOO SMALL"); return None; }
        let caplength = mmio.read8(0) as usize; let hcs1 = mmio.read32(0x04); let hcs2 = mmio.read32(0x08); let hcc1 = mmio.read32(0x10);
        let (doorbells, runtime) = ((mmio.read32(0x14) & !3) as usize, (mmio.read32(0x18) & !0x1F) as usize);
        let context = if hcc1 & 4 != 0 { 64 } else { 32 };
        // Take the controller from firmware (USB Legacy Support), then halt and reset it.
        let mut cap = ((hcc1 >> 16) << 2) as usize;
        while cap != 0 {
            let value = mmio.read32(cap);
            if value & 0xFF == 1 { mmio.write32(cap, value | 1 << 24); wait(|| mmio.read32(cap) & 1 << 16 == 0); mmio.write32(cap + 4, 0); }
            let next = ((value >> 8) & 0xFF) as usize; cap = if next == 0 { 0 } else { cap + next * 4 };
        }
        let op = caplength;
        mmio.write32(op, mmio.read32(op) & !1);
        if !wait(|| mmio.read32(op + 4) & 1 != 0) { return None; }
        mmio.write32(op, 2);
        if !wait(|| mmio.read32(op) & 2 == 0 && mmio.read32(op + 4) & 1 << 11 == 0) { return None; }
        let scratch = ((hcs2 >> 21 & 0x1F) << 5 | hcs2 >> 27) as usize;
        if scratch > MAX_SCRATCH { mind::println!("[USB] {} SCRATCHPAD PAGES NEEDED, {} AVAILABLE", scratch, MAX_SCRATCH); return None; }
        dma.zero(0, POOL);
        for i in 0..scratch { let page = dma.physical(SCRATCH_PAGES + i * PAGE); dma.write64(SCRATCH_ARRAY + i * 8, page); }
        if scratch > 0 { let array = dma.physical(SCRATCH_ARRAY); dma.write64(DCBAA, array); }
        let slots = (hcs1 & 0xFF).min(16) as usize;
        mmio.write32(op + 0x38, slots as u32);
        mmio.write64(op + 0x30, dma.physical(DCBAA));
        let mut xhci = Self { mmio, dma, op, runtime, doorbells, context, ports: (hcs1 >> 24) as usize, slots,
            command: Ring { page: COMMAND_RING, trbs: RING_TRBS, index: 0, cycle: 1 }, event: Ring { page: EVENT_RING, trbs: RING_TRBS, index: 0, cycle: 1 },
            free: u64::MAX, completion: None, done: [None; MAX_DONE], interrupts: [const { None }; MAX_INTERRUPTS], last: 0 };
        xhci.link(xhci.command);
        xhci.mmio.write64(op + 0x18, xhci.dma.physical(COMMAND_RING) | 1);
        let event = xhci.dma.physical(EVENT_RING); xhci.dma.write64(ERST, event); xhci.dma.write32(ERST + 8, RING_TRBS as u32);
        let interrupter = runtime + 0x20;
        xhci.mmio.write32(interrupter + 0x08, 1);
        xhci.mmio.write64(interrupter + 0x18, event);
        xhci.mmio.write64(interrupter + 0x10, xhci.dma.physical(ERST));
        xhci.mmio.write32(op, 1); // RS; events are polled
        if !wait(|| xhci.mmio.read32(op + 4) & 1 == 0) { return None; }
        Some(xhci)
    }

    pub fn ports(&self) -> usize { self.ports }

    // DMA pages for device contexts and transfer rings.
    pub fn alloc(&mut self) -> Option<usize> {
        let bit = self.free.trailing_zeros() as usize;
        if bit >= POOL_PAGES { return None; }
        self.free &= !(1 << bit);
        let page = POOL + bit * PAGE;
        self.dma.zero(page, PAGE);
        Some(page)
    }
    pub fn release(&mut self, page: usize) { if page >= POOL { self.free |= 1 << ((page - POOL) / PAGE); } }
    pub fn physical(&self, offset: usize) -> u64 { self.dma.physical(offset) }

    /// A transfer ring in a fresh page: a full page, or an interrupt ring with its report buffers.
    pub fn ring(&mut self, interrupt: bool) -> Option<Ring> {
        let ring = Ring { page: self.alloc()?, trbs: if interrupt { INTERRUPT_TRBS } else { RING_TRBS }, index: 0, cycle: 1 };
        self.link(ring);
        Some(ring)
    }

    // The last TRB of a ring is a Link back to its start with Toggle Cycle.
    fn link(&mut self, ring: Ring) {
        let (start, last) = (self.dma.physical(ring.page), ring.page + (ring.trbs - 1) * TRB);
        self.dma.write64(last, start); self.dma.write32(last + 12, TYPE_LINK << 10 | 2);
    }

    fn enqueue(dma: &mut Dma, ring: &mut Ring, parameter: u64, status: u32, control: u32) -> u64 {
        let at = ring.page + ring.index * TRB; let address = dma.physical(at);
        dma.write64(at, parameter); dma.write32(at + 8, status);
        fence(Ordering::SeqCst); // the TRB is complete before its cycle bit hands it over
        dma.write32(at + 12, control | ring.cycle);
        ring.index += 1;
        if ring.index == ring.trbs - 1 {
            let link = ring.page + (ring.trbs - 1) * TRB;
            let value = dma.read32(link + 12); dma.write32(link + 12, (value & !1) | ring.cycle);
            ring.index = 0; ring.cycle ^= 1;
        }
        address
    }

    fn doorbell(&self, slot: u8, target: u32) { fence(Ordering::SeqCst); self.mmio.write32(self.doorbells + slot as usize * 4, target); }

    /// Takes every event the controller wrote: command completions, transfer completions, interrupt reports.
    pub fn pump(&mut self) {
        loop {
            let at = self.event.page + self.event.index * TRB;
            let control = self.dma.read32(at + 12);
            if control & 1 != self.event.cycle { break; }
            fence(Ordering::SeqCst);
            let pointer = self.dma.read32(at) as u64 | (self.dma.read32(at + 4) as u64) << 32; let status = self.dma.read32(at + 8);
            self.event.index += 1; if self.event.index == RING_TRBS { self.event.index = 0; self.event.cycle ^= 1; }
            let next = self.dma.physical(self.event.page + self.event.index * TRB);
            self.mmio.write64(self.runtime + 0x20 + 0x18, next | 8);
            let (slot, dci) = ((control >> 24) as u8, ((control >> 16) & 0x1F) as u8);
            match (control >> 10) & 0x3F {
                EVENT_COMMAND => self.completion = Some((status >> 24, control >> 24)),
                EVENT_TRANSFER => {
                    if let Some(index) = self.interrupts.iter().position(|i| i.as_ref().is_some_and(|i| i.slot == slot && i.dci == dci)) {
                        self.report(index, pointer, status);
                    } else {
                        // A late completion nobody waits for may take the place of the oldest one.
                        let at = self.done.iter().position(Option::is_none).unwrap_or(0);
                        self.done[at] = Some(Done { slot, dci, code: status >> 24, residue: status & 0xFF_FFFF });
                    }
                }
                _ => {} // port status changes are seen by scanning the ports
            }
        }
    }

    fn command(&mut self, parameter: u64, control: u32) -> Option<u32> {
        self.completion = None;
        Self::enqueue(&mut self.dma, &mut self.command, parameter, 0, control);
        self.doorbell(0, 0);
        let mut result = None;
        wait(|| { self.pump(); result = self.completion.take(); result.is_some() });
        self.last = result.map_or(0, |r| r.0);
        let (code, slot) = result?;
        (code == SUCCESS).then_some(slot)
    }

    /// TRBs on `ring` of endpoint `dci`, then its completion: Ok(residue), or Err(completion code; 0: no answer).
    pub fn transfer(&mut self, slot: u8, dci: u8, ring: &mut Ring, trbs: &[(u64, u32, u32)]) -> Result<u32, u32> {
        self.done.iter_mut().filter(|d| d.is_some_and(|d| d.slot == slot && d.dci == dci)).for_each(|d| *d = None); // stale ones
        for &(parameter, status, control) in trbs { Self::enqueue(&mut self.dma, ring, parameter, status, control); }
        self.doorbell(slot, dci as u32);
        let mut result = None;
        wait(|| {
            self.pump();
            if let Some(entry) = self.done.iter_mut().find(|d| d.is_some_and(|d| d.slot == slot && d.dci == dci)) { result = entry.take(); }
            result.is_some()
        });
        self.last = result.map_or(0, |d| d.code);
        match result { Some(d) if matches!(d.code, SUCCESS | SHORT_PACKET) => Ok(d.residue), Some(d) => Err(d.code), None => Err(0) }
    }

    /// A control transfer on endpoint 0 with up to 4096 bytes of data in the SMALL area; the bytes moved.
    pub fn control(&mut self, slot: u8, ring: &mut Ring, request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Result<usize, u32> {
        let setup = request_type as u64 | (request as u64) << 8 | (value as u64) << 16 | (index as u64) << 32 | (length as u64) << 48;
        let input = request_type & 0x80 != 0;
        let transfer_type = if length == 0 { 0 } else if input { 3 } else { 2 };
        let data = self.dma.physical(SMALL);
        let setup_trb = (setup, 8, TYPE_SETUP << 10 | IDT | transfer_type << 16);
        let status_trb = (0, 0, TYPE_STATUS << 10 | IOC | ((!input || length == 0) as u32) << 16);
        let residue = if length == 0 { self.transfer(slot, 1, ring, &[setup_trb, status_trb])? }
                      else { self.transfer(slot, 1, ring, &[setup_trb, (data, length as u32, TYPE_DATA << 10 | (input as u32) << 16), status_trb])? };
        // The status stage completes the transfer; a short data stage reports its residue on the data TRB only when it
        // asks (ISP), so the length is taken as asked, less any residue reported.
        Ok((length as u32).saturating_sub(residue) as usize)
    }

    // PORTSC of root port `port` (1-based).
    fn portsc(&self, port: usize) -> usize { self.op + 0x400 + 0x10 * (port - 1) }
    pub fn connected(&self, port: usize) -> bool { self.mmio.read32(self.portsc(port)) & 1 != 0 }
    pub fn port_status(&self, port: usize) -> u32 { self.mmio.read32(self.portsc(port)) }
    /// Clears a root port's change bits; true if the connection changed.
    pub fn acknowledge(&self, port: usize) -> bool {
        let register = self.portsc(port); let value = self.mmio.read32(register);
        if value & PORT_CHANGES != 0 { self.mmio.write32(register, (value & PORT_NEUTRAL) | (value & PORT_CHANGES)); }
        value & 1 << 17 != 0
    }
    /// Enables a connected root port (USB 3 enables itself, USB 2 is reset); its speed (1 FS, 2 LS, 3 HS, 4 SS).
    pub fn enable_port(&mut self, port: usize) -> Option<u8> {
        let register = self.portsc(port);
        let value = self.mmio.read32(register);
        if value & 1 == 0 { return None; }
        if value & PORT_PED == 0 {
            self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PR);
            if !wait(|| self.mmio.read32(register) & PORT_PRC != 0) { return None; }
            let value = self.mmio.read32(register); self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PRC);
            mind::time::sleep(20);
        }
        let value = self.mmio.read32(register);
        (value & PORT_PED != 0).then_some(((value >> 10) & 0xF) as u8)
    }

    // Input contexts: control (flags), slot, endpoints by DCI.
    pub fn input_reset(&mut self, add: u32) { self.dma.zero(INPUT, 0x2000); self.dma.write32(INPUT + 4, add); }
    pub fn slot_context(&mut self, route: u32, speed: u8, entries: u32, root_port: u8, hub: Option<(u8, u8)>, tt: Option<(u8, u8)>) {
        let slot = INPUT + self.context;
        self.dma.write32(slot, route & 0xF_FFFF | (speed as u32) << 20 | (hub.is_some() as u32) << 26 | entries << 27);
        self.dma.write32(slot + 4, (root_port as u32) << 16 | (hub.map_or(0, |h| h.0) as u32) << 24);
        self.dma.write32(slot + 8, tt.map_or(0, |(s, p)| s as u32 | (p as u32) << 8) | (hub.map_or(0, |h| h.1) as u32) << 16);
    }
    pub fn endpoint_context(&mut self, dci: u8, kind: u32, packet: u16, interval: u8, ring: Ring) {
        let at = INPUT + self.context * (1 + dci as usize);
        let dequeue = self.dma.physical(ring.page) | 1;
        let average = match kind { EP_CONTROL => 8, EP_INTERRUPT_IN | EP_INTERRUPT_OUT => packet as u32, _ => 3072 };
        self.dma.write32(at, (interval as u32) << 16);
        self.dma.write32(at + 4, 3 << 1 | kind << 3 | (packet as u32) << 16);
        self.dma.write64(at + 8, dequeue);
        let esit = if matches!(kind, EP_INTERRUPT_IN | EP_INTERRUPT_OUT) { (packet as u32) << 16 } else { 0 };
        self.dma.write32(at + 16, average | esit);
    }

    pub fn enable_slot(&mut self) -> Option<u8> { self.command(0, TYPE_ENABLE_SLOT << 10).map(|s| s as u8) }
    pub fn disable_slot(&mut self, slot: u8) {
        self.interrupts.iter_mut().filter(|i| i.as_ref().is_some_and(|i| i.slot == slot)).for_each(|i| *i = None);
        let _ = self.command(0, TYPE_DISABLE_SLOT << 10 | (slot as u32) << 24);
        self.dma.write64(DCBAA + slot as usize * 8, 0);
    }
    pub fn set_output(&mut self, slot: u8, page: usize) { let output = self.dma.physical(page); self.dma.write64(DCBAA + slot as usize * 8, output); }
    pub fn address(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_ADDRESS << 10 | (slot as u32) << 24).map(drop) }
    pub fn configure(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_CONFIGURE << 10 | (slot as u32) << 24).map(drop) }
    pub fn evaluate(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_EVALUATE << 10 | (slot as u32) << 24).map(drop) }

    /// After a stall: reset the endpoint and move its dequeue pointer past what it left on the ring.
    pub fn recover(&mut self, slot: u8, dci: u8, ring: &Ring) {
        let _ = self.command(0, TYPE_RESET_ENDPOINT << 10 | (dci as u32) << 16 | (slot as u32) << 24);
        let dequeue = self.dma.physical(ring.page + ring.index * TRB) | ring.cycle as u64;
        let _ = self.command(dequeue, TYPE_SET_DEQUEUE << 10 | (dci as u32) << 16 | (slot as u32) << 24);
    }

    /// Starts polling interrupt IN endpoint `dci` on its ring with `ARMED` transfers of up to a packet each.
    pub fn arm(&mut self, slot: u8, dci: u8, ring: Ring, packet: u16) -> bool {
        let Some(index) = self.interrupts.iter().position(Option::is_none) else { return false };
        let length = (packet as u32).min(REPORT_BYTES as u32).max(1);
        self.interrupts[index] = Some(Interrupt { slot, dci, ring, length, queue: [[0; REPORT_BYTES]; QUEUE], lengths: [0; QUEUE], head: 0, count: 0, failed: false });
        for _ in 0..ARMED { self.queue_report(index); }
        self.doorbell(slot, dci as u32);
        true
    }
    pub fn armed(&self, slot: u8, dci: u8) -> Option<usize> { self.interrupts.iter().position(|i| i.as_ref().is_some_and(|i| i.slot == slot && i.dci == dci)) }
    /// Drops the reports queued so far; the endpoint keeps polling (its transfers stay on the ring) for the next owner.
    pub fn forget(&mut self, slot: u8, dci: u8) { if let Some(i) = self.armed(slot, dci) { if let Some(e) = self.interrupts[i].as_mut() { e.count = 0; } } }

    // One more transfer on interrupt endpoint `index`, into the report buffer its ring position names.
    fn queue_report(&mut self, index: usize) {
        let Some(interrupt) = self.interrupts[index].as_mut() else { return };
        let buffer = interrupt.ring.page + REPORT_AREA + (interrupt.ring.index % REPORT_BUFFERS) * REPORT_BYTES;
        let address = self.dma.physical(buffer);
        Self::enqueue(&mut self.dma, &mut interrupt.ring, address, interrupt.length, TYPE_NORMAL << 10 | IOC | ISP);
    }

    // A completed interrupt transfer: its report goes to the queue (the oldest is dropped when full) and the ring gets
    // a transfer in its place; an error stops the endpoint (the device went away or stalled it).
    fn report(&mut self, index: usize, pointer: u64, status: u32) {
        let Some(interrupt) = self.interrupts[index].as_mut() else { return };
        let code = status >> 24;
        if !matches!(code, SUCCESS | SHORT_PACKET) { interrupt.failed = true; return; }
        let at = (pointer.wrapping_sub(self.dma.physical(interrupt.ring.page)) / TRB as u64) as usize;
        let buffer = interrupt.ring.page + REPORT_AREA + (at % REPORT_BUFFERS) * REPORT_BYTES;
        let length = interrupt.length.saturating_sub(status & 0xFF_FFFF) as usize;
        let slot = (interrupt.head + interrupt.count) % QUEUE;
        if interrupt.count == QUEUE { interrupt.head = (interrupt.head + 1) % QUEUE; } else { interrupt.count += 1; }
        let bytes = self.dma.bytes(buffer, length);
        interrupt.queue[slot][..length].copy_from_slice(bytes);
        interrupt.lengths[slot] = length as u8;
        let (slot, dci) = (interrupt.slot, interrupt.dci);
        self.queue_report(index);
        self.doorbell(slot, dci as u32);
    }

    /// The queued reports of interrupt endpoint `index`, oldest first; Err if it failed.
    pub fn take_reports(&mut self, index: usize, mut each: impl FnMut(&[u8])) -> Result<usize, ()> {
        let Some(interrupt) = self.interrupts[index].as_mut() else { return Err(()) };
        let count = interrupt.count;
        for n in 0..count { let at = (interrupt.head + n) % QUEUE; each(&interrupt.queue[at][..interrupt.lengths[at] as usize]); }
        interrupt.head = (interrupt.head + count) % QUEUE; interrupt.count = 0;
        if interrupt.failed && count == 0 { return Err(()); }
        Ok(count)
    }
}
