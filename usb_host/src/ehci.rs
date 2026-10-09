// EHCI host controllers (211-DRV-0004): on an Intel Mac the internal keyboard and trackpad sit behind the
// rate-matching hub of an EHCI controller, whose ports the chipset cannot hand to xHCI. Polled like xhci.rs: control
// and bulk transfers through queue heads on the asynchronous schedule, interrupt IN endpoints through rings of
// transfer descriptors on the periodic schedule, split transactions for full- and low-speed devices behind a
// high-speed hub. The bus (devices, hubs, interfaces) follows the xHCI one in main.rs.
use crate::xhci::{wait, wait_for};
use crate::{speed_name, Iface, CLASS_HUB, MAX_INTERFACES, SERVED};
use mind::dev::{Dma, Mmio};
use mind::usb::{EndpointInfo, Interface, CONTROL_MAX, MAX_ENDPOINTS};

// One controller's DMA region: the periodic frame list, queue heads, transfer descriptors and buffers.
const FRAMES: usize = 0x0000; // 1024 frame list entries
const QHS: usize = 0x1000; const QH: usize = 128; const MAX_QHS: usize = 64;
const TDS: usize = 0x3000; const TD: usize = 64; const MAX_TDS: usize = 256;
pub const SMALL: usize = 0x7000; // control data, up to CONTROL_MAX
const SETUP: usize = 0x8000; // the setup packet
const REPORTS: usize = 0x9000; const REPORT: usize = 64; // interrupt report buffers, RING per armed endpoint
pub const DATA: usize = 0x10000; // bulk data, a chunk at a time
const DMA_BYTES: usize = mind::usb::EHCI_DMA_BYTES;
const CHUNK: usize = 16 * 1024; // bulk bytes per descriptor (five pages hold it at any alignment)

// Transfer descriptor token: status, PID, error count, interrupt on complete, length, data toggle.
const ACTIVE: u32 = 0x80; const HALTED: u32 = 0x40; const BUFFER_ERROR: u32 = 0x20; const BABBLE: u32 = 0x10; const XACT: u32 = 0x08; const MISSED: u32 = 0x04;
const PID_OUT: u32 = 0; const PID_IN: u32 = 1 << 8; const PID_SETUP: u32 = 2 << 8; const CERR: u32 = 3 << 10; const IOC: u32 = 1 << 15; const TOGGLE: u32 = 1 << 31;
const T: u32 = 1; const TYPE_QH: u32 = 2;

const RING: usize = 8; const QUEUE: usize = 8; const MAX_INTERRUPTS: usize = 8;
const MAX_DEVICES: usize = 16; const MAX_PIPES: usize = 6; const MAX_DEPTH: u8 = 5;
const SCAN_TRIES: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind { Control, Bulk, Interrupt }

struct Interrupt { qh: usize, tds: [usize; RING], next: usize, length: u32, buffer: usize, queue: [[u8; REPORT]; QUEUE], lengths: [u8; QUEUE], head: usize, count: usize, failed: bool, reported: bool }

pub struct Ehci {
    mmio: Mmio, dma: Dma, op: usize, pub ports: usize, high: u32,
    qhs: u64, tds: [u64; MAX_TDS / 64], head: usize, chain: usize,
    interrupts: [Option<Interrupt>; MAX_INTERRUPTS], pub last: u32,
}

fn qh_at(i: usize) -> usize { QHS + i * QH }
fn td_at(i: usize) -> usize { TDS + i * TD }

