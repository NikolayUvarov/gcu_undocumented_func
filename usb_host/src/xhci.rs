// xHCI: command and event rings, a pool of DMA pages for device contexts and transfer rings, input contexts, synchronous
// control and bulk transfers, interrupt IN endpoints kept polling with their reports queued (issue 164), and
// isochronous endpoints streaming, IN and OUT (000-DRV-0073).
use core::sync::atomic::{fence, Ordering};
use crate::spill::Spill;
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
// Scratchpad pages beyond MAX_SCRATCH come from the pool (211-DRV-0040: the DEXP's controller asks 34), at most half of it.
const POOL_SCRATCH: usize = POOL_PAGES / 2;

pub const TYPE_NORMAL: u32 = 1; const TYPE_SETUP: u32 = 2; const TYPE_DATA: u32 = 3; const TYPE_STATUS: u32 = 4; const TYPE_LINK: u32 = 6;
const TYPE_ENABLE_SLOT: u32 = 9; const TYPE_DISABLE_SLOT: u32 = 10; const TYPE_ADDRESS: u32 = 11; const TYPE_CONFIGURE: u32 = 12;
const TYPE_EVALUATE: u32 = 13; const TYPE_RESET_ENDPOINT: u32 = 14; const TYPE_STOP_ENDPOINT: u32 = 15; const TYPE_SET_DEQUEUE: u32 = 16;
// The Isoch transfer TRB is type 5 (xHCI 1.2, table 6-91); 17, which stood here, is the Reset Device command's.
const TYPE_ISOCH: u32 = 5;
// An isochronous TD names the 1 ms frame it runs in (Frame ID, bits 20-30), as Linux does on a host with contiguous frame
// IDs.
const FRAME_ID_SHIFT: u32 = 20;
// The first TD of a stream, and the first after its ring ran dry, goes this many microframes ahead of the controller.
const FIRST_AHEAD: u32 = 8 * 8;
const EVENT_TRANSFER: u32 = 32; const EVENT_COMMAND: u32 = 33;
pub const IOC: u32 = 1 << 5; const IDT: u32 = 1 << 6; pub const ISP: u32 = 1 << 2;
pub const SUCCESS: u32 = 1; pub const STALL: u32 = 6; pub const SHORT_PACKET: u32 = 13;
const RING_UNDERRUN: u32 = 14; const RING_OVERRUN: u32 = 15; const MISSED_SERVICE: u32 = 23;
// PORTSC bits written back without side effects (no RW1C bits, no PED).
const PORT_NEUTRAL: u32 = 0x4E00_FFE9; const PORT_PED: u32 = 1 << 1; const PORT_PR: u32 = 1 << 4; const PORT_CHANGES: u32 = 0x7F << 17; const PORT_PRC: u32 = 1 << 21;

/// Endpoint context types.
pub const EP_ISOCH_OUT: u32 = 1; pub const EP_ISOCH_IN: u32 = 5; pub const EP_BULK_OUT: u32 = 2; pub const EP_INTERRUPT_OUT: u32 = 3; pub const EP_CONTROL: u32 = 4; pub const EP_BULK_IN: u32 = 6; pub const EP_INTERRUPT_IN: u32 = 7;

#[derive(Clone, Copy, Default)]
pub struct Ring { pub page: usize, trbs: usize, index: usize, cycle: u32 }

// A synchronous transfer's completion: slot, endpoint (DCI), completion code, residue.
#[derive(Clone, Copy)]
struct Done { slot: u8, dci: u8, code: u32, residue: u32 }

const QUEUE: usize = 8;
/// An interrupt IN endpoint being polled and the reports it gave since they were last taken.
// `lossless` (a bulk IN endpoint, 800-DRV-0034; a Bluetooth controller's events, 700-DRV-0047): a transfer goes back on the ring only while the queue has room for its
// packet, so the device holds what does not fit (NAK) instead of the oldest report being dropped; `in_flight` counts the
// transfers on the ring.
pub struct Interrupt { pub slot: u8, pub dci: u8, ring: Ring, length: u32, queue: [[u8; REPORT_BYTES]; QUEUE], lengths: [u8; QUEUE], head: usize, count: usize, pub failed: bool, reported: bool, lossless: bool, in_flight: usize }

const MAX_INTERRUPTS: usize = 8; const MAX_DONE: usize = 8;

/// An isochronous stream's TDs: each one TRB, one service interval's packet, in a buffer of its own, in ISO_PAGES pages.
/// A bulk stream's (158-DRV-0082): each a page, as many as leave POOL_KEPT pages of the pool free, BULK_LEAST at least.
pub const STREAM_TDS: usize = 32; const STREAM_PAGES: usize = 32; const ISO_PAGES: usize = 4; const MAX_STREAMS: usize = 4;
const POOL_KEPT: usize = 12; const BULK_LEAST: usize = 8;
/// The bytes a bulk stream's spill holds at most beyond its TDs (158-DRV-0082): eight times the 32 pages on its ring.
const SPILL_BYTES: usize = 1 << 20;
/// The times a stream's ring stood empty that are said a line each (000-DRV-0094, 158-DRV-0082); the rest are counted.
const DRY_SAID: u32 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Td { Free, Armed(u64), Done(u16, bool) } // Armed: its TRB's address; Done (IN): the bytes received, and an error

/// An isochronous endpoint streaming (000-DRV-0073). An IN stream keeps every TD free of a packet on its ring; the
/// packets come back in order and wait until they are taken, then their TDs go on the ring again. An OUT stream puts each
/// packet it is given on the ring while a TD is free; a TD is free again once the controller sent it. Each TD starts as
/// soon as possible after the one before (SIA), so a ring that ran dry (a missed packet, under- or overrun) starts
/// again with the next TD. The streams of an interface end with its setting.
/// A bulk IN stream (158-DRV-0082, `bulk`) keeps its TDs on the endpoint's own ring, a page each, and gives them in order,
/// an empty one too; one that ended in error halts the endpoint (`halted`) until the owner's next take resets it. Its
/// finished TDs are moved into `spill` every STREAM_MS and go back on the ring at once, so the camera waits on the client
/// only when SPILL_BYTES wait.
pub struct Stream {
    pub slot: u8, pub dci: u8, pub interface: u8, input: bool, pub bulk: bool, halted: bool, ring: Ring, packet: usize, size: usize, pages: [usize; STREAM_PAGES],
    tds: usize, state: [Td; STREAM_TDS], next: usize, oldest: usize,
    /// TDs the controller missed, times its ring ran dry, packets that came with an error.
    missed: u32, dry: u32, damaged: u32, moved: u64, reported: bool,
    // When a TD last ended, the completion codes already logged (a bit each, below 64), and when a stall was last said.
    ended_ms: u64, codes: u64, said_ms: u64,
    // The microframe the next TD runs in (MFINDEX's count, 14 bits: 2048 frames), None until the first TD or after the
    // ring ran dry; the microframes between two TDs (2^interval).
    uframe: Option<u32>, step: u32,
    spill: Option<Spill>,
    // When a TD last ended and when the client last took or put (monotonic ns); a dry ring not yet refilled: when it was
    // said (uptime ms, ns), those two then, and the transfer usb_host waited for (000-DRV-0094); the time with no TD on
    // the ring in all and at most, and the times said.
    end_ns: u64, call_ns: u64, dry_seen: Option<Dry>, empty_ns: u64, longest_ns: u64, said: u32,
}

