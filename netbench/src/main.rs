#![no_std]
#![no_main]
// netbench <a.b.c.d>:<port> [MB]: network stack benchmark (issue 106) over the flow grant its launcher got from the
// policy broker. Against the host's bench server (tests/qemu_smoke.py `_BenchServer`): TCP download and upload of MB
// mebibytes (`DOWN n` / `UP n`), then 500 UDP round trips of 64 bytes to the same port. Each phase prints its rate and
// the CPU time `netstack` and the card drivers spent on it (from sysmon), per MiB or per round trip.
use core::fmt::Write;
use mind::abi::{BootInfo, SLOT_NETWORK, SLOT_SYSINFO};
use mind::idl::{socket, sysinfo};
use mind::ipc::Endpoint;
use mind::util::FixedBuf;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_SYSINFO);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const SYSMON: Endpoint = Endpoint(SLOT_SYSINFO);
const ROUND_TRIPS: u32 = 500;
const MIB: u64 = 1024 * 1024;

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

// CPU time so far of netstack and of the card drivers (virtio_net and its instances), in ns.
fn cpu() -> (u64, u64) {
    let Ok(Ok(tasks)) = sysinfo::tasks(SYSMON) else { return (0, 0) };
    let mut out = (0, 0);
    for t in tasks.as_slice() {
        let name = t.name.as_str();
        if name == "netstack" { out.0 += t.run_ns; } else if name.starts_with("virtio_net") { out.1 += t.run_ns; }
    }
    out
}

struct Phase { started: u64, cpu: (u64, u64) }
impl Phase {
    fn start() -> Self { let cpu = cpu(); Self { started: mind::time::monotonic_ns(), cpu } }
    // Elapsed ns and the CPU ns of the stack and the drivers since `start`.
    fn end(&self) -> (u64, u64, u64) {
        let elapsed = mind::time::monotonic_ns() - self.started;
        let now = cpu();
        (elapsed.max(1), now.0 - self.cpu.0, now.1 - self.cpu.1)
    }
}

fn error(what: &str, error: socket::Error) -> ! { mind::println!("NETBENCH {} FAILED: {:?}", what, error); mind::process::exit() }
fn flow<T>(what: &str, result: mind::sys::Result<Result<T, socket::Error>>) -> T {
    match result { Ok(Ok(value)) => value, Ok(Err(e)) => error(what, e), Err(_) => { mind::println!("NETBENCH {}: NO GRANT", what); mind::process::exit() } }
}

fn send_all(handle: u32, mut data: &[u8]) {
    while !data.is_empty() {
        let taken = flow("SEND", socket::tcp_send(STACK, handle, &data[..data.len().min(4096)])) as usize;
        data = &data[taken.min(data.len())..];
    }
}

// Rate in KiB/s and CPU in ms per MiB for `bytes` moved.
fn report(what: &str, bytes: u64, (ns, stack, drivers): (u64, u64, u64)) {
    let rate = bytes as u128 * 1_000_000_000 / (ns as u128 * 1024);
    let per = |cpu: u64| cpu as u128 * MIB as u128 / (bytes.max(1) as u128 * 1000); // µs per MiB
    mind::println!("NETBENCH {} {} BYTES {} MS {} KIB/S NETSTACK {} US/MIB DRIVERS {} US/MIB", what, bytes, ns / 1_000_000, rate, per(stack), per(drivers));
}

fn download(address: u32, port: u16, bytes: u64) {
    let handle = flow("CONNECT", socket::tcp_connect(STACK, address, port, 5000));
    let mut line = FixedBuf::<32>::new(); let _ = write!(line, "DOWN {}\n", bytes);
    let phase = Phase::start();
    send_all(handle, line.as_bytes());
    let (mut got, mut buffer) = (0u64, [0u8; 4096]);
    loop {
        match socket::tcp_receive(STACK, handle, 4096, &mut buffer) {
            Ok(Ok(n)) => got += n as u64,
            Ok(Err(socket::Error::Again)) => {}
            Ok(Err(socket::Error::Closed)) => break,
            Ok(Err(e)) => error("DOWN", e),
            Err(_) => error("DOWN", socket::Error::NoNetwork),
        }
    }
    let result = phase.end();
    let _ = socket::close(STACK, handle);
    if got != bytes { mind::println!("NETBENCH DOWN SHORT: {} OF {} BYTES", got, bytes); }
    report("DOWN", got, result);
}

