#![no_std]
#![no_main]
// Ring 3 serial service on a UART (100000-LNK-0038): the line init lends in SLOT_DEV0, served as idl/serial.wit to the
// one client init lends SLOT_SERVICE to, the link service, when link.conf says `uart` (100000-KRN-0221). The line is
// mind::dev::Uart's: a 16550 at its ports (a PC's COM1), a PL011's registers (QEMU's virt), or the kernel's console as a
// byte channel where the registers share a page with other devices' (the A64, 100000-KRN-0222). Received bytes are
// collected at the line's interrupt (SLOT_IRQ, when init lends one) and every POLL_MS into a 64 KiB queue; bytes the
// client leaves longer are counted as lost. 8N1, no flow control: the protocol above keeps a half-duplex line.
mod ns16550;

use mind::abi::{BootInfo, CAP_KIND_IRQ, SLOT_DEV0, SLOT_IRQ, SYSCALL_TEXT_READ, SYSCALL_TEXT_WRITE, SYSCALL_WAIT, TEXT_NOWAIT};
use mind::dev::{cap_info, Irq, Mmio, Uart};
use mind::idl::{serial, wire};
use mind::ipc::Endpoint;
use mind::sys::{call, syscall};

const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 2;
const WAITERS: usize = 4;
const QUEUE: usize = 64 * 1024;
const CHIP_16550: u8 = 3;
const CHIP_PL011: u8 = 4;
const CHIP_CHANNEL: u8 = 5;
/// The rate of a line whose clock it does not know: what the firmware and the kernel leave the console at.
const FIRST_BAUD: u32 = 115_200;
// The PL011's registers: data (its error bits above the byte), flags, the interrupt mask and its clear.
const PL011_DATA: usize = 0x00;
const PL011_FLAGS: usize = 0x18;
const PL011_MASK: usize = 0x38;
const PL011_CLEAR: usize = 0x44;
const PL011_RX_EMPTY: u32 = 1 << 4;
const PL011_TX_FULL: u32 = 1 << 5;
const PL011_OVERRUN: u32 = 1 << 11;
const PL011_RX_IRQS: u32 = 1 << 4 | 1 << 6; // a byte received, and the receive timeout

// The received bytes, oldest first, in a ring.
struct Queue { bytes: [u8; QUEUE], head: usize, len: usize, lost: u32 }

impl Queue {
    fn push(&mut self, data: &[u8]) {
        for &b in data {
            if self.len == QUEUE { self.lost = self.lost.wrapping_add(1); continue; }
            self.bytes[(self.head + self.len) % QUEUE] = b;
            self.len += 1;
        }
    }
    fn take(&mut self, out: &mut [u8]) -> usize {
        let n = self.len.min(out.len());
        for b in out[..n].iter_mut() { *b = self.bytes[self.head]; self.head = (self.head + 1) % QUEUE; }
        self.len -= n;
        n
    }
}

// The line, its kind's code in serial.wit, the rate asked and the one it makes, whether its interrupt is armed, and the
// receive overruns the chip reported.
struct Line { uart: Uart, chip: u8, baud: u32, actual: u32, armed: bool, overruns: u32 }

fn name(chip: u8) -> &'static str { match chip { CHIP_16550 => "16550", CHIP_PL011 => "PL011", _ => "THE KERNEL'S CONSOLE CHANNEL" } }

// Every byte the line holds, into the queue; the overruns the chip reported meanwhile.
fn collect(uart: &Uart, queue: &mut Queue) -> u32 {
    let mut overruns = 0;
    match uart {
        Uart::Ns16550(ports, base) => loop {
            let status = ports.in8(base + ns16550::STATUS);
            if status == 0xFF || status & ns16550::READY == 0 { break; }
            if status & ns16550::OVERRUN != 0 { overruns += 1; }
            queue.push(&[ports.in8(base + ns16550::DATA)]);
        },
        Uart::Pl011(registers) => collect_pl011(registers, queue, &mut overruns),
        Uart::Channel(slot) => loop {
            // ERR_BUSY at once when nothing came, 0 once the channel ended.
            let mut chunk = [0u8; 256];
            let n = syscall(SYSCALL_TEXT_READ, chunk.as_mut_ptr() as usize, chunk.len(), [*slot, TEXT_NOWAIT, 0, 0]).result;
            if n == 0 || n > chunk.len() { break; }
            queue.push(&chunk[..n]);
        },
    }
    overruns
}

