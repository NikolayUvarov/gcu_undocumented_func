#![no_std]
#![no_main]
// Ring 3 VirtIO network driver: the modern interface (configuration structures in a memory BAR, MSI-X, or the legacy
// line when MSI-X cannot be used) or the legacy one (registers in I/O BAR0, legacy line); a receive and a transmit
// virtqueue and frame buffers in its own DMA region. Serves idl/net.wit; holds nothing but its device (Appendix B.6:
// the driver neither parses nor routes frames).
use core::sync::atomic::{fence, Ordering};
use mind::abi::{BootInfo, CAP_KIND_IRQ, CAP_KIND_MMIO, CAP_KIND_PORTS, SLOT_DEV0, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, Dma, Irq, Mmio, Ports};
use mind::virtio::{Layout, Modern, NO_VECTOR};
use mind::idl::{net, wire};
use mind::ipc::Endpoint;
use mind::mem::Pages;

// Legacy register offsets from BAR0; device-specific configuration follows at 0x14 (no MSI-X).
const FEATURES: u16 = 0x00; const GUEST_FEATURES: u16 = 0x04; const QUEUE_PFN: u16 = 0x08; const QUEUE_SIZE: u16 = 0x0C;
const QUEUE_SELECT: u16 = 0x0E; const QUEUE_NOTIFY: u16 = 0x10; const STATUS: u16 = 0x12; const ISR: u16 = 0x13;
const CONFIG_MAC: u16 = 0x14; const CONFIG_STATUS: u16 = 0x1A;
const STATUS_ACK: u8 = 1; const STATUS_DRIVER: u8 = 2; const STATUS_DRIVER_OK: u8 = 4; const STATUS_FAILED: u8 = 128;
const F_MAC: u32 = 1 << 5; const F_STATUS: u32 = 1 << 16;
const DESC_WRITE: u16 = 2;
const RX: usize = 0; const TX: usize = 1;
// DMA layout: the two virtqueues in the first 64 KiB (up to 1024 entries each), then receive and transmit buffers.
const QUEUES_BYTES: usize = 64 * 1024; const BUFFER: usize = 2048; const RX_BUFFERS: usize = 32; const TX_BUFFERS: usize = 16;
const HEADER_LEGACY: usize = 10; // struct virtio_net_hdr without mergeable buffers
const HEADER_MODERN: usize = 12; // VERSION_1 always carries num_buffers
const QUEUE_MAX: u16 = 256;
const FRAME_MAX: usize = 1514;
const PENDING: usize = 16; // received frames kept until a client takes them
const WAITERS: usize = 4;
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 20; // rings are also checked without an interrupt (a shared or lost line)

const fn align(value: usize) -> usize { (value + 4095) & !4095 }

// One split virtqueue: descriptor table, available ring, used ring (legacy layout, 4 KiB alignment).
struct Queue { size: usize, base: usize, avail: usize, used: usize, last_used: u16 }

impl Queue {
    fn new(size: usize, base: usize) -> Self {
        let avail = base + 16 * size;
        Self { size, base, avail, used: base + align(16 * size + 6 + 2 * size), last_used: 0 }
    }
    fn bytes(size: usize) -> usize { align(16 * size + 6 + 2 * size) + align(6 + 8 * size) }
}

// How the device is reached: legacy I/O registers, or the modern structures (notification offsets per queue).
enum Transport { Legacy { ports: Ports, io: u16 }, Modern { modern: Modern, notify: [usize; 2], msix: bool } }

struct Device {
    transport: Transport, header: usize, dma: Dma, memory: *mut u8, queues: [Queue; 2], mac: u64, features: u32,
    tx_free: [bool; TX_BUFFERS], pending: Pages, lengths: [usize; PENDING], head: usize, count: usize, // pending: PENDING frames in own heap
    counters: net::Counters,
}

impl Device {
    fn read8(&self, at: usize) -> u8 { unsafe { core::ptr::read_volatile(self.memory.add(at)) } }
    fn read16(&self, at: usize) -> u16 { unsafe { core::ptr::read_volatile(self.memory.add(at).cast::<u16>()) } }
    fn read32(&self, at: usize) -> u32 { unsafe { core::ptr::read_volatile(self.memory.add(at).cast::<u32>()) } }
    fn write16(&self, at: usize, value: u16) { unsafe { core::ptr::write_volatile(self.memory.add(at).cast::<u16>(), value) } }
    fn write32(&self, at: usize, value: u32) { unsafe { core::ptr::write_volatile(self.memory.add(at).cast::<u32>(), value) } }
    fn write64(&self, at: usize, value: u64) { unsafe { core::ptr::write_volatile(self.memory.add(at).cast::<u64>(), value) } }

