//! The byte stream to the other end on Linux: a serial port set raw through termios, a TCP connection, or a pseudo-terminal
//! that `serve` makes for tests. Linux only (glibc's termios layout).
use std::ffi::{c_char, c_int, c_short, c_uint, c_void, CStr};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The bytes this process sent and received on its lines: `effector485` shows them, with the speed (800-LNK-0018).
pub static SENT: AtomicU64 = AtomicU64::new(0);
pub static RECEIVED: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios { iflag: c_uint, oflag: c_uint, cflag: c_uint, lflag: c_uint, line: u8, cc: [u8; 32], ispeed: c_uint, ospeed: c_uint }

#[repr(C)]
struct PollFd { fd: c_int, events: c_short, revents: c_short }

extern "C" {
    fn tcgetattr(fd: c_int, t: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, when: c_int, t: *const Termios) -> c_int;
    fn cfmakeraw(t: *mut Termios);
    fn cfsetspeed(t: *mut Termios, speed: c_uint) -> c_int;
    fn tcflush(fd: c_int, what: c_int) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
    fn posix_openpt(flags: c_int) -> c_int;
    fn grantpt(fd: c_int) -> c_int;
    fn unlockpt(fd: c_int) -> c_int;
    fn ptsname_r(fd: c_int, buf: *mut c_char, len: usize) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
}

const TCSANOW: c_int = 0;
const TCIOFLUSH: c_int = 2;
const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;
const O_RDWR: c_int = 2;
const O_NOCTTY: c_int = 0o400;
const O_NONBLOCK: c_int = 0o4000;
const CLOCAL: c_uint = 0o4000;
const CREAD: c_uint = 0o200;
const CRTSCTS: c_uint = 0o20000000000;
const VMIN: usize = 6;
const VTIME: usize = 5;
const POLLIN: c_short = 1;

/// Linux's speed codes for the rates a converter takes.
fn speed(baud: u32) -> Option<c_uint> {
    Some(match baud {
        9600 => 0o15, 19200 => 0o16, 38400 => 0o17, 57600 => 0o10001, 115200 => 0o10002, 230400 => 0o10003,
        460800 => 0o10004, 500000 => 0o10005, 576000 => 0o10006, 921600 => 0o10007, 1000000 => 0o10010,
        1152000 => 0o10011, 1500000 => 0o10012, 2000000 => 0o10013, 2500000 => 0o10014, 3000000 => 0o10015,
        _ => return None,
    })
}

fn raw(fd: c_int, baud: Option<u32>) -> Result<(), String> {
    let mut t: Termios = unsafe { std::mem::zeroed() };
    if unsafe { tcgetattr(fd, &mut t) } != 0 { return Err(format!("not a terminal: {}", std::io::Error::last_os_error())); }
    unsafe { cfmakeraw(&mut t) };
    t.cflag |= CLOCAL | CREAD;
    t.cflag &= !CRTSCTS;
    t.cc[VMIN] = 0;
    t.cc[VTIME] = 0;
    if let Some(b) = baud {
        let code = speed(b).ok_or_else(|| format!("{b} baud is not a rate this tool sets"))?;
        unsafe { cfsetspeed(&mut t, code) };
    }
    if unsafe { tcsetattr(fd, TCSANOW, &t) } != 0 { return Err(format!("cannot set the line: {}", std::io::Error::last_os_error())); }
    unsafe { tcflush(fd, TCIOFLUSH) };
    Ok(())
}

pub struct Port {
    file: File,
    /// The pseudo-terminal's other side, held open so the line does not hang up between clients.
    _held: Option<File>,
    start: Instant,
    failed: bool,
    /// A pause before the slave answers, for an RS-485 converter to turn the line round.
    pub turnaround: Duration,
    pub name: String,
}