impl Ehci {
    pub fn init(mmio: Mmio, mut dma: Dma) -> Result<Self, &'static str> {
        if dma.len() < DMA_BYTES { return Err("DMA REGION TOO SMALL"); }
        let op = mmio.read8(0) as usize;
        let (hcs, hcc) = (mmio.read32(4), mmio.read32(8));
        let (base, end) = (dma.physical(0), dma.physical(DMA_BYTES - 1));
        // Every structure and buffer shares the upper 32 address bits (CTRLDSSEGMENT); a 32-bit controller needs zero.
        if base >> 32 != end >> 32 { return Err("DMA REGION CROSSES A 4 GIB LINE"); }
        if base >> 32 != 0 && hcc & 1 == 0 { return Err("DMA REGION ABOVE 4 GIB FOR A 32-BIT CONTROLLER"); }
        mmio.write32(op, mmio.read32(op) & !1);
        if !wait(|| mmio.read32(op + 4) & 1 << 12 != 0) { return Err("DID NOT HALT"); }
        mmio.write32(op, 2);
        if !wait(|| mmio.read32(op) & 2 == 0) { return Err("DID NOT RESET"); }
        dma.zero(0, REPORTS + MAX_INTERRUPTS * RING * REPORT);
        let mut ehci = Self { mmio, dma, op, ports: (hcs & 0xF) as usize, high: (base >> 32) as u32, qhs: 0, tds: [0; MAX_TDS / 64], head: 0, chain: 0,
            interrupts: [const { None }; MAX_INTERRUPTS], last: 0 };
        // The asynchronous list's head (H, halted, linked to itself) and the periodic chain's head every frame names.
        ehci.head = ehci.qh_alloc().ok_or("NO QUEUE HEAD")?; ehci.chain = ehci.qh_alloc().ok_or("NO QUEUE HEAD")?;
        let (head, chain) = (qh_at(ehci.head), qh_at(ehci.chain));
        let head_link = ehci.physical(head) | TYPE_QH;
        ehci.write_qh(head, head_link, 1 << 15 | 2 << 12 | 64 << 16, 1 << 30, HALTED);
        ehci.write_qh(chain, T, 2 << 12 | 64 << 16, 1 << 30 | 0x01, 0);
        let chain_link = ehci.physical(chain) | TYPE_QH;
        for frame in 0..1024 { ehci.dma.write32(FRAMES + frame * 4, chain_link); }
        if hcc & 1 != 0 { ehci.mmio.write32(op + 0x10, ehci.high); } // CTRLDSSEGMENT
        ehci.mmio.write32(op + 0x14, ehci.physical(FRAMES)); ehci.mmio.write32(op + 0x18, ehci.physical(head));
        ehci.mmio.write32(op + 0x08, 0); // polled: no interrupts
        ehci.mmio.write32(op, 8 << 16 | 1 << 5 | 1 << 4 | 1); // 1 ms threshold, asynchronous and periodic schedules, run
        let mmio = &ehci.mmio;
        if !wait(|| mmio.read32(op + 4) & 1 << 12 == 0) { return Err("DID NOT RUN"); }
        mmio.write32(op + 0x40, 1); // CONFIGFLAG: every port to this controller (Intel chipsets have no companions)
        if hcs & 1 << 4 != 0 { for port in 1..=ehci.ports { let at = ehci.portsc(port); mmio.write32(at, (mmio.read32(at) & !0x2A) | 1 << 12); } }
        mind::time::sleep(20);
        Ok(ehci)
    }

    fn physical(&self, offset: usize) -> u32 { self.dma.physical(offset) as u32 }
    fn write_qh(&mut self, at: usize, link: u32, characteristics: u32, capabilities: u32, token: u32) {
        self.dma.write32(at, link); self.dma.write32(at + 4, characteristics); self.dma.write32(at + 8, capabilities);
        self.dma.write32(at + 12, 0); self.dma.write32(at + 16, T); self.dma.write32(at + 20, T); self.dma.write32(at + 24, token);
        for word in 0..10 { self.dma.write32(at + 28 + word * 4, if word >= 5 { self.high } else { 0 }); }
    }
    fn qh_alloc(&mut self) -> Option<usize> { let i = (0..MAX_QHS).find(|&i| self.qhs & 1 << i == 0)?; self.qhs |= 1 << i; Some(i) }
    fn td_alloc(&mut self) -> Option<usize> {
        let i = (0..MAX_TDS).find(|&i| self.tds[i / 64] & 1 << (i % 64) == 0)?; self.tds[i / 64] |= 1 << (i % 64); Some(i)
    }
    fn td_free(&mut self, i: usize) { self.tds[i / 64] &= !(1 << (i % 64)); }

    // A transfer descriptor: `next` (None: the end), the token, and its buffer in this region.
    fn write_td(&mut self, td: usize, next: Option<usize>, token: u32, buffer: usize) {
        let at = td_at(td);
        let next = next.map_or(T, |n| self.physical(td_at(n)));
        let page = self.physical(buffer);
        self.dma.write32(at, next); self.dma.write32(at + 4, T); self.dma.write32(at + 8, token);
        for k in 0..5 { self.dma.write32(at + 12 + k * 4, if k == 0 { page } else { (page & !0xFFF).wrapping_add(k as u32 * 0x1000) }); self.dma.write32(at + 32 + k * 4, self.high); }
    }
    fn token(&self, td: usize) -> u32 { self.dma.read32(td_at(td) + 8) }

    // Root ports: PORTSC from 0x44; bit 0 a device, bit 2 enabled, bits 1, 3, 5 changes (write one to clear).
    fn portsc(&self, port: usize) -> usize { self.op + 0x44 + 4 * (port - 1) }
    pub fn port_status(&self, port: usize) -> u32 { self.mmio.read32(self.portsc(port)) }
    pub fn connected(&self, port: usize) -> bool { self.port_status(port) & 1 != 0 }
    /// Clears a root port's change bits; true if the connection changed.
    pub fn acknowledge(&self, port: usize) -> bool {
        let value = self.port_status(port);
        if value & 0x2A != 0 { self.mmio.write32(self.portsc(port), value); }
        value & 2 != 0
    }
    /// Resets a root port; Some(3) for the high-speed device it then enables (full and low speed need a companion
    /// controller, which Intel chipsets replace with the rate-matching hub).
    pub fn enable_port(&mut self, port: usize) -> Option<u8> {
        let at = self.portsc(port);
        let value = self.mmio.read32(at);
        if value & 1 == 0 { return None; }
        if value & 4 != 0 { return Some(3); }
        if (value >> 10) & 3 == 1 { mind::println!("[USB] EHCI PORT {}: A LOW-SPEED DEVICE, NO COMPANION CONTROLLER", port); return None; }
        self.mmio.write32(at, (value & !(0x2A | 4)) | 1 << 8);
        mind::time::sleep(50);
        self.mmio.write32(at, self.mmio.read32(at) & !(0x2A | 4 | 1 << 8));
        let mmio = &self.mmio;
        if !wait(|| mmio.read32(at) & 1 << 8 == 0) { return None; }
        mind::time::sleep(5);
        if self.mmio.read32(at) & 4 == 0 { mind::println!("[USB] EHCI PORT {}: A FULL-SPEED DEVICE, NO COMPANION CONTROLLER", port); return None; }
        Some(3)
    }

    /// A queue head for endpoint `endpoint` of device `address`, linked into its schedule. Full- and low-speed
    /// devices behind a high-speed hub go through its transaction translator `tt` (hub address, port).
    pub fn pipe(&mut self, address: u8, endpoint: u8, speed: u8, packet: u16, tt: Option<(u8, u8)>, kind: Kind) -> Option<usize> {
        let qh = self.qh_alloc()?;
        let eps = match speed { 1 => 0, 2 => 1, _ => 2 };
        let mut characteristics = address as u32 | (endpoint as u32 & 0xF) << 8 | eps << 12 | (packet as u32 & 0x7FF) << 16;
        if kind == Kind::Control { characteristics |= 1 << 14; if speed != 3 { characteristics |= 1 << 27; } }
        if kind != Kind::Interrupt && speed == 3 { characteristics |= 4 << 28; } // NAK reload: high speed only, zero for a split
        let (hub, port) = if speed == 3 { (0, 0) } else { tt.unwrap_or((0, 0)) };
        let mut capabilities = 1 << 30 | (hub as u32) << 16 | (port as u32) << 23;
        // Interrupt: a start in microframe 0 of every frame, and for a split its completions in microframes 2 to 4.
        if kind == Kind::Interrupt { capabilities |= if speed == 3 { 0x01 } else { 0x1C << 8 | 0x01 }; }
        let list = if kind == Kind::Interrupt { self.chain } else { self.head };
        let link = self.dma.read32(qh_at(list));
        self.write_qh(qh_at(qh), link, characteristics, capabilities, 0);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let me = self.physical(qh_at(qh)) | TYPE_QH;
        self.dma.write32(qh_at(list), me);
        Some(qh)
    }

    /// Unlinks a queue head (and an interrupt ring on it) once the controller no longer uses it, and frees it.
    pub fn drop_pipe(&mut self, qh: usize) {
        if let Some(i) = self.interrupts.iter().position(|e| e.as_ref().is_some_and(|e| e.qh == qh)) {
            let tds = self.interrupts[i].as_ref().unwrap().tds;
            self.interrupts[i] = None;
            self.unlink(qh, true);
            for td in tds { self.td_free(td); }
        } else { self.unlink(qh, self.in_chain(qh)); }
        self.qhs &= !(1 << qh);
    }
    fn in_chain(&self, qh: usize) -> bool {
        let mut at = self.chain;
        for _ in 0..MAX_QHS { let link = self.dma.read32(qh_at(at)); if link & T != 0 { return false; } at = (link as usize & !0x1F).wrapping_sub(self.physical(QHS) as usize) / QH; if at == qh { return true; } }
        false
    }
    fn unlink(&mut self, qh: usize, periodic: bool) {
        let me = self.physical(qh_at(qh)) | TYPE_QH;
        let next = self.dma.read32(qh_at(qh));
        let start = if periodic { self.chain } else { self.head };
        let mut at = start;
        for _ in 0..MAX_QHS {
            let link = self.dma.read32(qh_at(at));
            if link == me { self.dma.write32(qh_at(at), next); break; }
            if link & T != 0 { break; }
            at = ((link & !0x1F) - self.physical(QHS)) as usize / QH;
            if at == start { break; }
        }
        if periodic { mind::time::sleep(2); return; } // two frames: the controller has left it
        // The asynchronous list: the advance doorbell says when the controller holds no copy of it.
        let (op, mmio) = (self.op, &self.mmio);
        mmio.write32(op, mmio.read32(op) | 1 << 6);
        wait(|| mmio.read32(op + 4) & 1 << 5 != 0);
        mmio.write32(op + 4, 1 << 5);
    }

    // Starts descriptors `tds` (linked, the first at index 0) on idle queue head `qh` and waits for all of them, or a
    // halt; Ok(bytes not moved by the data descriptors in `data`), Err(code as xHCI names it; 0: no answer).
    fn run(&mut self, qh: usize, tds: &[usize], data: &[usize], attempts: usize) -> Result<u32, u32> {
        let at = qh_at(qh);
        let toggle = self.dma.read32(at + 24) & TOGGLE; // bulk endpoints keep their data toggle in the queue head
        self.dma.write32(at + 20, T); self.dma.write32(at + 24, toggle);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let first = self.physical(td_at(tds[0]));
        self.dma.write32(at + 16, first);
        let mut halted = 0;
        let done = wait_for(attempts, || {
            if let Some(&td) = tds.iter().find(|&&td| self.token(td) & HALTED != 0) { halted = self.token(td); return true; }
            tds.iter().all(|&td| self.token(td) & ACTIVE == 0)
        });
        let code = if !done { Some(0) } else if halted == 0 { None }
                   else if halted & BABBLE != 0 { Some(3) } else if halted & (XACT | MISSED) != 0 { Some(4) } else if halted & BUFFER_ERROR != 0 { Some(2) } else { Some(6) };
        self.last = code.unwrap_or(1);
        let residue = data.iter().map(|&td| (self.token(td) >> 16) & 0x7FFF).sum();
        if code.is_some() { self.reset(qh, tds); }
        for &td in tds { self.td_free(td); }
        match code { None => Ok(residue), Some(code) => Err(code) }
    }

    // After a halt or no answer: the queue head is taken out, emptied (data toggle 0) and put back.
    fn reset(&mut self, qh: usize, tds: &[usize]) {
        let periodic = self.in_chain(qh);
        self.unlink(qh, periodic);
        for &td in tds { self.dma.write32(td_at(td) + 8, 0); }
        let at = qh_at(qh);
        self.dma.write32(at + 12, 0); self.dma.write32(at + 16, T); self.dma.write32(at + 20, T); self.dma.write32(at + 24, 0);
        let list = if periodic { self.chain } else { self.head };
        self.dma.write32(at, self.dma.read32(qh_at(list)));
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let me = self.physical(at) | TYPE_QH;
        self.dma.write32(qh_at(list), me);
    }

    /// A control transfer on pipe `qh` with up to CONTROL_MAX bytes of data in SMALL; the bytes moved. At most 5 s.
    pub fn control(&mut self, qh: usize, request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Result<usize, u32> {
        if length as usize > CONTROL_MAX { return Err(2); }
        let setup = [request_type, request, value as u8, (value >> 8) as u8, index as u8, (index >> 8) as u8, length as u8, (length >> 8) as u8];
        self.dma.bytes(SETUP, 8).copy_from_slice(&setup);
        let input = request_type & 0x80 != 0;
        let (Some(first), Some(status)) = (self.td_alloc(), self.td_alloc()) else { return Err(0) };
        let data = if length > 0 { self.td_alloc() } else { None };
        let status_pid = if input && length > 0 { PID_OUT } else { PID_IN };
        self.write_td(first, Some(data.unwrap_or(status)), ACTIVE | PID_SETUP | CERR | 8 << 16, SETUP);
        if let Some(data) = data { self.write_td(data, Some(status), ACTIVE | if input { PID_IN } else { PID_OUT } | CERR | TOGGLE | (length as u32) << 16, SMALL); }
        self.write_td(status, None, ACTIVE | status_pid | CERR | TOGGLE | IOC, SMALL);
        let tds: &[usize] = match data { Some(data) => &[first, data, status], None => &[first, status] };
        let tds: [usize; 3] = [tds[0], tds[1], *tds.get(2).unwrap_or(&tds[1])];
        let count = if data.is_some() { 3 } else { 2 };
        let residue = self.run(qh, &tds[..count], data.as_slice(), 1_500)?;
        Ok((length as u32).saturating_sub(residue) as usize)
    }

    /// A bulk transfer of `length` bytes in DATA on pipe `qh`, a chunk at a time; the bytes moved (fewer: a short packet).
    pub fn bulk(&mut self, qh: usize, input: bool, length: usize) -> Result<usize, u32> {
        let mut moved = 0;
        while moved < length {
            let chunk = (length - moved).min(CHUNK);
            let td = self.td_alloc().ok_or(0u32)?;
            self.write_td(td, None, ACTIVE | if input { PID_IN } else { PID_OUT } | CERR | IOC | (chunk as u32) << 16, DATA + moved);
            let residue = self.run(qh, &[td], &[td], 4_000)? as usize;
            moved += chunk - residue;
            if residue > 0 { break; }
        }
        Ok(moved)
    }

    /// Starts polling interrupt IN pipe `qh` with a ring of descriptors of up to `packet` bytes each.
    pub fn arm(&mut self, qh: usize, packet: u16) -> bool {
        let Some(slot) = self.interrupts.iter().position(Option::is_none) else { return false };
        let mut tds = [0usize; RING];
        for k in 0..RING { match self.td_alloc() { Some(td) => tds[k] = td, None => { for &td in &tds[..k] { self.td_free(td); } return false } } }
        let (length, buffer) = ((packet as u32).clamp(1, REPORT as u32), REPORTS + slot * RING * REPORT);
        for k in 0..RING { self.write_td(tds[k], Some(tds[(k + 1) % RING]), ACTIVE | PID_IN | CERR | length << 16, buffer + k * REPORT); }
        self.interrupts[slot] = Some(Interrupt { qh, tds, next: 0, length, buffer, queue: [[0; REPORT]; QUEUE], lengths: [0; QUEUE], head: 0, count: 0, failed: false, reported: false });
        let at = qh_at(qh);
        self.dma.write32(at + 20, T); self.dma.write32(at + 24, 0);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let first = self.physical(td_at(tds[0]));
        self.dma.write32(at + 16, first);
        true
    }
    pub fn armed(&self, qh: usize) -> Option<usize> { self.interrupts.iter().position(|e| e.as_ref().is_some_and(|e| e.qh == qh)) }

    /// Collects the reports the controller has put in the interrupt rings, and gives their descriptors back to it.
    pub fn pump(&mut self) {
        for slot in 0..MAX_INTERRUPTS {
            loop {
                let Some(e) = self.interrupts[slot].as_ref() else { break };
                if e.failed { break; }
                let (td, length, buffer) = (e.tds[e.next], e.length, e.buffer + e.next * REPORT);
                let token = self.token(td);
                if token & ACTIVE != 0 { break; }
                if token & HALTED != 0 {
                    let qh = e.qh;
                    mind::println!("[USB] EHCI QUEUE HEAD {}: INTERRUPT TRANSFER FAILED (STATUS {:02X})", qh, token & 0xFF);
                    if let Some(e) = self.interrupts[slot].as_mut() { e.failed = true; }
                    break;
                }
                let got = (length - ((token >> 16) & 0x7FFF).min(length)) as usize;
                let mut report = [0u8; REPORT]; report[..got].copy_from_slice(self.dma.bytes(buffer, got));
                let next = { let e = self.interrupts[slot].as_mut().unwrap();
                    if !e.reported { e.reported = true; mind::println!("[USB] EHCI QUEUE HEAD {}: FIRST REPORT ({} BYTES)", e.qh, got); }
                    let at = (e.head + e.count) % QUEUE;
                    if e.count == QUEUE { e.head = (e.head + 1) % QUEUE; } else { e.count += 1; }
                    e.queue[at] = report; e.lengths[at] = got as u8;
                    let next = e.tds[(e.next + 1) % RING]; e.next = (e.next + 1) % RING; next };
                self.write_td(td, Some(next), ACTIVE | PID_IN | CERR | length << 16, buffer);
            }
        }
    }

    /// The queued reports of armed endpoint `index`, oldest first; Err if it failed.
    pub fn take_reports(&mut self, index: usize, mut each: impl FnMut(&[u8])) -> Result<usize, ()> {
        let Some(e) = self.interrupts[index].as_mut() else { return Err(()) };
        let count = e.count;
        for n in 0..count { let at = (e.head + n) % QUEUE; each(&e.queue[at][..e.lengths[at] as usize]); }
        e.head = (e.head + count) % QUEUE; e.count = 0;
        if e.failed && count == 0 { return Err(()); }
        Ok(count)
    }
    pub fn forget(&mut self, qh: usize) { if let Some(i) = self.armed(qh) { if let Some(e) = self.interrupts[i].as_mut() { e.count = 0; } } }
    pub fn small(&mut self, length: usize) -> &mut [u8] { self.dma.bytes(SMALL, length) }
    pub fn data(&mut self, length: usize) -> &mut [u8] { self.dma.bytes(DATA, length) }
}