    fn probe() -> Option<Self> {
        let mut dma = Dma::map(SLOT_MEM).ok()?;
        if dma.len() < QUEUES_BYTES + (RX_BUFFERS + TX_BUFFERS) * BUFFER { return None; }
        let memory = dma.bytes(0, 1).as_mut_ptr();
        let (transport, queues, mac, features) = match cap_info(SLOT_DEV0).0 {
            CAP_KIND_MMIO => Self::modern(&dma)?,
            CAP_KIND_PORTS => Self::legacy(&dma)?,
            _ => return None,
        };
        let header = if matches!(transport, Transport::Modern { .. }) { HEADER_MODERN } else { HEADER_LEGACY };
        let pending = Pages::new(PENDING * FRAME_MAX)?;
        let device = Self { transport, header, dma, memory, queues, mac, features, tx_free: [true; TX_BUFFERS], pending,
                            lengths: [0; PENDING], head: 0, count: 0, counters: net::Counters::default() };
        for id in 0..RX_BUFFERS { device.offer_rx(id as u16); }
        match &device.transport {
            Transport::Legacy { ports, io } => ports.out8(io + STATUS, STATUS_ACK | STATUS_DRIVER | STATUS_DRIVER_OK),
            Transport::Modern { modern, .. } => modern.ready(),
        }
        device.notify(RX);
        Some(device)
    }

    // Legacy interface: reset, features, queues at page frame numbers (the size is the device's).
    fn legacy(dma: &Dma) -> Option<(Transport, [Queue; 2], u64, u32)> {
        let ports = Ports(SLOT_DEV0);
        let (io, count) = ports.range()?;
        if count < 0x20 { return None; }
        ports.out8(io + STATUS, 0);
        ports.out8(io + STATUS, STATUS_ACK);
        ports.out8(io + STATUS, STATUS_ACK | STATUS_DRIVER);
        let features = ports.in32(io + FEATURES) & (F_MAC | F_STATUS);
        ports.out32(io + GUEST_FEATURES, features);
        let mut queues = [Queue::new(0, 0), Queue::new(0, 0)];
        let mut base = 0;
        for (index, queue) in queues.iter_mut().enumerate() {
            ports.out16(io + QUEUE_SELECT, index as u16);
            let size = ports.in16(io + QUEUE_SIZE) as usize;
            if size < RX_BUFFERS || base + Queue::bytes(size) > QUEUES_BYTES { ports.out8(io + STATUS, STATUS_FAILED); return None; }
            *queue = Queue::new(size, base);
            ports.out32(io + QUEUE_PFN, (dma.physical(base) >> 12) as u32);
            base += Queue::bytes(size);
        }
        let mac = if features & F_MAC != 0 { (0..6).fold(0u64, |mac, i| mac << 8 | ports.in8(io + CONFIG_MAC + i) as u64) } else { 0 };
        Some((Transport::Legacy { ports, io }, queues, mac, features))
    }

    // Modern interface: VERSION_1, queues of our size at three addresses each, MSI-X entry 0 for both queues and
    // configuration changes if the device takes it (init granted the vector), else the legacy line.
    fn modern(dma: &Dma) -> Option<(Transport, [Queue; 2], u64, u32)> {
        let layout = Layout::read(SLOT_DEV0)?;
        layout.single_bar()?;
        let modern = Modern { bar: Mmio::map(SLOT_DEV0).ok()?, layout };
        let features = modern.negotiate((F_MAC | F_STATUS) as u64)? as u32;
        // init grants an MSI-X vector as a line above 15 (the device accepts vector numbers even with MSI-X off).
        let (kind, line, _) = cap_info(SLOT_IRQ);
        let mut msix = kind == CAP_KIND_IRQ && line >= 16;
        modern.set16(mind::virtio::MSIX_CONFIG, if msix { 0 } else { NO_VECTOR });
        if msix && modern.common16(mind::virtio::MSIX_CONFIG) != 0 { msix = false; modern.set16(mind::virtio::MSIX_CONFIG, NO_VECTOR); }
        let (mut queues, mut notify, mut base) = ([Queue::new(0, 0), Queue::new(0, 0)], [0usize; 2], 0);
        for index in 0..2 {
            modern.set16(mind::virtio::QUEUE_SELECT, index as u16);
            let size = modern.common16(mind::virtio::QUEUE_SIZE).min(QUEUE_MAX) as usize;
            if size < RX_BUFFERS || base + Queue::bytes(size) > QUEUES_BYTES { return None; }
            let queue = Queue::new(size, base);
            let addresses = (dma.physical(queue.base), dma.physical(queue.avail), dma.physical(queue.used));
            let set = |vector| modern.queue(index as u16, size as u16, addresses.0, addresses.1, addresses.2, vector);
            let (_, at) = match set(if msix { 0 } else { NO_VECTOR }) { Some(done) => done, None if msix => { msix = false; set(NO_VECTOR)? } None => return None };
            queues[index] = queue; notify[index] = at; base += Queue::bytes(size);
        }
        let mac = if features & F_MAC != 0 { (0..6).fold(0u64, |mac, i| mac << 8 | modern.device8(i) as u64) } else { 0 };
        Some((Transport::Modern { modern, notify, msix }, queues, mac, features))
    }

