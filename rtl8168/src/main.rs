#![no_std]
#![no_main]
// RTL8168G raw-frame service; DMA stays mapped for the process lifetime and init must quiesce it before reclaim.
use mind::abi::{BootInfo, CAP_KIND_IRQ, CAP_KIND_MMIO, SLOT_DEV0, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, device_config, Dma, Irq, Mmio};
use mind::idl::{net, wire};
use mind::ipc::Endpoint;
use mind::mem::Pages;
use mind::network::BADGE_COUNTERS;
use rtl8168::controller::{Controller, Error, Registers, State};
use rtl8168::rings::{self, Received, Rings, SendError, DMA_BYTES, MAX_FRAME};

const POLL_MS: u32 = 20;
const WORK: usize = 64;
const PENDING: usize = 16;
const WAITERS: usize = 4;
const RECEIVED_CAP: usize = 9;
const TX_WATCHDOG_MS: u64 = 5000;

struct Hardware(Mmio);
impl Registers for Hardware {
    fn len(&self) -> usize { self.0.len() }
    fn read8(&mut self, offset: usize) -> u8 { self.0.read8(offset) }
    fn read16(&mut self, offset: usize) -> u16 { self.0.read16(offset) }
    fn read32(&mut self, offset: usize) -> u32 { self.0.read32(offset) }
    fn write8(&mut self, offset: usize, value: u8) { self.0.write8(offset, value); }
    fn write16(&mut self, offset: usize, value: u16) { self.0.write16(offset, value); }
    fn write32(&mut self, offset: usize, value: u32) { self.0.write32(offset, value); }
    fn now_ms(&self) -> u64 { now() }
    fn sleep_ms(&mut self, milliseconds: u32) { mind::time::sleep(milliseconds as usize); }
}

fn now() -> u64 { mind::time::monotonic_ns() / 1_000_000 }

#[derive(Debug)]
enum ProbeError { Grant, Device(Error), Dma, Memory }

struct Device {
    controller: Controller<Hardware>, rings: Rings, _dma: Dma, pending: Pages,
    lengths: [usize; PENDING], head: usize, count: usize, counters: net::Counters,
    tx_progress: u64, tx_errors: u64, rx_errors: u64, index: usize,
}

impl Device {
    fn probe(index: usize, interrupts: bool) -> Result<Self, ProbeError> {
        if cap_info(SLOT_DEV0).0 != CAP_KIND_MMIO { return Err(ProbeError::Grant); }
        let pci = device_config(SLOT_DEV0, 0).map_err(|_| ProbeError::Grant)?;
        let command = device_config(SLOT_DEV0, 4).map_err(|_| ProbeError::Grant)?;
        if command & 7 != 2 { return Err(ProbeError::Grant); }
        let mmio = Mmio::map(SLOT_DEV0).map_err(|_| ProbeError::Grant)?;
        let mut controller = Controller::new(Hardware(mmio), pci).map_err(ProbeError::Device)?;
        if let Err(error) = controller.prepare() {
            let _ = mind::platform::quiesce(index);
            return Err(ProbeError::Device(error));
        }
        let mut dma = Dma::map(SLOT_MEM).map_err(|_| ProbeError::Dma)?;
        if dma.len() < DMA_BYTES { return Err(ProbeError::Dma); }
        let pending = Pages::new(PENDING * MAX_FRAME).ok_or(ProbeError::Memory)?;
        let address = dma.bytes(0, DMA_BYTES).as_mut_ptr();
        // Reset completed before ownership is initialized; only Rings accesses this mapping from now on.
        let rings = unsafe { Rings::initialize(address, dma.len(), dma.physical(0)) }.map_err(|_| ProbeError::Dma)?;
        // Engine enable follows the verified platform mastering transition; every later failure retains this object.
        unsafe { controller.program(&rings) }.map_err(ProbeError::Device)?;
        let mut device = Self { controller, rings, _dma: dma, pending, lengths: [0; PENDING], head: 0, count: 0,
                  counters: net::Counters::default(), tx_progress: now(), tx_errors: 0, rx_errors: 0, index };
        if mind::platform::resume(index).is_err() { device.fail("PCI MASTERING REJECTED"); }
        else if unsafe { device.controller.enable(interrupts) }.is_err() { device.fail("ENGINE ENABLE REJECTED"); }
        Ok(device)
    }