// The bus behind one EHCI controller: its devices, hubs and the interfaces class drivers claim, as in main.rs.
#[derive(Clone, Copy)]
pub struct Device {
    address: u8, root: u8, depth: u8, speed: u8,
    parent: Option<(usize, u8)>, tt: Option<(u8, u8)>,
    hub_ports: u8, failed: u16,
    pub generation: u16, ep0: usize, pipes: [(u8, usize); MAX_PIPES], pipe_count: usize,
    pub interfaces: [Iface; MAX_INTERFACES], pub count: usize,
}

pub struct Bus {
    pub hc: Ehci, pub devices: [Option<Device>; MAX_DEVICES], generation: u16, addresses: u128,
    root_tries: [u8; 16], root_failed: u16, name: u8,
}

impl Bus {
    pub fn new(hc: Ehci, name: u8) -> Self {
        mind::println!("[USB] EHCI {}: {} PORTS", name, hc.ports);
        for port in 1..=hc.ports { mind::println!("[USB] EHCI {} PORT {} PORTSC {:08X}", name, port, hc.port_status(port)); }
        Self { hc, devices: [None; MAX_DEVICES], generation: 0, addresses: 1, root_tries: [0; 16], root_failed: 0, name }
    }

    pub fn control(&mut self, index: usize, request_type: u8, request: u8, value: u16, windex: u16, length: u16) -> Result<usize, u32> {
        let Some(device) = self.devices[index] else { return Err(0) };
        let result = self.hc.control(device.ep0, request_type, request, value, windex, length);
        if let Err(code) = result { mind::println!("[USB] EHCI {} DEVICE {} REQUEST {:02X} {:02X} {:04X}: COMPLETION {}", self.name, device.address, request_type, request, value, code); }
        result
    }