    fn rx_buffer(&self, id: usize) -> usize { QUEUES_BYTES + id * BUFFER }
    fn tx_buffer(&self, id: usize) -> usize { QUEUES_BYTES + (RX_BUFFERS + id) * BUFFER }

    // Puts descriptor `id` (pointing at `offset`, `len` bytes) on the available ring of queue `q`.
    fn offer(&self, q: usize, id: u16, offset: usize, len: usize, flags: u16) {
        let queue = &self.queues[q];
        let (desc, index) = (queue.base + 16 * id as usize, self.read16(queue.avail + 2));
        self.write64(desc, self.dma.physical(offset)); self.write32(desc + 8, len as u32); self.write16(desc + 12, flags); self.write16(desc + 14, 0);
        self.write16(queue.avail + 4 + 2 * (index as usize % queue.size), id);
        fence(Ordering::SeqCst); // the descriptor and ring entry are visible before the index moves
        self.write16(queue.avail + 2, index.wrapping_add(1));
        fence(Ordering::SeqCst);
    }
    fn offer_rx(&self, id: u16) { self.offer(RX, id, self.rx_buffer(id as usize), BUFFER, DESC_WRITE); }
    fn notify(&self, q: usize) {
        match &self.transport { Transport::Legacy { ports, io } => ports.out16(io + QUEUE_NOTIFY, q as u16), Transport::Modern { modern, notify, .. } => modern.notify(notify[q], q as u16) }
    }
    // Acknowledges a legacy-line interrupt (reading the ISR); MSI-X messages need no acknowledgement.
    fn acknowledge(&self) {
        match &self.transport { Transport::Legacy { ports, io } => { let _ = ports.in8(io + ISR); } Transport::Modern { modern, msix: false, .. } => { let _ = modern.isr(); } _ => {} }
    }
    fn mode(&self) -> &'static str { match &self.transport { Transport::Legacy { .. } => "LEGACY INTX", Transport::Modern { msix: true, .. } => "MODERN MSI-X", Transport::Modern { .. } => "MODERN INTX" } }

    // Takes finished receive buffers into the pending frames and gives them back to the device; frees sent buffers.
    fn service(&mut self) {
        let (used, size, mut last) = { let queue = &self.queues[RX]; (queue.used, queue.size, queue.last_used) };
        let mut returned = false;
        while last != self.read16(used + 2) {
            fence(Ordering::SeqCst);
            let entry = used + 4 + 8 * (last as usize % size);
            let (id, len) = (self.read32(entry) as usize, self.read32(entry + 4) as usize);
            if id < RX_BUFFERS {
                let frame = len.saturating_sub(self.header).min(FRAME_MAX);
                if self.count == PENDING || frame < 14 { self.counters.dropped += 1; } else {
                    let slot = (self.head + self.count) % PENDING;
                    let start = self.rx_buffer(id) + self.header;
                    for i in 0..frame { let byte = self.read8(start + i); self.pending.as_mut_slice()[slot * FRAME_MAX + i] = byte; }
                    self.lengths[slot] = frame; self.count += 1; self.counters.received += 1;
                }
                self.offer_rx(id as u16); returned = true;
            }
            last = last.wrapping_add(1);
        }
        self.queues[RX].last_used = last;
        if returned { self.notify(RX); }
        let (used, size, mut last) = { let queue = &self.queues[TX]; (queue.used, queue.size, queue.last_used) };
        while last != self.read16(used + 2) {
            fence(Ordering::SeqCst);
            let id = self.read32(used + 4 + 8 * (last as usize % size)) as usize;
            if id < TX_BUFFERS { self.tx_free[id] = true; }
            last = last.wrapping_add(1);
        }
        self.queues[TX].last_used = last;
    }