impl Port {
    /// `tcp:HOST:PORT`, `listen:PORT` (one connection; port 0 picks one and says it), `pty`, or a serial port's path.
    pub fn open(spec: &str, baud: u32) -> Result<Port, String> {
        let (file, held, name) = if let Some(addr) = spec.strip_prefix("tcp:") {
            let s = TcpStream::connect(addr).map_err(|e| format!("{addr}: {e}"))?;
            s.set_nodelay(true).ok();
            (File::from(OwnedFd::from(s)), None, addr.to_string())
        } else if let Some(port) = spec.strip_prefix("listen:") {
            let l = TcpListener::bind(("127.0.0.1", port.parse::<u16>().map_err(|_| format!("bad port {port}"))?)).map_err(|e| e.to_string())?;
            println!("listening on {}", l.local_addr().map_err(|e| e.to_string())?);
            let (s, peer) = l.accept().map_err(|e| e.to_string())?;
            s.set_nodelay(true).ok();
            (File::from(OwnedFd::from(s)), None, peer.to_string())
        } else if spec == "pty" {
            let fd = unsafe { posix_openpt(O_RDWR | O_NOCTTY) };
            if fd < 0 || unsafe { grantpt(fd) } != 0 || unsafe { unlockpt(fd) } != 0 { return Err(format!("no pseudo-terminal: {}", std::io::Error::last_os_error())); }
            let file = unsafe { File::from_raw_fd(fd) };
            let mut buf = [0 as c_char; 128];
            if unsafe { ptsname_r(fd, buf.as_mut_ptr(), buf.len()) } != 0 { return Err("ptsname failed".into()); }
            let path = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned();
            let held = OpenOptions::new().read(true).write(true).custom_flags(O_NOCTTY).open(&path).map_err(|e| format!("{path}: {e}"))?;
            raw(held.as_raw_fd(), None)?;
            println!("pty {path}");
            (file, Some(held), path)
        } else {
            // Opened without waiting for a carrier, then blocking again.
            let file = OpenOptions::new().read(true).write(true).custom_flags(O_NOCTTY | O_NONBLOCK).open(spec).map_err(|e| format!("{spec}: {e}"))?;
            let fd = file.as_raw_fd();
            raw(fd, Some(baud))?;
            unsafe { fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) & !O_NONBLOCK) };
            (file, None, spec.to_string())
        };
        std::io::stdout().flush().ok();
        Ok(Port { file, _held: held, start: Instant::now(), failed: false, turnaround: Duration::ZERO, name })
    }
}

impl linkproto::Line for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        if self.failed { return false; }
        if self.file.write_all(bytes).is_err() { self.failed = true; } else { SENT.fetch_add(bytes.len() as u64, Ordering::Relaxed); }
        !self.failed
    }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        if self.failed { std::thread::sleep(Duration::from_millis(timeout_ms.min(100) as u64)); return 0; }
        let mut p = PollFd { fd: self.file.as_raw_fd(), events: POLLIN, revents: 0 };
        let ready = unsafe { poll(&mut p, 1, timeout_ms.min(i32::MAX as u32) as c_int) };
        if ready <= 0 { return 0; }
        let n = unsafe { read(p.fd, out.as_mut_ptr().cast(), out.len()) };
        if n > 0 { RECEIVED.fetch_add(n as u64, Ordering::Relaxed); return n as usize; }
        // Readable yet nothing: the other side hung up (a closed connection, a converter pulled out).
        let e = std::io::Error::last_os_error();
        if n == 0 || e.kind() != std::io::ErrorKind::Interrupted { self.failed = true; }
        0
    }

    fn now_ms(&mut self) -> u64 { self.start.elapsed().as_millis() as u64 }

    fn turnaround(&mut self) { if !self.turnaround.is_zero() { std::thread::sleep(self.turnaround); } }
}

impl Port {
    /// The line failed for good (closed or gone).
    pub fn failed(&self) -> bool { self.failed }

    /// The FTDI driver's latency timer for this port, in ms, when the system shows it (it delays every answer).
    pub fn latency_ms(&self) -> Option<u32> {
        let tty = std::fs::canonicalize(&self.name).ok()?;
        let base = tty.file_name()?.to_str()?;
        std::fs::read_to_string(format!("/sys/bus/usb-serial/devices/{base}/latency_timer")).ok()?.trim().parse().ok()
    }
}