    /// Looks at every root and hub port: new devices are set up, gone ones removed.
    pub fn scan(&mut self) {
        for port in 1..=self.hc.ports.min(15) {
            let changed = self.hc.acknowledge(port);
            let connected = self.hc.connected(port);
            let present = (0..MAX_DEVICES).find(|&i| self.devices[i].is_some_and(|d| d.parent.is_none() && d.root as usize == port));
            if changed { self.root_failed &= !(1 << port); self.root_tries[port] = 0; mind::println!("[USB] EHCI {} PORT {}: {} (PORTSC {:08X})", self.name, port, if connected { "CONNECTED" } else { "DISCONNECTED" }, self.hc.port_status(port)); }
            if let Some(index) = present { if !connected || changed { self.remove(index); } else { continue; } }
            if !connected || self.root_failed & 1 << port != 0 { continue; }
            let set_up = self.hc.enable_port(port).and_then(|speed| self.enumerate(port as u8, 0, speed, None));
            self.hc.acknowledge(port);
            if set_up.is_none() {
                self.root_tries[port] += 1;
                if self.root_tries[port] >= SCAN_TRIES { self.root_failed |= 1 << port; }
                mind::println!("[USB] EHCI {} PORT {}: DEVICE NOT SET UP (COMPLETION {}, PORTSC {:08X}, TRY {} OF {})", self.name, port, self.hc.last, self.hc.port_status(port), self.root_tries[port], SCAN_TRIES);
            }
        }
        for hub in 0..MAX_DEVICES {
            let Some(device) = self.devices[hub] else { continue };
            for port in 1..=device.hub_ports {
                let Some((status, change)) = self.hub_port(hub, port) else { break };
                if change & 1 != 0 { let _ = self.control(hub, 0x23, 1, 16, port as u16, 0); } // CLEAR_FEATURE C_PORT_CONNECTION
                let bit = 1u16 << port;
                if change & 1 != 0 { if let Some(d) = self.devices[hub].as_mut() { d.failed &= !bit; } }
                let child = (0..MAX_DEVICES).find(|&i| self.devices[i].is_some_and(|d| d.parent == Some((hub, port))));
                let connected = status & 1 != 0;
                if let Some(index) = child { if !connected || change & 1 != 0 { self.remove(index); } else { continue; } }
                if !connected || self.devices[hub].is_none_or(|d| d.failed & bit != 0) { continue; }
                let set_up = self.reset_hub_port(hub, port).and_then(|speed| self.enumerate(device.root, device.depth + 1, speed, Some((hub, port))));
                if set_up.is_none() {
                    if let Some(d) = self.devices[hub].as_mut() { d.failed |= bit; }
                    mind::println!("[USB] EHCI {} HUB {} PORT {}: DEVICE NOT SET UP (COMPLETION {})", self.name, device.address, port, self.hc.last);
                }
            }
        }
    }