    fn send(&mut self, frame: &[u8]) -> Result<(), net::Error> {
        if frame.len() < 14 || frame.len() > FRAME_MAX { return Err(net::Error::Invalid); }
        self.service();
        let id = self.tx_free.iter().position(|&free| free).ok_or(net::Error::Busy)?;
        self.tx_free[id] = false;
        let at = self.tx_buffer(id);
        for i in 0..self.header { unsafe { core::ptr::write_volatile(self.memory.add(at + i), 0) } }
        for (i, &byte) in frame.iter().enumerate() { unsafe { core::ptr::write_volatile(self.memory.add(at + self.header + i), byte) } }
        self.offer(TX, id as u16, at, self.header + frame.len(), 0);
        self.notify(TX);
        self.counters.sent += 1;
        Ok(())
    }

    fn receive(&mut self) -> Option<&[u8]> {
        self.service();
        if self.count == 0 { return None; }
        let slot = self.head;
        self.head = (self.head + 1) % PENDING; self.count -= 1;
        Some(&self.pending.as_slice()[slot * FRAME_MAX..slot * FRAME_MAX + self.lengths[slot]])
    }

    fn link(&self) -> bool {
        self.features & F_STATUS == 0 || match &self.transport { Transport::Legacy { ports, io } => ports.in16(io + CONFIG_STATUS) & 1 != 0, Transport::Modern { modern, .. } => modern.device16(6) & 1 != 0 }
    }
    fn info(&self) -> net::Info { net::Info { mac: self.mac, mtu: (FRAME_MAX - 14) as u16, link: self.link() } }
}

fn mac_text(mac: u64) -> [u8; 17] {
    let mut text = [b':'; 17];
    for i in 0..6 {
        let byte = (mac >> (40 - 8 * i)) as u8;
        text[i * 3] = b"0123456789ABCDEF"[(byte >> 4) as usize]; text[i * 3 + 1] = b"0123456789ABCDEF"[(byte & 15) as usize];
    }
    text
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut device = Device::probe();
    let irq = Irq(SLOT_IRQ);
    let mut waiters: [Option<wire::Call>; WAITERS] = [const { None }; WAITERS];
    let mut scratch = [0u8; net::REQUEST_MAX];
    let mut frame = [0u8; FRAME_MAX];
    match &device {
        Some(d) => {
            let _ = irq.bind(Endpoint::SERVICE);
            mind::println!("[VIRTIO_NET] MAC={} LINK={} QUEUES={}/{} {}", core::str::from_utf8(&mac_text(d.mac)).unwrap_or("?"),
                           if d.link() { "UP" } else { "DOWN" }, d.queues[RX].size, d.queues[TX].size, d.mode());
        }
        None => mind::println!("[VIRTIO_NET] NO DEVICE"),
    }
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        if let Some(d) = device.as_mut() {
            if matches!(&received, Ok(request) if request.irq.is_some()) {
                d.acknowledge();
                d.counters.interrupts += 1;
                let _ = irq.ack();
            }
            d.service();
            // Clients parked in `wait` are answered once a frame is waiting.
            if d.count > 0 { for waiter in waiters.iter_mut() { if let Some(call) = waiter.take() { let _ = net::reply_wait(call, d.count as u32); } } }
        }
        let Ok(request) = received else { continue };
        if request.irq.is_some() { continue; }
        let (request, call) = match net::decode(&request, RECEIVED_CAP, &mut scratch) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let _ = match (request, device.as_mut()) {
            (net::Request::Counters, d) => net::reply_counters(call, &d.map_or(net::Counters::default(), |d| d.counters)),
            (net::Request::Info, None) => net::reply_info(call, Err(net::Error::NoDevice)),
            (net::Request::Send { .. }, None) => net::reply_send(call, Err(net::Error::NoDevice)),
            (net::Request::Receive, None) => net::reply_receive(call, Err(net::Error::NoDevice)),
            (net::Request::Wait, None) => net::reply_wait(call, 0),
            (net::Request::Info, Some(d)) => net::reply_info(call, Ok(&d.info())),
            (net::Request::Send { frame: data }, Some(d)) => net::reply_send(call, d.send(data)),
            (net::Request::Receive, Some(d)) => {
                let got = d.receive().map(|data| { frame[..data.len()].copy_from_slice(data); data.len() });
                net::reply_receive(call, got.map(|len| &frame[..len]).ok_or(net::Error::Empty))
            }
            (net::Request::Wait, Some(d)) => {
                if d.count > 0 { net::reply_wait(call, d.count as u32) } else {
                    let mut call = call;
                    match (waiters.iter().position(Option::is_none), call.defer()) {
                        (Some(index), Ok(())) => { waiters[index] = Some(call); Ok(()) }
                        _ => net::reply_wait(call, 0),
                    }
                }
            }
        };
    }
}