fn collect_pl011(registers: &Mmio, queue: &mut Queue, overruns: &mut u32) {
    while registers.read32(PL011_FLAGS) & PL011_RX_EMPTY == 0 {
        let word = registers.read32(PL011_DATA);
        if word & PL011_OVERRUN != 0 { *overruns += 1; }
        queue.push(&[word as u8]);
    }
}

impl Line {
    fn open() -> Option<Line> {
        let uart = Uart::open(SLOT_DEV0)?;
        let chip = match uart { Uart::Ns16550(..) => CHIP_16550, Uart::Pl011(_) => CHIP_PL011, Uart::Channel(_) => CHIP_CHANNEL };
        // The 16550's FIFO on and cleared, DTR, RTS and OUT2; its rate and 8N1 stay as the firmware set them.
        if let Uart::Ns16550(ports, base) = &uart {
            ports.out8(base + ns16550::FIFO, ns16550::FIFO_ON);
            ports.out8(base + ns16550::MODEM, ns16550::MODEM_ON);
        }
        Some(Line { uart, chip, baud: FIRST_BAUD, actual: FIRST_BAUD, armed: false, overruns: 0 })
    }

    // The chip's interrupt for each byte received, once init lent its line; the kernel's channel has none of its own.
    fn arm(&mut self) {
        match &self.uart {
            Uart::Ns16550(ports, base) => ports.out8(base + ns16550::INTERRUPTS, ns16550::RECEIVED_IRQ),
            Uart::Pl011(registers) => { registers.write32(PL011_CLEAR, 0x7FF); registers.write32(PL011_MASK, PL011_RX_IRQS); }
            Uart::Channel(_) => return,
        }
        self.armed = true;
    }

    // After an interrupt: the PL011's receive timeout cleared (taking the bytes clears the rest).
    fn served(&self) { if let Uart::Pl011(registers) = &self.uart { registers.write32(PL011_CLEAR, PL011_RX_IRQS); } }

    fn collect(&mut self, queue: &mut Queue) { self.overruns = self.overruns.wrapping_add(collect(&self.uart, queue)); }

    // `data` sent; what comes meanwhile is collected, so the chip's FIFO does not overflow while a long write goes out.
    fn send(&mut self, data: &[u8], queue: &mut Queue) -> Result<(), serial::Error> {
        if data.is_empty() { return Err(serial::Error::Invalid); }
        let Line { uart, overruns, .. } = self;
        match uart {
            Uart::Ns16550(ports, base) => for &b in data {
                while ports.in8(*base + ns16550::STATUS) & ns16550::SENDABLE == 0 { *overruns += collect_16550(*ports, *base, queue); }
                ports.out8(*base + ns16550::DATA, b);
            },
            Uart::Pl011(registers) => for &b in data {
                while registers.read32(PL011_FLAGS) & PL011_TX_FULL != 0 { collect_pl011(registers, queue, overruns); }
                registers.write32(PL011_DATA, b as u32);
            },
            Uart::Channel(slot) => {
                let slot = *slot;
                let mut done = 0;
                while done < data.len() {
                    let rest = &data[done..];
                    // The count queued, 0 while the kernel's queue is full, ERR_PEER once the channel ended.
                    let n = syscall(SYSCALL_TEXT_WRITE, slot, 0, [rest.as_ptr() as usize, rest.len(), 0, 0]).result;
                    if n > rest.len() { return Err(serial::Error::Failed); }
                    if n == 0 { *overruns += collect(uart, queue); call(SYSCALL_WAIT, 1, 0); }
                    done += n;
                }
            }
        }
        Ok(())
    }

