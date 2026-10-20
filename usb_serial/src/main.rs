#![no_std]
#![no_main]
// Ring 3 USB-serial class driver (800-LNK-0002): the first converter usb_host gives it (its client with the serial
// badge in SLOT_DEV0 claims only the chips mind::usb::SERIAL_DEVICES names), served as idl/serial.wit to the one client
// init lends SLOT_SERVICE to. The FTDI FT232R now; the WCH CH340 is claimed and refused until its requests are in. The
// converter's bulk IN endpoint is polled by usb_host a packet at a time (usb.wit 1.3) and collected every 2 ms into a
// 64 KiB queue; bytes the client leaves longer are counted as lost. A converter pulled out is claimed again when back.
mod ftdi;

use mind::abi::{BootInfo, SLOT_DEV0};
use mind::idl::{serial, wire};
use mind::ipc::Endpoint;
use mind::usb::Host;

const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 2;
const WAITERS: usize = 4;
const QUEUE: usize = 64 * 1024;
const OUT: usize = 0; // the bulk OUT data in the buffer lent to usb_host
const CHIP_FTDI: u8 = 1;
const CHIP_CH340: u8 = 2;
const FIRST_BAUD: u32 = 115_200;
/// The FTDI latency timer: a part-filled packet goes to the host after this many ms.
const LATENCY_MS: u16 = 2;

struct Port { host: Host, handle: u32, chip: u8, vendor: u16, product: u16, input: u8, out: u8, baud: u32, actual: u32 }

// The received bytes, oldest first, in a ring.
struct Queue { bytes: [u8; QUEUE], head: usize, len: usize, lost: u32, errors: u32 }

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

fn vendor(port: &mut Port, request: u8, value: u16, index: u16) -> Result<(), serial::Error> {
    port.host.control(port.handle, 0x40, request, value, index, 0).map(|_| ()).map_err(|_| serial::Error::Failed)
}

impl Port {
    // The first converter usb_host has, set up at FIRST_BAUD; None when there is none now.
    fn claim(host: Host) -> Result<Port, Host> {
        let mut host = host;
        let Ok((handle, info)) = host.claim() else { return Err(host) };
        let input = info.endpoints().iter().find(|e| e.is_bulk() && e.is_in()).map(|e| e.address);
        let out = info.endpoints().iter().find(|e| e.is_bulk() && !e.is_in()).map(|e| e.address);
        let chip = if info.vendor == 0x0403 { CHIP_FTDI } else { CHIP_CH340 };
        let (Some(input), Some(out), CHIP_FTDI) = (input, out, chip) else {
            mind::println!("[SERIAL] CONVERTER {:04X}:{:04X} NOT DRIVEN YET (800-LNK-0002)", info.vendor, info.product);
            let _ = host.release(handle);
            return Err(host);
        };
        let mut port = Port { host, handle, chip, vendor: info.vendor, product: info.product, input, out, baud: 0, actual: 0 };
        let setup = [(ftdi::RESET, ftdi::RESET_SIO, 0), (ftdi::RESET, ftdi::PURGE_RX, 0), (ftdi::RESET, ftdi::PURGE_TX, 0), (ftdi::SET_DATA, ftdi::DATA_8N1, 0), (ftdi::SET_FLOW_CTRL, 0, 0),
                     (ftdi::MODEM_CTRL, ftdi::DTR_RTS_ON, 0), (ftdi::SET_LATENCY_TIMER, LATENCY_MS, 0)];
        for (request, value, index) in setup {
            if vendor(&mut port, request, value, index).is_err() {
                mind::println!("[SERIAL] FTDI {:04X}:{:04X}: REQUEST {} REFUSED", port.vendor, port.product, request);
                let _ = port.host.release(port.handle);
                return Err(port.host);
            }
        }
        if port.configure(FIRST_BAUD).is_err() { let _ = port.host.release(port.handle); return Err(port.host); }
        mind::println!("[SERIAL] FTDI {:04X}:{:04X} AT {} BIT/S (MAKES {}), 8N1, NO FLOW CONTROL", port.vendor, port.product, port.baud, port.actual);
        Ok(port)
    }