    fn hub_port(&mut self, hub: usize, port: u8) -> Option<(u16, u16)> {
        let got = self.control(hub, 0xA3, 0, 0, port as u16, 4).ok()?; // GET_STATUS of the port
        if got < 4 { return None; }
        let bytes = self.hc.small(4);
        Some((u16::from_le_bytes([bytes[0], bytes[1]]), u16::from_le_bytes([bytes[2], bytes[3]])))
    }

    fn reset_hub_port(&mut self, hub: usize, port: u8) -> Option<u8> {
        self.control(hub, 0x23, 3, 4, port as u16, 0).ok()?; // SET_FEATURE PORT_RESET
        let mut status = 0;
        for _ in 0..50 {
            mind::time::sleep(10);
            let (s, change) = self.hub_port(hub, port)?;
            if change & 0x10 != 0 { status = s; break; }
        }
        let _ = self.control(hub, 0x23, 1, 20, port as u16, 0); // CLEAR_FEATURE C_PORT_RESET
        if status & 2 == 0 { return None; }
        mind::time::sleep(10);
        Some(if status & 0x200 != 0 { 2 } else if status & 0x400 != 0 { 3 } else { 1 })
    }

    // A device on a root port or a hub port: address, descriptors, configuration, its endpoints or its hub ports.
    fn enumerate(&mut self, root: u8, depth: u8, speed: u8, parent: Option<(usize, u8)>) -> Option<usize> {
        let index = self.devices.iter().position(Option::is_none)?;
        let address = (1..128u8).find(|&a| self.addresses & 1 << a == 0)?;
        let tt = parent.and_then(|(p, port)| { let hub = self.devices[p]?; if hub.speed == 3 && speed < 3 { Some((hub.address, port)) } else { hub.tt } });
        // Address 0 with the smallest packet size, for the first 8 bytes of the device descriptor and SET_ADDRESS.
        let zero = self.hc.pipe(0, 0, speed, if speed == 3 { 64 } else { 8 }, tt, Kind::Control)?;
        let mut device = Device { address: 0, root, depth, speed, parent, tt, hub_ports: 0, failed: 0, generation: 0, ep0: zero, pipes: [(0, 0); MAX_PIPES], pipe_count: 0, interfaces: [Iface::default(); MAX_INTERFACES], count: 0 };
        self.devices[index] = Some(device);
        let first = self.control(index, 0x80, 6, 0x0100, 0, 8).ok().map(|_| self.hc.small(8)[7] as u16);
        let addressed = first.is_some() && self.control(index, 0x00, 5, address as u16, 0, 0).is_ok();
        self.hc.drop_pipe(zero);
        self.devices[index] = None;
        let packet = first.filter(|_| addressed)?;
        mind::time::sleep(2); // SET_ADDRESS recovery
        let ep0 = self.hc.pipe(address, 0, speed, if matches!(packet, 8 | 16 | 32 | 64) { packet } else { 8 }, tt, Kind::Control)?;
        self.addresses |= 1 << address;
        self.generation = self.generation.wrapping_add(1).max(1);
        device.address = address; device.ep0 = ep0; device.generation = self.generation;
        self.devices[index] = Some(device);
        if self.configure(index).is_none() { self.remove(index); return None; }
        Some(index)
    }