    fn running(&self) -> bool { self.controller.state() == State::Running }

    fn fail(&mut self, reason: &str) {
        self.controller.fail();
        let stopped = mind::platform::quiesce(self.index).is_ok();
        mind::println!("[RTL8168] {}: QUIESCE={} DMA RETAINED; TX_ERRORS={} RX_ERRORS={}", reason,
            if stopped { "VERIFIED" } else { "FAILED" }, self.tx_errors, self.rx_errors);
    }

    fn service(&mut self) {
        if !self.running() { return; }
        let status = match self.controller.acknowledge() { Ok(status) => status, Err(_) => { self.fail("DEVICE UNAVAILABLE"); return; } };
        if status & 0x8000 != 0 { self.fail("PCI SYSTEM ERROR"); return; }
        for _ in 0..WORK {
            let Some(completion) = self.rings.reclaim() else { break; };
            self.tx_progress = now();
            if completion.status & ((1 << 23) | (1 << 25)) != 0 { self.tx_errors = self.tx_errors.saturating_add(1); }
        }
        let mut frame = [0u8; MAX_FRAME];
        for _ in 0..WORK {
            match self.rings.receive(&mut frame) {
                Received::Empty => break,
                Received::Frame(length) => {
                    if self.count == PENDING { self.counters.dropped = self.counters.dropped.saturating_add(1); }
                    else {
                        let slot = (self.head + self.count) % PENDING;
                        self.pending.as_mut_slice()[slot * MAX_FRAME..slot * MAX_FRAME + length].copy_from_slice(&frame[..length]);
                        self.lengths[slot] = length; self.count += 1;
                        self.counters.received = self.counters.received.saturating_add(1);
                    }
                }
                Received::Dropped { .. } => { self.rx_errors = self.rx_errors.saturating_add(1); self.counters.dropped = self.counters.dropped.saturating_add(1); }
                Received::OutputTooSmall { .. } => { self.fail("RX BUFFER CONTRACT"); return; }
            }
        }
        if self.rings.tx_pending() != 0 && now().saturating_sub(self.tx_progress) >= TX_WATCHDOG_MS { self.fail("TX COMPLETION TIMEOUT"); }
    }

    fn send(&mut self, frame: &[u8]) -> Result<(), net::Error> {
        if !self.running() { return Err(net::Error::NoDevice); }
        if !self.controller.link() { return Err(net::Error::Busy); }
        let was_empty = self.rings.tx_pending() == 0;
        self.rings.send(frame).map_err(|error| match error { SendError::FrameLength => net::Error::Invalid, _ => net::Error::Busy })?;
        if was_empty { self.tx_progress = now(); }
        self.controller.kick();
        self.counters.sent = self.counters.sent.saturating_add(1);
        Ok(())
    }

    fn receive(&mut self, frame: &mut [u8; MAX_FRAME]) -> Result<usize, net::Error> {
        if !self.running() { return Err(net::Error::NoDevice); }
        if self.count == 0 { return Err(net::Error::Empty); }
        let length = self.lengths[self.head];
        frame[..length].copy_from_slice(&self.pending.as_slice()[self.head * MAX_FRAME..self.head * MAX_FRAME + length]);
        self.head = (self.head + 1) % PENDING; self.count -= 1;
        Ok(length)
    }

