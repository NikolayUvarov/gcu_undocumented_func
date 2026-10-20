//! The byte stream to the other end on Windows (800-LNK-0007): a COM port set through the Win32 communications calls,
//! or a TCP connection. `pty` is Unix's.
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Default)]
struct Dcb {
    length: u32, baud: u32, flags: u32, reserved: u16, xon_limit: u16, xoff_limit: u16, byte_size: u8, parity: u8,
    stop_bits: u8, xon: i8, xoff: i8, error: i8, eof: i8, event: i8, reserved1: u16,
}

#[repr(C)]
struct Timeouts { interval: u32, total_multiplier: u32, total_constant: u32, write_multiplier: u32, write_constant: u32 }

#[link(name = "kernel32")]
extern "system" {
    fn GetCommState(handle: *mut c_void, dcb: *mut Dcb) -> i32;
    fn SetCommState(handle: *mut c_void, dcb: *const Dcb) -> i32;
    fn SetCommTimeouts(handle: *mut c_void, timeouts: *const Timeouts) -> i32;
    fn PurgeComm(handle: *mut c_void, flags: u32) -> i32;
}

// DCB's bit fields: binary mode, DTR and RTS held on; no parity check, no flow control, no error abort.
const F_BINARY: u32 = 1;
const DTR_ENABLE: u32 = 1 << 4;
const RTS_ENABLE: u32 = 1 << 12;
const PURGE_TX_RX: u32 = 0x4 | 0x8;

enum Stream { Serial { file: File, timeout: Option<u32> }, Tcp(TcpStream) }

pub struct Port {
    stream: Stream,
    start: Instant,
    failed: bool,
    /// A pause before the slave answers, for an RS-485 converter to turn the line round.
    pub turnaround: Duration,
    pub name: String,
}

fn os(what: &str) -> String { format!("{what}: {}", std::io::Error::last_os_error()) }

impl Port {
    /// `tcp:HOST:PORT`, `listen:PORT` (one connection; port 0 picks one and says it), or a COM port (`COM10`).
    pub fn open(spec: &str, baud: u32) -> Result<Port, String> {
        let (stream, name) = if let Some(addr) = spec.strip_prefix("tcp:") {
            let s = TcpStream::connect(addr).map_err(|e| format!("{addr}: {e}"))?;
            s.set_nodelay(true).ok();
            (Stream::Tcp(s), addr.to_string())
        } else if let Some(port) = spec.strip_prefix("listen:") {
            let l = TcpListener::bind(("127.0.0.1", port.parse::<u16>().map_err(|_| format!("bad port {port}"))?)).map_err(|e| e.to_string())?;
            println!("listening on {}", l.local_addr().map_err(|e| e.to_string())?);
            std::io::stdout().flush().ok();
            let (s, peer) = l.accept().map_err(|e| e.to_string())?;
            s.set_nodelay(true).ok();
            (Stream::Tcp(s), peer.to_string())
        } else if spec == "pty" {
            return Err("pty: pseudo-terminals are Unix's; use listen:0 for a test on Windows".into());
        } else {
            // COM10 and above are reached only through the device namespace.
            let path = if spec.starts_with(r"\\.\") { spec.to_string() } else { format!(r"\\.\{spec}") };
            let file = OpenOptions::new().read(true).write(true).open(&path).map_err(|e| format!("{spec}: {e}"))?;
            let handle = file.as_raw_handle() as *mut c_void;
            let mut dcb = Dcb { length: std::mem::size_of::<Dcb>() as u32, ..Default::default() };
            if unsafe { GetCommState(handle, &mut dcb) } == 0 { return Err(os(&format!("{spec}: not a serial port"))); }
            dcb.baud = baud;
            dcb.flags = F_BINARY | DTR_ENABLE | RTS_ENABLE;
            (dcb.byte_size, dcb.parity, dcb.stop_bits) = (8, 0, 0);
            if unsafe { SetCommState(handle, &dcb) } == 0 { return Err(os(&format!("{spec}: {baud} bit/s refused"))); }
            unsafe { PurgeComm(handle, PURGE_TX_RX) };
            (Stream::Serial { file, timeout: None }, spec.to_string())
        };
        Ok(Port { stream, start: Instant::now(), failed: false, turnaround: Duration::ZERO, name })
    }

    /// The line failed for good (closed or gone).
    pub fn failed(&self) -> bool { self.failed }

    /// The FTDI latency timer is a driver setting on Windows (the port's Advanced properties), not read here.
    pub fn latency_ms(&self) -> Option<u32> { None }
}

impl linkproto::Line for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        if self.failed { return false; }
        let r = match &mut self.stream { Stream::Serial { file, .. } => file.write_all(bytes), Stream::Tcp(s) => s.write_all(bytes) };
        if r.is_err() { self.failed = true; }
        !self.failed
    }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        if self.failed { std::thread::sleep(Duration::from_millis(timeout_ms.min(100) as u64)); return 0; }
        let r = match &mut self.stream {
            Stream::Serial { file, timeout } => {
                // Returns as soon as a byte is there, or after the wait with none (the documented MAXDWORD pairing).
                let wait = timeout_ms.clamp(1, u32::MAX - 1);
                if *timeout != Some(wait) {
                    let t = Timeouts { interval: u32::MAX, total_multiplier: u32::MAX, total_constant: wait, write_multiplier: 0, write_constant: 0 };
                    if unsafe { SetCommTimeouts(file.as_raw_handle() as *mut c_void, &t) } == 0 { self.failed = true; return 0; }
                    *timeout = Some(wait);
                }
                file.read(out)
            }
            Stream::Tcp(s) => {
                s.set_read_timeout(Some(Duration::from_millis(timeout_ms.max(1) as u64))).ok();
                match s.read(out) { Ok(0) => { self.failed = true; return 0; } other => other }
            }
        };
        match r {
            Ok(n) => n,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => 0,
            Err(_) => { self.failed = true; 0 }
        }
    }

    fn now_ms(&mut self) -> u64 { self.start.elapsed().as_millis() as u64 }

    fn turnaround(&mut self) { if !self.turnaround.is_zero() { std::thread::sleep(self.turnaround); } }
}