// A ring that ran dry, until its next TD (000-DRV-0094).
#[derive(Clone, Copy)]
struct Dry { at_ms: u64, at_ns: u64, end_ns: u64, call_ns: u64, wait: Option<Wait> }

// A transfer or command usb_host waits for: its slot and DCI (0: a command), and since when (monotonic ns).
#[derive(Clone, Copy)]
struct Wait { slot: u8, dci: u8, since_ns: u64 }

// A time in ms with a tenth.
struct Ms(u64);
impl core::fmt::Display for Ms {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { write!(f, "{}.{}", self.0 / 1_000_000, self.0 / 100_000 % 10) }
}
// What usb_host waited for when a ring stood empty, for the log: the wait, until when (its end, or the time said), and
// whether it had ended.
struct During(Option<Wait>, u64, bool);
impl core::fmt::Display for During {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let verb = if self.2 { "WAITED" } else { "WAITING" };
        match self.0 {
            Some(w) if w.dci == 0 => write!(f, "{} ON A COMMAND FOR {} MS", verb, Ms(self.1.saturating_sub(w.since_ns))),
            Some(w) => write!(f, "{} ON SLOT {} ENDPOINT {} FOR {} MS", verb, w.slot, w.dci, Ms(self.1.saturating_sub(w.since_ns))),
            None => write!(f, "NOT WAITING"),
        }
    }
}

impl Stream {
    // TD k's buffer: its page and offset.
    fn buffer(&self, k: usize) -> usize { self.pages[k * self.size / PAGE] + k * self.size % PAGE }
}

pub struct Xhci {
    mmio: Mmio, pub dma: Dma, op: usize, runtime: usize, doorbells: usize, context: usize, ports: usize, pub slots: usize,
    command: Ring, event: Ring, free: u64, completion: Option<(u32, u32)>, done: [Option<Done>; MAX_DONE], interrupts: [Option<Interrupt>; MAX_INTERRUPTS],
    streams: [Option<Stream>; MAX_STREAMS],
    pub last: u32, // completion code of the last command or transfer (0: no answer), for reports
    /// A command had no answer: the controller is set up again, and no command waits on it until then (000-DRV-0075).
    pub stuck: bool,
    pub number: u8, // which of usb_host's xHCI controllers, for the log (213-DRV-0048)
    // The transfer or command being waited for, and the last one waited for and when it ended (000-DRV-0094).
    wait: Option<Wait>, waited: Option<(Wait, u64)>,
}

/// A log line's controller: nothing for the first, so its lines read as before; `XHCI n: ` for a further one.
pub struct Tag(pub u8);
impl core::fmt::Display for Tag {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { if self.0 == 0 { Ok(()) } else { write!(f, "XHCI {}: ", self.0) } }
}

/// Busy-poll, then sleep: QEMU completes commands at once, transfers asynchronously; about 30 s in all.
pub fn wait(done: impl FnMut() -> bool) -> bool { wait_for(4_000, done) }

/// The attach debounce (000-DRV-0077): a new connection must hold this long, looked at in these steps, within the most.
pub const DEBOUNCE_MS: u32 = 100;
pub const DEBOUNCE_STEP_MS: u32 = 25;
pub const DEBOUNCE_MAX_MS: u32 = 1_500;
// After a root port's reset, before its device is addressed: USB 2.0's 10 ms recovery and more; Linux's 50 ms on a retry.
const RESET_RECOVERY_MS: usize = 20; const RESET_RECOVERY_AGAIN_MS: usize = 50;

// Polls `done`: without pause for the first 20 ms (most transfers end within a few frames; when each wait began with a
// sleep, the MacBook Pro's mouse moved once a second, 211-DRV-0017), then every 10 ms; `attempts` 1500 is about 5 s,
// 4000 about 30 s.
pub(crate) fn wait_for(attempts: usize, mut done: impl FnMut() -> bool) -> bool {
    let start = mind::time::monotonic_ns();
    while mind::time::monotonic_ns().saturating_sub(start) < SPIN_NS { if done() { return true; } core::hint::spin_loop(); }
    for _ in 0..attempts.saturating_sub(1_000) { if done() { return true; } mind::time::sleep(10); }
    done()
}
const SPIN_NS: u64 = 20_000_000;