    fn info(&mut self) -> Result<net::Info, net::Error> {
        if !self.running() { return Err(net::Error::NoDevice); }
        Ok(net::Info { mac: self.controller.mac, mtu: 1500, link: self.controller.link() })
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let irq = Irq(SLOT_IRQ);
    let (kind, line, _) = cap_info(SLOT_IRQ);
    let interrupts = kind == CAP_KIND_IRQ && line >= 16 && irq.bind(Endpoint::SERVICE).is_ok();
    let index = mind::process::args_str().parse::<usize>().ok();
    let mut device = match index.ok_or(ProbeError::Grant).and_then(|index| Device::probe(index, interrupts)) {
        Ok(mut device) => {
            let link = device.running() && device.controller.link();
            mind::println!("[RTL8168] XID={:03X} PHY={:08X} LINK={} MODE={} RINGS={}; EXPERIMENTAL BSD BASELINE",
                device.controller.identity.xid, device.controller.phy, if link { "UP" } else { "DOWN" },
                if interrupts { "MSI" } else { "POLL" }, rings::SLOTS);
            Some(device)
        }
        Err(ProbeError::Device(error)) => { mind::println!("[RTL8168] INITIALIZATION FAILED: {:?}", error); None }
        Err(error) => { mind::println!("[RTL8168] INITIALIZATION FAILED: {:?}", error); None }
    };
    let mut waiters: [Option<wire::Call>; WAITERS] = [const { None }; WAITERS];
    let mut scratch = [0u8; net::REQUEST_MAX];
    let mut frame = [0u8; MAX_FRAME];
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        if let Some(d) = device.as_mut() {
            if matches!(&received, Ok(request) if request.irq.is_some()) { d.counters.interrupts = d.counters.interrupts.saturating_add(1); }
            d.service();
            if d.count != 0 || !d.running() {
                for waiter in &mut waiters { if let Some(call) = waiter.take() { let _ = net::reply_wait(call, if d.running() { d.count as u32 } else { 0 }); } }
            }
        }
        let Ok(request) = received else { continue; };
        if request.irq.is_some() { if interrupts { let _ = irq.ack(); } continue; }
        let reader = request.badge & BADGE_COUNTERS != 0;
        let (request, call) = match net::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        if reader && !matches!(request, net::Request::Info | net::Request::Counters | net::Request::Offloads) {
            let _ = match request {
                net::Request::Send { .. } => net::reply_send(call, Err(net::Error::Denied)),
                net::Request::SendPartial { .. } => net::reply_send_partial(call, Err(net::Error::Denied)),
                net::Request::Receive => net::reply_receive(call, Err(net::Error::Denied)),
                net::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); net::reply_attach(call, Err(net::Error::Denied)) }
                net::Request::Kick => net::reply_kick(call, Err(net::Error::Denied)),
                _ => wire::reject(wire::Reject::Invalid),
            };
            continue;
        }
        let _ = match request {
            net::Request::Info => net::reply_info(call, device.as_mut().ok_or(net::Error::NoDevice).and_then(Device::info).as_ref().map_err(|&e| e)),
            net::Request::Counters => net::reply_counters(call, &device.as_ref().map_or(net::Counters::default(), |d| d.counters)),
            net::Request::Offloads => net::reply_offloads(call, 0),
            net::Request::Send { frame } => net::reply_send(call, device.as_mut().ok_or(net::Error::NoDevice).and_then(|d| d.send(frame))),
            net::Request::SendPartial { .. } => net::reply_send_partial(call, Err(net::Error::Invalid)),
            net::Request::Receive => {
                let got = device.as_mut().ok_or(net::Error::NoDevice).and_then(|d| d.receive(&mut frame));
                net::reply_receive(call, got.map(|length| &frame[..length]))
            }
            net::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); net::reply_attach(call, Err(net::Error::Invalid)) }
            net::Request::Kick => net::reply_kick(call, Err(net::Error::Invalid)),
            net::Request::Wait => {
                let ready = device.as_ref().filter(|d| d.running());
                if ready.is_none() || ready.is_some_and(|d| d.count != 0) { net::reply_wait(call, ready.map_or(0, |d| d.count as u32)) }
                else {
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