    fn configure(&mut self, baud: u32) -> Result<u32, serial::Error> {
        let (value, index, actual) = ftdi::divisor(baud).ok_or(serial::Error::Invalid)?;
        vendor(self, ftdi::SET_BAUD_RATE, value, index)?;
        (self.baud, self.actual) = (baud, actual);
        Ok(actual)
    }

    // The packets usb_host collected, their data into `queue`; false when the converter is gone.
    fn collect(&mut self, queue: &mut Queue) -> bool {
        let mut data = [0u8; 8 * 64];
        let mut at = 0;
        let mut errors = 0;
        let r = self.host.reports(self.handle, self.input, |packet| {
            let (n, bad) = ftdi::strip(packet, &mut data[at..]);
            at += n;
            if bad { errors += 1; }
        });
        queue.push(&data[..at]);
        queue.errors = queue.errors.wrapping_add(errors);
        r.is_ok()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), serial::Error> {
        if data.is_empty() { return Err(serial::Error::Invalid); }
        self.host.buffer_mut()[OUT..OUT + data.len()].copy_from_slice(data);
        match self.host.bulk(self.handle, self.out, OUT, data.len()) {
            Ok(n) if n == data.len() => Ok(()),
            Ok(_) => Err(serial::Error::Failed),
            Err(_) => Err(serial::Error::NoDevice),
        }
    }
}

enum State { Up(Port), Down(Host) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(host) = Host::new(Endpoint(SLOT_DEV0)) else { mind::println!("[SERIAL] NO USB HOST"); return };
    let mut state = match Port::claim(host) { Ok(port) => State::Up(port), Err(host) => { mind::println!("[SERIAL] NO CONVERTER YET"); State::Down(host) } };
    let queue: &'static mut Queue = unsafe { &mut *core::ptr::addr_of_mut!(QUEUE_SPACE) };
    let mut scratch = [0u8; serial::REQUEST_MAX];
    let mut out = [0u8; 4096];
    let mut waiters: [Option<(wire::Call, u64)>; WAITERS] = [const { None }; WAITERS];
    let mut retry_at = 0u64;
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        let now = mind::time::uptime_ms() as u64;
        state = match state {
            State::Up(mut port) => if port.collect(queue) { State::Up(port) } else {
                mind::println!("[SERIAL] THE CONVERTER IS GONE");
                let _ = port.host.release(port.handle);
                State::Down(port.host)
            },
            // Looked for again every second: a converter plugged in later, or back.
            State::Down(host) if now >= retry_at => { retry_at = now + 1000; match Port::claim(host) { Ok(port) => State::Up(port), Err(host) => State::Down(host) } }
            down => down,
        };
        for slot in waiters.iter_mut() {
            if slot.as_ref().is_some_and(|&(_, deadline)| queue.len > 0 || now >= deadline) {
                if let Some((call, _)) = slot.take() { let _ = serial::reply_wait(call, queue.len as u32); }
            }
        }
        let Ok(request) = received else { continue };
        let (request, call) = match serial::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let port = match &mut state { State::Up(port) => Some(port), State::Down(_) => None };
        let no = serial::Error::NoDevice;
        let _ = match request {
            serial::Request::Info => match port {
                Some(p) => serial::reply_info(call, Ok(&serial::Info { chip: p.chip, vendor: p.vendor, product: p.product, baud: p.baud, actual: p.actual, lost: queue.lost })),
                None => serial::reply_info(call, Err(no)),
            },
            serial::Request::Configure { baud } => serial::reply_configure(call, port.ok_or(no).and_then(|p| p.configure(baud))),
            serial::Request::Write { data } => serial::reply_write(call, port.ok_or(no).and_then(|p| p.write(data))),
            serial::Request::Read => {
                if port.is_none() && queue.len == 0 { serial::reply_read(call, Err(no)) } else {
                    let n = queue.take(&mut out);
                    serial::reply_read(call, Ok(&out[..n]))
                }
            }
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

static mut QUEUE_SPACE: Queue = Queue { bytes: [0; QUEUE], head: 0, len: 0, lost: 0, errors: 0 };