impl Xhci {
    pub fn init(mmio: Mmio, mut dma: Dma, number: u8) -> Option<Self> {
        if dma.len() < DMA_BYTES { mind::println!("[USB] {}DMA REGION TOO SMALL", Tag(number)); return None; }
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
        // One that does not halt (it left a command unanswered, 000-DRV-0075) is reset all the same: nothing else brings
        // it back. The specification gives a halt 16 ms.
        if !wait_for(1_500, || mmio.read32(op + 4) & 1 != 0) { mind::println!("[USB] {}THE CONTROLLER DID NOT HALT (USBSTS {:08X}): RESET ALL THE SAME", Tag(number), mmio.read32(op + 4)); }
        mmio.write32(op, 2);
        if !wait(|| mmio.read32(op) & 2 == 0 && mmio.read32(op + 4) & 1 << 11 == 0) { return None; }
        let scratch = ((hcs2 >> 21 & 0x1F) << 5 | hcs2 >> 27) as usize;
        if scratch > MAX_SCRATCH + POOL_SCRATCH { mind::println!("[USB] {}{} SCRATCHPAD PAGES NEEDED, {} AVAILABLE", Tag(number), scratch, MAX_SCRATCH + POOL_SCRATCH); return None; }
        dma.zero(0, POOL);
        let pooled = scratch.saturating_sub(MAX_SCRATCH);
        for i in 0..scratch {
            let offset = if i < MAX_SCRATCH { SCRATCH_PAGES + i * PAGE } else { POOL + (i - MAX_SCRATCH) * PAGE };
            if i >= MAX_SCRATCH { dma.zero(offset, PAGE); }
            let page = dma.physical(offset); dma.write64(SCRATCH_ARRAY + i * 8, page);
        }
        if pooled > 0 { mind::println!("[USB] {}{} SCRATCHPAD PAGES, {} OF THEM FROM THE POOL", Tag(number), scratch, pooled); }
        if scratch > 0 { let array = dma.physical(SCRATCH_ARRAY); dma.write64(DCBAA, array); }
        let slots = (hcs1 & 0xFF).min(16) as usize;
        mmio.write32(op + 0x38, slots as u32);
        mmio.write64(op + 0x30, dma.physical(DCBAA));
        let mut xhci = Self { mmio, dma, op, runtime, doorbells, context, ports: (hcs1 >> 24) as usize, slots,
            command: Ring { page: COMMAND_RING, trbs: RING_TRBS, index: 0, cycle: 1 }, event: Ring { page: EVENT_RING, trbs: RING_TRBS, index: 0, cycle: 1 },
            free: u64::MAX << pooled, completion: None, done: [None; MAX_DONE], interrupts: [const { None }; MAX_INTERRUPTS], streams: [const { None }; MAX_STREAMS], last: 0, stuck: false, number, wait: None, waited: None };
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

    /// USBSTS: bit 0 halted, bit 2 a host system error, bit 12 a host controller error.
    pub fn status(&self) -> u32 { self.mmio.read32(self.op + 4) }
    /// Whether the controller stopped by itself, with a host system or controller error (000-DRV-0073: the Ryzen's
    /// second xHCI did, on its first isochronous TDs).
    pub fn failed(&self) -> bool { let s = self.status(); s & 1 != 0 && s & (1 << 2 | 1 << 12) != 0 }
    /// The controller set up anew on its registers and DMA region, after it failed: every device on it is gone.
    pub fn start_again(self) -> Option<Self> { let Xhci { mmio, dma, number, .. } = self; Self::init(mmio, dma, number) }

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
                    if let Some(index) = self.streams.iter().position(|t| t.as_ref().is_some_and(|t| t.slot == slot && t.dci == dci)) {
                        self.stream_event(index, pointer, status);
                    } else if let Some(index) = self.interrupts.iter().position(|i| i.as_ref().is_some_and(|i| i.slot == slot && i.dci == dci)) {
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

    // A command and its completion: the slot it names, on success. One without an answer in 5 s, as Linux waits, marks
    // the controller stuck: the Ryzen's AMD xHCI stopped answering at a device's set-up there (000-DRV-0075).
    fn command(&mut self, parameter: u64, control: u32) -> Option<u32> {
        if self.stuck { self.last = 0; return None; }
        self.completion = None;
        Self::enqueue(&mut self.dma, &mut self.command, parameter, 0, control);
        self.doorbell(0, 0);
        let mut result = None;
        self.begin_wait(control >> 24, 0);
        wait_for(1_500, || { self.pump(); result = self.completion.take(); result.is_some() });
        self.end_wait();
        self.last = result.map_or(0, |r| r.0);
        if result.is_none() {
            self.stuck = true;
            mind::println!("[USB] {}COMMAND {} HAD NO ANSWER IN 5 S (USBSTS {:08X}, CRCR {:08X})", Tag(self.number), control >> 10 & 0x3F, self.status(), self.mmio.read32(self.op + 0x18));
        }
        let (code, slot) = result?;
        (code == SUCCESS).then_some(slot)
    }

    // A wait for a command (`dci` 0) or a transfer begins and ends, for what a ring that stood empty meanwhile says.
    fn begin_wait(&mut self, slot: u32, dci: u8) { self.wait = Some(Wait { slot: slot as u8, dci, since_ns: mind::time::monotonic_ns() }); }
    fn end_wait(&mut self) { self.waited = self.wait.take().map(|w| (w, mind::time::monotonic_ns())).or(self.waited); }

    /// TRBs on `ring` of endpoint `dci`, then its completion: Ok(residue), or Err(completion code; 0: no answer).
    pub fn transfer(&mut self, slot: u8, dci: u8, ring: &mut Ring, trbs: &[(u64, u32, u32)]) -> Result<u32, u32> { self.transfer_for(4_000, slot, dci, ring, trbs) }

    fn transfer_for(&mut self, attempts: usize, slot: u8, dci: u8, ring: &mut Ring, trbs: &[(u64, u32, u32)]) -> Result<u32, u32> {
        self.done.iter_mut().filter(|d| d.is_some_and(|d| d.slot == slot && d.dci == dci)).for_each(|d| *d = None); // stale ones
        for &(parameter, status, control) in trbs { Self::enqueue(&mut self.dma, ring, parameter, status, control); }
        self.doorbell(slot, dci as u32);
        self.completion_for(attempts, slot, dci)
    }

    // The next completion on endpoint `dci` of `slot`: Ok(residue), or Err(completion code; 0: no answer).
    fn completion_for(&mut self, attempts: usize, slot: u8, dci: u8) -> Result<u32, u32> {
        let mut result = None;
        self.begin_wait(slot as u32, dci);
        wait_for(attempts, || {
            self.pump();
            if let Some(entry) = self.done.iter_mut().find(|d| d.is_some_and(|d| d.slot == slot && d.dci == dci)) { result = entry.take(); }
            result.is_some()
        });
        self.end_wait();
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
        // At most 5 s (USB 2.0 9.2.6.4). A device that stalls a request (SET_IDLE, often) halts endpoint 0 on the
        // controller, and one that does not answer leaves it running: either way it is reset to after this transfer, or
        // every later request would wait in vain (211-DRV-0003).
        let trbs = [setup_trb, (data, length as u32, TYPE_DATA << 10 | (input as u32) << 16 | if input { ISP } else { 0 }), status_trb];
        let result = if length == 0 { self.transfer_for(1_500, slot, 1, ring, &[setup_trb, status_trb]) } else { self.transfer_for(1_500, slot, 1, ring, &trbs) };
        // The transfer's own completion code stays for the report: the commands that put the endpoint right have theirs.
        let residue = result.inspect_err(|&code| { self.restart(slot, 1, ring, code != 0); self.last = code; })?;
        // A short IN data stage reports its residue on the data TRB (ISP), and the status stage completes after it. The
        // length is what came, not what was asked: a report descriptor read with room to spare came with the bytes of
        // earlier transfers after it, and a mouse's layout was lost (211-DRV-0008).
        if self.last == SHORT_PACKET { self.completion_for(1_500, slot, 1).inspect_err(|&code| { self.restart(slot, 1, ring, code != 0); self.last = code; })?; }
        Ok((length as u32).saturating_sub(residue) as usize)
    }

    // PORTSC of root port `port` (1-based).
    fn portsc(&self, port: usize) -> usize { self.op + 0x400 + 0x10 * (port - 1) }
    pub fn connected(&self, port: usize) -> bool { self.mmio.read32(self.portsc(port)) & 1 != 0 }
    /// Whether root port `port`'s new connection holds for 100 ms on end, looked at every 25 ms for at most 1.5 s: USB 2.0
    /// 7.1.7.3's attach debounce (TATTDB), as Linux's `hub_port_debounce`. A device that has only just powered up is not
    /// reset and addressed at once (000-DRV-0077).
    pub fn debounced(&self, port: usize) -> bool {
        let (mut stable, mut waited) = (0, 0);
        while waited < DEBOUNCE_MAX_MS {
            mind::time::sleep(DEBOUNCE_STEP_MS as usize); waited += DEBOUNCE_STEP_MS;
            if self.connected(port) { stable += DEBOUNCE_STEP_MS; if stable >= DEBOUNCE_MS { return true; } } else { stable = 0; }
        }
        false
    }
    pub fn port_status(&self, port: usize) -> u32 { self.mmio.read32(self.portsc(port)) }
    /// Clears a root port's change bits; true if the connection changed.
    pub fn acknowledge(&self, port: usize) -> bool {
        let register = self.portsc(port); let value = self.mmio.read32(register);
        if value & PORT_CHANGES != 0 { self.mmio.write32(register, (value & PORT_NEUTRAL) | (value & PORT_CHANGES)); }
        value & 1 << 17 != 0
    }
    /// Enables a connected root port (USB 3 enables itself, USB 2 is reset); its speed (1 FS, 2 LS, 3 HS, 4 SS). `again`,
    /// a further try at its device, resets an enabled port too and gives the device Linux's 50 ms after it: the try
    /// before left the device addressed or half set up, and it answers at address 0 only after a reset (000-DRV-0079).
    pub fn enable_port(&mut self, port: usize, again: bool) -> Option<u8> {
        let register = self.portsc(port);
        let value = self.mmio.read32(register);
        if value & 1 == 0 { return None; }
        if value & PORT_PED == 0 || again {
            self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PR);
            if !wait(|| self.mmio.read32(register) & PORT_PRC != 0) { return None; }
            let value = self.mmio.read32(register); self.mmio.write32(register, (value & PORT_NEUTRAL) | PORT_PRC);
            mind::time::sleep(if again { RESET_RECOVERY_AGAIN_MS } else { RESET_RECOVERY_MS });
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

    /// An isochronous endpoint's context (000-DRV-0073): no error count; a high-speed endpoint's extra transactions
    /// (wMaxPacketSize bits 11-12) as its burst; interval as 2^n units of 125 µs.
    pub fn endpoint_context_isochronous(&mut self, dci: u8, input: bool, packet: u16, interval: u8, ring: Ring) {
        let at = INPUT + self.context * (1 + dci as usize);
        let (size, burst) = ((packet & 0x7FF) as u32, (packet >> 11 & 3) as u32);
        let esit = size * (burst + 1);
        self.dma.write32(at, (interval as u32) << 16);
        self.dma.write32(at + 4, (if input { EP_ISOCH_IN } else { EP_ISOCH_OUT }) << 3 | burst << 8 | size << 16);
        let dequeue = self.dma.physical(ring.page) | 1;
        self.dma.write64(at + 8, dequeue);
        self.dma.write32(at + 16, esit | (esit & 0xFFFF) << 16);
    }
    /// The input slot context as the device's output one is, with `entries` (Linux's xhci_slot_copy): a Configure
    /// Endpoint that adds endpoints to a running device changes nothing else of its slot.
    pub fn slot_context_from(&mut self, output: usize, entries: u32) {
        let slot = INPUT + self.context;
        for i in 0..4 { let v = self.dma.read32(output + 4 * i); self.dma.write32(slot + 4 * i, v); }
        let first = self.dma.read32(slot) & !(0x1F << 27) | entries << 27;
        self.dma.write32(slot, first);
    }
    /// The raw words of the input context's control, slot and endpoint `dci` contexts and of the output's endpoint
    /// `dci` context, for the log (000-DRV-0073: compared with Linux's on the same controller).
    pub fn context_words(&mut self, output: usize, dci: u8) -> [u32; 18] {
        let mut w = [0u32; 18];
        for i in 0..2 { w[i] = self.dma.read32(INPUT + 4 * i); }
        for i in 0..4 { w[2 + i] = self.dma.read32(INPUT + self.context + 4 * i); }
        for i in 0..6 { w[6 + i] = self.dma.read32(INPUT + self.context * (1 + dci as usize) + 4 * i); }
        for i in 0..6 { w[12 + i] = self.dma.read32(output + self.context * dci as usize + 4 * i); }
        w
    }
    /// An endpoint's state in the device's output context: 1 running, 2 halted, 3 stopped, 4 error.
    pub fn endpoint_state(&mut self, output: usize, dci: u8) -> u32 { self.dma.read32(output + self.context * dci as usize) & 7 }
    /// The endpoints a Configure Endpoint drops (input control context's drop flags).
    pub fn input_drop(&mut self, drop: u32) { self.dma.write32(INPUT, drop); }

    /// A new stream of isochronous endpoint `dci` of `slot`, packets of up to `packet` bytes (with a high-speed
    /// endpoint's extra transactions); its ring, for the endpoint's context. None when the pool has no room.
    pub fn new_stream(&mut self, slot: u8, dci: u8, interface: u8, input: bool, packet: u16, interval: u8) -> Option<Ring> {
        let at = self.streams.iter().position(Option::is_none)?;
        let bytes = (packet & 0x7FF) as usize * ((packet >> 11 & 3) as usize + 1);
        let size = bytes.max(64).next_power_of_two().min(PAGE);
        if bytes > PAGE { return None; }
        let tds = (ISO_PAGES * PAGE / size).min(STREAM_TDS);
        let ring = self.ring(false)?;
        let mut pages = [0usize; STREAM_PAGES];
        for n in 0..(tds * size).div_ceil(PAGE) {
            match self.alloc() { Some(page) => pages[n] = page, None => { pages.iter().filter(|&&p| p != 0).for_each(|&p| self.release(p)); self.release(ring.page); return None } }
        }
        let now = mind::time::uptime_ms() as u64;
        self.streams[at] = Some(Stream { slot, dci, interface, input, bulk: false, halted: false, ring, packet: bytes, size, pages, tds, state: [Td::Free; STREAM_TDS], next: 0, oldest: 0,
            missed: 0, dry: 0, damaged: 0, moved: 0, reported: false, ended_ms: now, codes: 0, said_ms: 0, uframe: None, step: 1 << interval.min(15), spill: None,
            end_ns: mind::time::monotonic_ns(), call_ns: mind::time::monotonic_ns(), dry_seen: None, empty_ns: 0, longest_ns: 0, said: 0 });
        Some(ring)
    }
    /// A bulk IN stream on endpoint `dci` of `slot` (158-DRV-0082), on `ring`, the endpoint's own, which its context
    /// already names: its TDs of a page each go on it at once. The TDs it got, or None without room.
    pub fn new_bulk_stream(&mut self, slot: u8, dci: u8, interface: u8, ring: Ring) -> Option<usize> {
        let at = self.streams.iter().position(Option::is_none)?;
        let tds = (self.free.count_ones() as usize).saturating_sub(POOL_KEPT).min(STREAM_TDS);
        if tds < BULK_LEAST { return None; }
        let mut pages = [0usize; STREAM_PAGES];
        for n in 0..tds {
            match self.alloc() { Some(page) => pages[n] = page, None => { pages[..n].iter().for_each(|&p| self.release(p)); return None } }
        }
        let now = mind::time::uptime_ms() as u64;
        self.streams[at] = Some(Stream { slot, dci, interface, input: true, bulk: true, halted: false, ring, packet: PAGE, size: PAGE, pages, tds, state: [Td::Free; STREAM_TDS],
            next: 0, oldest: 0, missed: 0, dry: 0, damaged: 0, moved: 0, reported: false, ended_ms: now, codes: 0, said_ms: 0, uframe: None, step: 1, spill: Some(Spill::new(SPILL_BYTES)),
            end_ns: mind::time::monotonic_ns(), call_ns: mind::time::monotonic_ns(), dry_seen: None, empty_ns: 0, longest_ns: 0, said: 0 });
        self.take_stream(at, &mut []);
        Some(tds)
    }
    /// Ends the bulk stream of endpoint `dci` of `slot`: the endpoint stopped (reset if it halted), its dequeue pointer
    /// at its ring's next TRB, the TDs' pages back to the pool. Its ring, for the endpoint's later transfers.
    pub fn end_bulk_stream(&mut self, slot: u8, dci: u8) -> Option<Ring> {
        let at = self.stream(slot, dci).filter(|&at| self.streams[at].as_ref().is_some_and(|t| t.bulk))?;
        let t = self.streams[at].take()?;
        self.restart(slot, dci, &t.ring, t.halted);
        t.pages[..t.tds].iter().for_each(|&p| self.release(p));
        let (most, refused, left) = t.spill.as_ref().map_or((0, 0, 0), |s| (s.most, s.refused, s.len().0));
        mind::println!("[USB] {}SLOT {} ENDPOINT {}: BULK STREAM ENDED: {} BYTES, {} WITH ERRORS; ITS SPILL HELD {} BYTES AT MOST, FULL {} TIMES, {} LEFT UNREAD", Tag(self.number), slot, dci, t.moved, t.damaged, most, refused, left);
        mind::println!("[USB] {}SLOT {} ENDPOINT {}: ITS RING STOOD EMPTY {} TIMES, {} MS IN ALL, AT MOST {} MS", Tag(self.number), slot, dci, t.dry, Ms(t.empty_ns), Ms(t.longest_ns));
        Some(t.ring)
    }
    /// Whether bulk stream `index` halted on an error: its owner then clears the device's halt and calls `recover_bulk`.
    pub fn bulk_halted(&self, index: usize) -> bool { self.streams[index].as_ref().is_some_and(|t| t.bulk && t.halted) }
    /// Bulk stream `index` after an error: the endpoint reset, its dequeue pointer past what is on its ring, and the TDs
    /// still there free again; the next take puts them back.
    pub fn recover_bulk(&mut self, index: usize) {
        let Some(t) = self.streams[index].as_ref().filter(|t| t.bulk && t.halted) else { return };
        let (slot, dci, ring) = (t.slot, t.dci, t.ring);
        self.recover(slot, dci, &ring);
        let Some(t) = self.streams[index].as_mut() else { return };
        for k in 0..t.tds { if matches!(t.state[k], Td::Armed(_)) { t.state[k] = Td::Free; } }
        t.next = t.oldest;
        t.halted = false;
    }
    /// The stream of endpoint `dci` of `slot`.
    pub fn stream(&self, slot: u8, dci: u8) -> Option<usize> { self.streams.iter().position(|t| t.as_ref().is_some_and(|t| t.slot == slot && t.dci == dci)) }
    /// The DCIs (a bit each) of the streams of `slot`'s interface `interface`, or of all its interfaces.
    /// A bulk stream (158-DRV-0082) is not among them: its endpoint is the interface's own, configured with it.
    pub fn streams_of(&self, slot: u8, interface: Option<u8>) -> u32 {
        self.streams.iter().flatten().filter(|t| t.slot == slot && !t.bulk && interface.is_none_or(|i| t.interface == i)).fold(0, |m, t| m | 1 << t.dci)
    }
    /// The DCIs (a bit each) of the bulk streams of `slot`'s interface `interface` (158-DRV-0082).
    pub fn bulk_streams_of(&self, slot: u8, interface: u8) -> u32 {
        self.streams.iter().flatten().filter(|t| t.slot == slot && t.bulk && t.interface == interface).fold(0, |m, t| m | 1 << t.dci)
    }
    /// Ends the streams of `slot`'s interface `interface` (all with None): their pages back to the pool. The endpoints
    /// themselves are dropped by the caller's Configure Endpoint, or with the slot.
    pub fn end_streams(&mut self, slot: u8, interface: Option<u8>) {
        for at in 0..MAX_STREAMS {
            // A bulk stream ends by end_bulk_stream, which keeps its ring, the endpoint's own; here only with its slot.
            let Some(t) = self.streams[at].as_ref().filter(|t| t.slot == slot && interface.is_none_or(|i| t.interface == i) && (interface.is_none() || !t.bulk)) else { continue };
            let (ring, pages, bulk) = (if t.bulk { 0 } else { t.ring.page }, t.pages, t.bulk);
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: STREAM ENDED: {} {}, {} MISSED, {} TIMES DRY ({} MS WITHOUT A TD, AT MOST {}), {} WITH ERRORS", Tag(self.number), t.slot, t.dci,
                t.moved, if t.input { "BYTES" } else { "PACKETS SENT" }, t.missed, t.dry, Ms(t.empty_ns), Ms(t.longest_ns), t.damaged);
            if !bulk { self.release(ring); }
            pages.iter().filter(|&&p| p != 0).for_each(|&p| self.release(p));
            self.streams[at] = None;
        }
    }

    // A TD of stream `index` ended: an IN one's packet waits to be taken; an OUT one's TD is free. A ring that ran dry has
    // no TD to name.
    fn stream_event(&mut self, index: usize, pointer: u64, status: u32) {
        let Some(t) = self.streams[index].as_mut() else { return };
        let code = status >> 24;
        // Each completion code other than success is said once a stream, with what the controller's status says.
        if !matches!(code, SUCCESS | SHORT_PACKET) && code < 64 && t.codes & 1 << code == 0 {
            t.codes |= 1 << code;
            let (slot, dci, usbsts) = (t.slot, t.dci, self.mmio.read32(self.op + 4));
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: {} TD ENDED WITH COMPLETION {}; USBSTS {:08X}", Tag(self.number), slot, dci, if t.bulk { "A BULK" } else { "AN ISOCHRONOUS" }, code, usbsts);
        }
        let Some(t) = self.streams[index].as_mut() else { return };
        // A bulk TD: its bytes, or an error that halted the endpoint (the TDs after it wait until it is reset).
        if t.bulk {
            let Some(k) = (0..t.tds).find(|&k| t.state[k] == Td::Armed(pointer)) else { return };
            (t.ended_ms, t.end_ns) = (mind::time::uptime_ms() as u64, mind::time::monotonic_ns());
            if matches!(code, SUCCESS | SHORT_PACKET) {
                let got = t.packet.saturating_sub((status & 0xFF_FFFF) as usize);
                t.moved += got as u64;
                t.state[k] = Td::Done(got as u16, false);
            } else { t.damaged += 1; t.halted = true; t.state[k] = Td::Done(0, true); }
            return;
        }
        // A ring that ran dry starts again from the controller's frame with its next TD, which says how long it stood empty.
        if matches!(code, RING_UNDERRUN | RING_OVERRUN) {
            t.dry += 1; t.uframe = None;
            if t.dry_seen.is_none() { t.dry_seen = Some(Dry { at_ms: mind::time::uptime_ms() as u64, at_ns: mind::time::monotonic_ns(), end_ns: t.end_ns, call_ns: t.call_ns, wait: self.wait }); }
            return;
        }
        if code == MISSED_SERVICE { t.uframe = None; }
        (t.ended_ms, t.end_ns) = (mind::time::uptime_ms() as u64, mind::time::monotonic_ns());
        let Some(k) = (0..t.tds).find(|&k| t.state[k] == Td::Armed(pointer)) else { return };
        let good = matches!(code, SUCCESS | SHORT_PACKET);
        if code == MISSED_SERVICE { t.missed += 1; } else if !good { t.damaged += 1; }
        if !t.reported && good {
            t.reported = true;
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: FIRST ISOCHRONOUS {} DONE ({} BYTES)", Tag(self.number), t.slot, t.dci, if t.input { "PACKET" } else { "SEND" }, t.packet.saturating_sub((status & 0xFF_FFFF) as usize));
        }
        t.state[k] = if t.input {
            let got = if good { t.packet.saturating_sub((status & 0xFF_FFFF) as usize) } else { 0 };
            t.moved += got as u64;
            Td::Done(got as u16, !good && code != MISSED_SERVICE)
        } else { if good { t.moved += 1; } Td::Free };
    }

    // Puts free TD k of stream `index` on its ring with `length` bytes; its TRB's address is how its end is known.
    fn queue_td(&mut self, index: usize, k: usize, length: usize) {
        let mfindex = self.mmio.read32(self.runtime) & 0x3FFF;
        let Some(t) = self.streams[index].as_mut() else { return };
        let (buffer, input) = (t.buffer(k), t.input);
        let address = self.dma.physical(buffer);
        // A bulk TD: a Normal TRB of a page, ended early by a short packet.
        if t.bulk {
            t.state[k] = Td::Armed(Self::enqueue(&mut self.dma, &mut t.ring, address, length as u32, TYPE_NORMAL << 10 | IOC | ISP));
            return;
        }
        // Its frame: the one after the TD before, or a little ahead of the controller; one that fell behind starts anew.
        let ahead = t.uframe.filter(|u| (u.wrapping_sub(mfindex) & 0x3FFF) < 0x2000 && (u.wrapping_sub(mfindex) & 0x3FFF) >= 16);
        let uframe = ahead.unwrap_or((mfindex + FIRST_AHEAD) & 0x3FFF);
        t.uframe = Some((uframe + t.step) & 0x3FFF);
        // A dry ring's first TD again (000-DRV-0094): no TD from the last one's end until this one's frame, FIRST_AHEAD on.
        if let (None, Some(d)) = (ahead, t.dry_seen.take()) {
            let now = mind::time::monotonic_ns();
            let empty = (now + FIRST_AHEAD as u64 * 125_000).saturating_sub(d.end_ns);
            (t.empty_ns, t.longest_ns) = (t.empty_ns + empty, t.longest_ns.max(empty));
            if t.said < DRY_SAID {
                t.said += 1;
                mind::println!("[USB] {}SLOT {} ENDPOINT {}: DRY {} AT {}.{:03} S: NO TD {} MS; THE LAST {} MS BEFORE, THE CLIENT {} BEFORE, {} AFTER; {}",
                    Tag(self.number), t.slot, t.dci, t.dry, d.at_ms / 1000, d.at_ms % 1000, Ms(empty), Ms(d.at_ns.saturating_sub(d.end_ns)), Ms(d.at_ns.saturating_sub(d.call_ns)),
                    Ms(now.saturating_sub(d.at_ns)), During(d.wait, d.at_ns, false));
            }
        }
        let control = TYPE_ISOCH << 10 | IOC | ((uframe >> 3) & 0x7FF) << FRAME_ID_SHIFT | if input { ISP } else { 0 };
        let first = t.next == 0 && t.state.iter().all(|s| *s == Td::Free);
        let trb = Self::enqueue(&mut self.dma, &mut t.ring, address, length as u32, control);
        t.state[k] = Td::Armed(trb);
        // The first TD of a stream as the controller reads it, its MFINDEX and USBSTS, for the log (000-DRV-0073).
        if first {
            let (slot, dci, at) = (t.slot, t.dci, (trb - self.dma.physical(0)) as usize);
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: FIRST TD AT {:X}: {:08X} {:08X} {:08X} {:08X}; MFINDEX {:04X}; USBSTS {:08X}", Tag(self.number), slot, dci, trb,
                self.dma.read32(at), self.dma.read32(at + 4), self.dma.read32(at + 8), self.dma.read32(at + 12), mfindex, self.mmio.read32(self.op + 4));
        }
    }

    /// IN stream `index`: its packets in order into `out` (two length bytes each, bit 15 for an error, then the packet;
    /// empty ones left out), as far as they fit; their TDs and every other free one go on the ring again. The bytes written.
    pub fn take_stream(&mut self, index: usize, out: &mut [u8]) -> usize {
        let mut written = 0;
        if let Some(t) = self.streams[index].as_mut() { t.call_ns = mind::time::monotonic_ns(); }
        // A bulk stream's spill holds its oldest transfers: they come first, and the TDs' only once it is empty.
        if let Some(spill) = self.streams[index].as_mut().and_then(|t| t.spill.as_mut()) {
            written = spill.take(out);
            if !spill.is_empty() { self.refill(index); return written; }
        }
        loop {
            let Some(t) = self.streams[index].as_mut() else { return written };
            let Td::Done(length, damaged) = t.state[t.oldest] else { break };
            let length = length as usize;
            // A bulk stream's empty transfer is given too: it ends a payload.
            if length > 0 || t.bulk {
                if written + 2 + length > out.len() { break; }
                let head = length as u16 | if damaged { 0x8000 } else { 0 };
                out[written..written + 2].copy_from_slice(&head.to_le_bytes());
                let buffer = t.buffer(t.oldest);
                out[written + 2..written + 2 + length].copy_from_slice(self.dma.bytes(buffer, length));
                written += 2 + length;
            }
            let Some(t) = self.streams[index].as_mut() else { return written };
            t.state[t.oldest] = Td::Free;
            t.oldest = (t.oldest + 1) % t.tds;
        }
        self.refill(index);
        written
    }

    // Stream `index`'s free TDs back on the ring, in order, and the doorbell rung.
    fn refill(&mut self, index: usize) {
        let mut queued = false;
        // A bulk ring with none of its TDs on it stood empty from the last one's end: the camera holds its data meanwhile.
        let empty = self.streams[index].as_ref().is_some_and(|t| t.bulk && t.moved > 0 && !t.state[..t.tds].iter().any(|s| matches!(s, Td::Armed(_))));
        loop {
            let Some(t) = self.streams[index].as_ref() else { break };
            let (k, packet) = (t.next, t.packet);
            if !t.input || t.halted || t.state[k] != Td::Free { break; }
            self.queue_td(index, k, packet);
            if let Some(t) = self.streams[index].as_mut() { t.next = (k + 1) % t.tds; }
            queued = true;
        }
        if let (true, Some(t)) = (queued, self.streams[index].as_ref()) { self.doorbell(t.slot, t.dci as u32); }
        if let (true, true, Some(t)) = (queued, empty, self.streams[index].as_mut()) {
            let now = mind::time::monotonic_ns();
            let gone = now.saturating_sub(t.end_ns);
            (t.dry, t.empty_ns, t.longest_ns) = (t.dry + 1, t.empty_ns + gone, t.longest_ns.max(gone));
            if t.said < DRY_SAID {
                t.said += 1;
                // The wait usb_host is in, or the last one when it ended after the ring emptied.
                let during = match (self.wait, self.waited) {
                    (Some(w), _) => During(Some(w), now, false),
                    (None, Some((w, ended))) if ended >= t.end_ns => During(Some(w), ended, true),
                    _ => During(None, now, false),
                };
                let held = t.spill.as_ref().map_or(0, |s| s.len().0);
                let at = mind::time::uptime_ms() as u64;
                mind::println!("[USB] {}SLOT {} ENDPOINT {}: BULK RING EMPTY {} AT {}.{:03} S: NO TD {} MS, ITS SPILL HOLDING {} BYTES; {}",
                    Tag(self.number), t.slot, t.dci, t.dry, at / 1000, at % 1000, Ms(gone), held, during);
            }
        }
    }

    /// Each bulk stream's finished TDs, oldest first, moved into its spill while it has room, and their TDs back on the
    /// ring (158-DRV-0082): called every STREAM_MS, so a camera's burst does not wait for its client's next take.
    pub fn spill_streams(&mut self) {
        for index in 0..MAX_STREAMS {
            let mut moved = false;
            loop {
                let Some(t) = self.streams[index].as_mut() else { break };
                let Some(spill) = t.spill.as_mut() else { break };
                let Td::Done(length, damaged) = t.state[t.oldest] else { break };
                let buffer = t.pages[t.oldest * t.size / PAGE] + t.oldest * t.size % PAGE;
                if !spill.push(self.dma.bytes(buffer, length as usize), damaged) { break; }
                t.state[t.oldest] = Td::Free;
                t.oldest = (t.oldest + 1) % t.tds;
                moved = true;
            }
            if moved { self.refill(index); }
        }
    }

    /// OUT stream `index`: the packets in `packets` (two length bytes each, then the packet) go on the ring while TDs are
    /// free. The bytes taken; what is left waits for the client's next call.
    pub fn put_stream(&mut self, index: usize, packets: &[u8]) -> usize {
        let mut at = 0;
        if let Some(t) = self.streams[index].as_mut() { t.call_ns = mind::time::monotonic_ns(); }
        let mut queued = false;
        while at + 2 <= packets.len() {
            let Some(t) = self.streams[index].as_ref() else { break };
            let length = u16::from_le_bytes([packets[at], packets[at + 1]]) as usize & 0x7FFF;
            let k = t.next;
            if t.input || t.state[k] != Td::Free || length > t.packet || at + 2 + length > packets.len() { break; }
            let buffer = t.buffer(k);
            self.dma.bytes(buffer, length).copy_from_slice(&packets[at + 2..at + 2 + length]);
            self.queue_td(index, k, length);
            if let Some(t) = self.streams[index].as_mut() { t.next = (k + 1) % t.tds; }
            at += 2 + length;
            queued = true;
        }
        if let (true, Some(t)) = (queued, self.streams[index].as_ref()) { self.doorbell(t.slot, t.dci as u32); }
        at
    }
    /// Streams with TDs on their ring and none ended for a second: said every 5 s at most, with the controller's
    /// status (USBSTS: halted, host system error, controller error) and the TDs it holds.
    pub fn watch_streams(&mut self) {
        let now = mind::time::uptime_ms() as u64;
        let usbsts = self.mmio.read32(self.op + 4);
        for t in self.streams.iter_mut().flatten() {
            let armed = t.state[..t.tds].iter().filter(|s| matches!(s, Td::Armed(_))).count();
            if armed == 0 || now < t.ended_ms + 1000 || now < t.said_ms + 5000 { continue; }
            t.said_ms = now;
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: NO TD ENDED FOR {} MS, {} ON THE RING; USBSTS {:08X}; {} MOVED, {} MISSED, {} TIMES DRY",
                Tag(self.number), t.slot, t.dci, now - t.ended_ms, armed, usbsts, t.moved, t.missed, t.dry);
        }
    }
    /// Whether a stream runs: events are then taken every STREAM_MS, so the event ring never fills.
    pub fn streaming(&self) -> bool { self.streams.iter().any(Option::is_some) }