    fn configure(&mut self, index: usize) -> Option<()> {
        let device = self.devices[index]?;
        self.control(index, 0x80, 6, 0x0100, 0, 18).ok()?;
        let descriptor: [u8; 18] = self.hc.small(18).try_into().ok()?;
        let (class, vendor, product) = (descriptor[4], u16::from_le_bytes([descriptor[8], descriptor[9]]), u16::from_le_bytes([descriptor[10], descriptor[11]]));
        self.control(index, 0x80, 6, 0x0200, 0, 9).ok()?;
        let total = u16::from_le_bytes([self.hc.small(4)[2], self.hc.small(4)[3]]).clamp(9, CONTROL_MAX as u16);
        self.control(index, 0x80, 6, 0x0200, 0, total).ok()?;
        let mut config = [0u8; CONTROL_MAX];
        config[..total as usize].copy_from_slice(self.hc.small(total as usize));
        let config = &config[..total as usize];
        let (mut interfaces, mut count, mut current) = ([Iface::default(); MAX_INTERFACES], 0usize, None);
        let mut at = 0;
        while at + 2 <= config.len() && config[at] >= 2 {
            let (length, kind) = (config[at] as usize, config[at + 1]);
            if kind == 4 && at + 9 <= config.len() {
                current = None;
                if config[at + 3] == 0 && count < MAX_INTERFACES {
                    interfaces[count].info = Interface { number: config[at + 2], class: config[at + 5], subclass: config[at + 6], protocol: config[at + 7], speed: device.speed, vendor, product, ..Default::default() };
                    current = Some(count); count += 1;
                }
            }
            if let (5, Some(i)) = (kind, current) {
                let info = &mut interfaces[i].info;
                if at + 7 <= config.len() && (info.count as usize) < MAX_ENDPOINTS {
                    info.endpoints[info.count as usize] = EndpointInfo { address: config[at + 2], attributes: config[at + 3], packet: u16::from_le_bytes([config[at + 4], config[at + 5]]) & 0x7FF, interval: config[at + 6] };
                    info.count += 1;
                }
            }
            at += length;
        }
        self.control(index, 0x00, 9, config[5] as u16, 0, 0).ok()?; // SET_CONFIGURATION
        let hub = class == CLASS_HUB || interfaces[..count].iter().any(|i| i.info.class == CLASS_HUB);
        mind::println!("[USB] EHCI {} {:04X}:{:04X} ADDRESS {} ({} SPEED){}", self.name, vendor, product, device.address, speed_name(device.speed), if hub { " HUB" } else { "" });
        // Pipes for the endpoints of the interfaces a class driver serves; a hub's ports are polled instead.
        let mut pipes = [(0u8, 0usize); MAX_PIPES]; let mut pipe_count = 0;
        for iface in interfaces[..count].iter().filter(|i| SERVED.contains(&i.info.class)) {
            for endpoint in iface.info.endpoints() {
                if !(endpoint.is_bulk() || endpoint.is_interrupt()) || pipe_count == MAX_PIPES { continue; }
                let kind = if endpoint.is_bulk() { Kind::Bulk } else { Kind::Interrupt };
                let Some(qh) = self.hc.pipe(device.address, endpoint.address & 0xF, device.speed, endpoint.packet, device.tt, kind) else { break };
                pipes[pipe_count] = (endpoint.address, qh); pipe_count += 1;
            }
        }
        if let Some(d) = self.devices[index].as_mut() { d.pipes = pipes; d.pipe_count = pipe_count; d.interfaces = interfaces; d.count = count; }
        if hub && device.depth < MAX_DEPTH {
            self.control(index, 0xA0, 6, 0x2900, 0, 9).ok()?;
            let descriptor: [u8; 9] = self.hc.small(9).try_into().ok()?;
            let (ports, power) = (descriptor[2].min(15), descriptor[5] as u64 * 2);
            for port in 1..=ports { let _ = self.control(index, 0x23, 3, 8, port as u16, 0); } // SET_FEATURE PORT_POWER
            mind::time::sleep(power.clamp(20, 500) as usize);
            if let Some(d) = self.devices[index].as_mut() { d.hub_ports = ports; }
        }
        Some(())
    }

    /// Removes a device that went away, and everything behind it if it is a hub.
    pub fn remove(&mut self, index: usize) {
        for child in 0..MAX_DEVICES { if self.devices[child].is_some_and(|d| d.parent.is_some_and(|p| p.0 == index)) { self.remove(child); } }
        let Some(device) = self.devices[index].take() else { return };
        for &(_, qh) in &device.pipes[..device.pipe_count] { self.hc.drop_pipe(qh); }
        self.hc.drop_pipe(device.ep0);
        if device.address != 0 { self.addresses &= !(1 << device.address); }
        mind::println!("[USB] EHCI {} DEVICE {} GONE", self.name, device.address);
    }

    /// The queue head of a device's endpoint.
    pub fn pipe_of(&self, index: usize, address: u8) -> Option<usize> {
        let device = self.devices[index]?;
        device.pipes[..device.pipe_count].iter().find(|p| p.0 == address).map(|p| p.1)
    }
    pub fn address_of(&self, index: usize) -> u8 { self.devices[index].map_or(0, |d| d.address) }
}