    // A 16550 at its ports takes any divisor of its 1.8432 MHz clock; the others' clocks are the board's, so they keep
    // the rate the firmware left them at.
    fn configure(&mut self, baud: u32) -> Result<u32, serial::Error> {
        let Uart::Ns16550(ports, base) = &self.uart else { return if baud == self.baud { Ok(self.actual) } else { Err(serial::Error::Invalid) } };
        let (divisor, actual) = ns16550::divisor(baud).ok_or(serial::Error::Invalid)?;
        ports.out8(base + ns16550::LINE, ns16550::DLAB | ns16550::LINE_8N1);
        ports.out8(base + ns16550::DATA, divisor as u8);
        ports.out8(base + ns16550::INTERRUPTS, (divisor >> 8) as u8);
        ports.out8(base + ns16550::LINE, ns16550::LINE_8N1);
        // Under DLAB the interrupt enable register was the divisor's high byte: armed again.
        ports.out8(base + ns16550::INTERRUPTS, if self.armed { ns16550::RECEIVED_IRQ } else { 0 });
        (self.baud, self.actual) = (baud, actual);
        Ok(actual)
    }
}

fn collect_16550(ports: mind::dev::Ports, base: u16, queue: &mut Queue) -> u32 { collect(&Uart::Ns16550(ports, base), queue) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("uart_serial — a UART's line (a 16550, a PL011 or the kernel's console channel) for MIND Link's service alone; init starts it when link.conf says uart (100000-LNK-0038).");
    let Some(mut line) = Line::open() else { mind::println!("[UART] NO UART IN ITS SLOT"); return };
    let irq = Irq(SLOT_IRQ);
    let interrupts = !matches!(line.uart, Uart::Channel(_)) && cap_info(SLOT_IRQ).0 == CAP_KIND_IRQ && irq.bind(Endpoint::SERVICE).is_ok();
    if interrupts { line.arm(); }
    mind::println!("[UART] {} AT {} BIT/S, 8N1, NO FLOW CONTROL; RECEIVED {}", name(line.chip), line.baud,
                   if interrupts { "AT ITS INTERRUPT AND EVERY 2 MS" } else { "BY POLLING EVERY 2 MS" });
    let queue: &'static mut Queue = unsafe { &mut *core::ptr::addr_of_mut!(QUEUE_SPACE) };
    let mut scratch = [0u8; serial::REQUEST_MAX];
    let mut out = [0u8; 4096];
    let mut waiters: [Option<(wire::Call, u64)>; WAITERS] = [const { None }; WAITERS];
    let (mut lost, mut overruns) = (0, 0);
    loop {
        if (queue.lost, line.overruns) != (lost, overruns) {
            (lost, overruns) = (queue.lost, line.overruns);
            mind::println!("[UART] {} BYTES LOST (NOT TAKEN IN TIME), {} OVERRUNS OF THE CHIP SO FAR", lost, overruns);
        }
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        let now = mind::time::uptime_ms() as u64;
        line.collect(queue);
        for slot in waiters.iter_mut() {
            if slot.as_ref().is_some_and(|&(_, deadline)| queue.len > 0 || now >= deadline) {
                if let Some((call, _)) = slot.take() { let _ = serial::reply_wait(call, queue.len as u32); }
            }
        }
        let Ok(request) = received else { continue };
        if request.irq.is_some() { line.served(); if interrupts { let _ = irq.ack(); } continue; }
        let (request, call) = match serial::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let _ = match request {
            serial::Request::Info => serial::reply_info(call, Ok(&serial::Info { chip: line.chip, vendor: 0, product: 0, baud: line.baud, actual: line.actual, lost: queue.lost })),
            serial::Request::Configure { baud } => serial::reply_configure(call, line.configure(baud)),
            serial::Request::Write { data } => serial::reply_write(call, line.send(data, queue)),
            serial::Request::Read => { let n = queue.take(&mut out); serial::reply_read(call, Ok(&out[..n])) }
            serial::Request::Wait { timeout_ms } => {
                if queue.len > 0 { serial::reply_wait(call, queue.len as u32) } else {
                    let mut call = call;
                    let deadline = now + timeout_ms.min(1000) as u64;
                    match waiters.iter_mut().find(|w| w.is_none()) {
                        Some(slot) if call.defer().is_ok() => { *slot = Some((call, deadline)); Ok(()) }
                        _ => serial::reply_wait(call, 0),
                    }
                }
            }
        };
    }
}

static mut QUEUE_SPACE: Queue = Queue { bytes: [0; QUEUE], head: 0, len: 0, lost: 0 };