fn upload(address: u32, port: u16, bytes: u64) {
    let handle = flow("CONNECT", socket::tcp_connect(STACK, address, port, 5000));
    let mut line = FixedBuf::<32>::new(); let _ = write!(line, "UP {}\n", bytes);
    let phase = Phase::start();
    send_all(handle, line.as_bytes());
    let block = [0x5Au8; 4096];
    let mut left = bytes;
    while left > 0 { let n = left.min(4096) as usize; send_all(handle, &block[..n]); left -= n as u64; }
    // The server answers once it has everything.
    let mut buffer = [0u8; 64];
    loop {
        match socket::tcp_receive(STACK, handle, 64, &mut buffer) {
            Ok(Ok(n)) if buffer[..n].starts_with(b"OK") => break,
            Ok(Ok(_)) | Ok(Err(socket::Error::Again)) => {}
            Ok(Err(e)) => error("UP", e),
            Err(_) => error("UP", socket::Error::NoNetwork),
        }
    }
    let result = phase.end();
    let _ = socket::close(STACK, handle);
    report("UP", bytes, result);
}

fn round_trips(address: u32, port: u16) {
    let handle = flow("UDP", socket::udp_open(STACK, 0));
    let payload = [0xA5u8; 64];
    let phase = Phase::start();
    let mut answered = 0;
    for _ in 0..ROUND_TRIPS {
        flow("UDP SEND", socket::udp_send(STACK, handle, address, port, &payload));
        let deadline = mind::time::monotonic_ns() + 1_000_000_000;
        while mind::time::monotonic_ns() < deadline {
            match socket::udp_receive(STACK, handle) { Ok(Ok(_)) => { answered += 1; break } _ => {} }
        }
    }
    let (ns, stack, drivers) = phase.end();
    let _ = socket::close(STACK, handle);
    let per = answered.max(1) as u64;
    mind::println!("NETBENCH UDP {} OF {} ROUND TRIPS {} MS {} PER S NETSTACK {} US DRIVERS {} US PER ROUND TRIP", answered, ROUND_TRIPS, ns / 1_000_000,
                   answered as u64 * 1_000_000_000 / ns, stack / 1000 / per, drivers / 1000 / per);
}

// What the software TCP/UDP checksum costs: the stack's loop over 1 MiB, in µs (what checksum offload could save per
// MiB sent).
fn checksum_cost() {
    let block = [0x5Au8; 4096];
    let started = mind::time::monotonic_ns();
    let mut sum = 0u32;
    for round in 0..256u32 {
        let data = core::hint::black_box(&block[..]);
        let mut s = data.chunks_exact(2).map(|w| u16::from_be_bytes([w[0], w[1]]) as u32).sum::<u32>() + round;
        while s > 0xFFFF { s = (s & 0xFFFF) + (s >> 16); }
        sum = sum.wrapping_add(s);
    }
    core::hint::black_box(sum);
    mind::println!("NETBENCH CHECKSUM {} US/MIB", (mind::time::monotonic_ns() - started) / 1000);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut words = mind::process::args_str().split_whitespace();
    let target = words.next().and_then(|t| t.split_once(':')).and_then(|(a, p)| Some((ipv4(a)?, p.parse::<u16>().ok()?)));
    let Some((address, port)) = target else { mind::println!("NETBENCH <A.B.C.D>:<PORT> [MB]"); return };
    let mib = words.next().and_then(|m| m.parse::<u64>().ok()).unwrap_or(4).clamp(1, 64);
    download(address, port, mib * MIB);
    upload(address, port, mib * MIB);
    round_trips(address, port);
    checksum_cost();
}