    pub fn enable_slot(&mut self) -> Option<u8> { self.command(0, TYPE_ENABLE_SLOT << 10).map(|s| s as u8) }
    pub fn disable_slot(&mut self, slot: u8) {
        self.interrupts.iter_mut().filter(|i| i.as_ref().is_some_and(|i| i.slot == slot)).for_each(|i| *i = None);
        self.end_streams(slot, None);
        let _ = self.command(0, TYPE_DISABLE_SLOT << 10 | (slot as u32) << 24);
        self.dma.write64(DCBAA + slot as usize * 8, 0);
    }
    pub fn set_output(&mut self, slot: u8, page: usize) { let output = self.dma.physical(page); self.dma.write64(DCBAA + slot as usize * 8, output); }
    pub fn address(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_ADDRESS << 10 | (slot as u32) << 24).map(drop) }
    pub fn configure(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_CONFIGURE << 10 | (slot as u32) << 24).map(drop) }
    pub fn evaluate(&mut self, slot: u8) -> Option<()> { let input = self.dma.physical(INPUT); self.command(input, TYPE_EVALUATE << 10 | (slot as u32) << 24).map(drop) }

    /// After a stall: reset the endpoint and move its dequeue pointer past what it left on the ring.
    pub fn recover(&mut self, slot: u8, dci: u8, ring: &Ring) { self.restart(slot, dci, ring, true) }

    /// Makes endpoint `dci` run again from `ring`'s next TRB: a halted one is reset, a running one stopped first.
    pub fn restart(&mut self, slot: u8, dci: u8, ring: &Ring, halted: bool) {
        let kind = if halted { TYPE_RESET_ENDPOINT } else { TYPE_STOP_ENDPOINT };
        let _ = self.command(0, kind << 10 | (dci as u32) << 16 | (slot as u32) << 24);
        let dequeue = self.dma.physical(ring.page + ring.index * TRB) | ring.cycle as u64;
        let _ = self.command(dequeue, TYPE_SET_DEQUEUE << 10 | (dci as u32) << 16 | (slot as u32) << 24);
    }

    /// Starts polling IN endpoint `dci` (interrupt, or with `lossless` bulk) on its ring with `ARMED` transfers of up to a
    /// packet each.
    pub fn arm(&mut self, slot: u8, dci: u8, ring: Ring, packet: u16, lossless: bool) -> bool {
        // Only a ring made for polling leaves room for the report buffers after its transfers.
        if ring.trbs != INTERRUPT_TRBS { return false; }
        let Some(index) = self.interrupts.iter().position(Option::is_none) else { return false };
        let length = (packet as u32).min(REPORT_BYTES as u32).max(1);
        self.interrupts[index] = Some(Interrupt { slot, dci, ring, length, queue: [[0; REPORT_BYTES]; QUEUE], lengths: [0; QUEUE], head: 0, count: 0, failed: false, reported: false, lossless, in_flight: ARMED });
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
        interrupt.in_flight = interrupt.in_flight.saturating_sub(1);
        let code = status >> 24;
        // The first report and a failure are logged: a keyboard that sends nothing shows which (211-DRV-0003).
        if !matches!(code, SUCCESS | SHORT_PACKET) { interrupt.failed = true; mind::println!("[USB] {}SLOT {} ENDPOINT {}: INTERRUPT TRANSFER FAILED (COMPLETION {})", Tag(self.number), interrupt.slot, interrupt.dci, code); return; }
        if !interrupt.reported {
            interrupt.reported = true;
            let at = (pointer.wrapping_sub(self.dma.physical(interrupt.ring.page)) / TRB as u64) as usize;
            let (got, buffer) = (interrupt.length.saturating_sub(status & 0xFF_FFFF) as usize, interrupt.ring.page + REPORT_AREA + (at % REPORT_BUFFERS) * REPORT_BYTES);
            let (slot, dci, bytes) = (interrupt.slot, interrupt.dci, self.dma.bytes(buffer, got.min(8)));
            mind::println!("[USB] {}SLOT {} ENDPOINT {}: FIRST REPORT ({} BYTES: {:02X?})", Tag(self.number), slot, dci, got, bytes);
        }
        let Some(interrupt) = self.interrupts[index].as_mut() else { return };
        let at = (pointer.wrapping_sub(self.dma.physical(interrupt.ring.page)) / TRB as u64) as usize;
        let buffer = interrupt.ring.page + REPORT_AREA + (at % REPORT_BUFFERS) * REPORT_BYTES;
        let length = interrupt.length.saturating_sub(status & 0xFF_FFFF) as usize;
        let slot = (interrupt.head + interrupt.count) % QUEUE;
        if interrupt.count == QUEUE { interrupt.head = (interrupt.head + 1) % QUEUE; } else { interrupt.count += 1; }
        let bytes = self.dma.bytes(buffer, length);
        interrupt.queue[slot][..length].copy_from_slice(bytes);
        interrupt.lengths[slot] = length as u8;
        if interrupt.lossless && interrupt.in_flight + interrupt.count >= QUEUE { return; }
        interrupt.in_flight += 1;
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
        // A lossless endpoint's transfers held back for want of room go on the ring again.
        let (lossless, slot, dci, held) = (interrupt.lossless, interrupt.slot, interrupt.dci, ARMED.saturating_sub(interrupt.in_flight));
        if lossless && held > 0 && !interrupt.failed {
            for _ in 0..held { self.queue_report(index); }
            if let Some(interrupt) = self.interrupts[index].as_mut() { interrupt.in_flight += held; }
            self.doorbell(slot, dci as u32);
        }
        Ok(count)
    }
}
