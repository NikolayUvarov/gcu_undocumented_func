#![no_std]
#![no_main]
// link (800-LNK-0003): MIND Core's end of MIND Link, the slave of linkproto (docs/link/protocol.md) over usb_serial's
// port (idl/serial.wit, SLOT_DEV0). Its authority is what init grants (MC-2.3): files only in data/link/ (a vfs client
// confined there, SLOT_VFS), the system log through a reading log client (SLOT_LOG), and no console. init starts it
// when the boot volume holds link.conf at its root and passes that file's words as arguments (`baud=<rate>`).
// Everything from the line is checked by linkproto before use (MC-11.4); hellos, files and refusals are logged (MC-10.2).
use linkproto::app::{FileService, Files};
use linkproto::endpoint::{channel, code, kind};
use linkproto::{Line, Service, Slave};
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_VFS};
use mind::fs::{self, File};
use mind::idl::serial;
use mind::ipc::Endpoint;

const DIR: &str = "data/link";
/// Where a put is written until its SHA-256 matches; left out of listings.
const PART: &str = "data/link/link-part.tmp";
const FIRST_BAUD: u32 = 115_200;

/// The port as linkproto's line: bytes through usb_serial, waiting with its `wait`.
struct Port { endpoint: Endpoint, pending: [u8; 4096], at: usize, len: usize }

impl Line for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        bytes.chunks(4096).all(|chunk| matches!(serial::write(self.endpoint, chunk), Ok(Ok(()))))
    }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        if self.at == self.len {
            let waiting = serial::wait(self.endpoint, timeout_ms.min(1000)).unwrap_or(0);
            if waiting == 0 { return 0; }
            self.len = match serial::read(self.endpoint, &mut self.pending) { Ok(Ok(n)) => n, _ => 0 };
            self.at = 0;
        }
        let n = (self.len - self.at).min(out.len());
        out[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        n
    }

    fn now_ms(&mut self) -> u64 { mind::time::uptime_ms() as u64 }
}

fn path<'a>(name: &str, out: &'a mut [u8; 320]) -> &'a str {
    let mut n = 0;
    for part in [DIR, "/", name] { out[n..n + part.len()].copy_from_slice(part.as_bytes()); n += part.len(); }
    core::str::from_utf8(&out[..n]).unwrap_or(PART)
}

fn io(e: fs::Error) -> u8 {
    match e { fs::Error::NotFound => code::NOT_FOUND, fs::Error::Denied | fs::Error::ReadOnly | fs::Error::Name => code::DENIED, _ => code::IO }
}

/// The files of data/link/ and the system log.
struct Store { put: Option<(File, [u8; 255], usize)>, get: Option<File> }

impl Files for Store {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        if size > u32::MAX as u64 { return Err(code::IO); }
        self.put = None;
        let file = File::create(PART).map_err(io)?;
        let mut kept = [0u8; 255];
        kept[..name.len()].copy_from_slice(name.as_bytes());
        mind::println!("[LINK] RECEIVING {} ({} BYTES)", name, size);
        self.put = Some((file, kept, name.len()));
        Ok(())
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> {
        let (file, _, _) = self.put.as_mut().ok_or(code::BAD_REQUEST)?;
        match file.write_at(offset as usize, data) { Ok(n) if n == data.len() => Ok(()), Ok(_) => Err(code::IO), Err(e) => Err(io(e)) }
    }

    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8> {
        let (file, kept, len) = self.put.take().ok_or(code::BAD_REQUEST)?;
        let name = core::str::from_utf8(&kept[..len]).map_err(|_| code::BAD_REQUEST)?;
        if !matches {
            drop(file);
            let _ = fs::remove(PART);
            mind::println!("[LINK] {}: SHA-256 DOES NOT MATCH, NOT KEPT", name);
            return Ok(());
        }
        file.flush().map_err(io)?;
        drop(file);
        let mut buf = [0u8; 320];
        let target = path(name, &mut buf);
        // A file of the same name is replaced.
        let _ = fs::remove(target);
        fs::rename(PART, target).map_err(io)?;
        mind::println!("[LINK] RECEIVED {}: SHA-256 {}", name, Hex(digest));
        Ok(())
    }

    fn open(&mut self, name: &str) -> Result<u64, u8> {
        let mut buf = [0u8; 320];
        let file = File::open(path(name, &mut buf)).map_err(io)?;
        let size = file.size() as u64;
        mind::println!("[LINK] SENDING {} ({} BYTES)", name, size);
        self.get = Some(file);
        Ok(size)
    }

    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> {
        let file = self.get.as_ref().ok_or(code::BAD_REQUEST)?;
        match file.read_at(offset as usize, out) { Ok(0) => Err(code::IO), Ok(n) => Ok(n), Err(e) => Err(io(e)) }
    }

    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> {
        let mut buf = [0u8; 320];
        let file = File::open(path(name, &mut buf)).map_err(io)?;
        let mut sha = mind::sha256::Sha256::new();
        let mut chunk = [0u8; 4096];
        let mut at = 0;
        while at < file.size() {
            let n = file.read_at(at, &mut chunk).map_err(io)?;
            if n == 0 { return Err(code::IO); }
            sha.update(&chunk[..n]);
            at += n;
        }
        Ok((at as u64, sha.finish()))
    }

    fn delete(&mut self, name: &str) -> Result<(), u8> {
        let mut buf = [0u8; 320];
        fs::remove(path(name, &mut buf)).map_err(io)?;
        mind::println!("[LINK] DELETED {}", name);
        Ok(())
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let (mut used, mut index, mut next) = (0usize, 0u32, None);
        fs::list(DIR, |e| {
            if e.is_dir || next.is_some() || e.name_str() == "link-part.tmp" { return; }
            index += 1;
            if index - 1 < cursor { return; }
            let mut line = Text::<300>::new();
            line.push(e.name_str()); line.push("\t"); line.number(e.size as u64); line.push("\n");
            if used + line.len > out.len() { next = Some(index - 1); return; }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }).map_err(io)?;
        Ok((used, next))
    }

    fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), u8> {
        let (mut used, mut next, mut full) = (0usize, cursor, false);
        while !full {
            let seen = mind::log::read(next, |e| {
                if full { return; }
                let mut line = Text::<300>::new();
                line.number(e.time_ms / 1000); line.push("."); line.number_width(e.time_ms % 1000, 3); line.push(" ");
                line.push(e.name.as_str()); line.push(": "); line.push(e.text.as_str()); line.push("\n");
                // A line longer than a whole answer is cut, so the log never stalls.
                let n = if used == 0 { line.len.min(out.len()) } else { line.len };
                if used + n > out.len() { full = true; return; }
                out[used..used + n].copy_from_slice(&line.bytes()[..n]);
                used += n;
                next = e.seq + 1;
            }).map_err(|_| code::DENIED)?;
            if seen == 0 { break; }
        }
        Ok((used, next))
    }
}

/// A line of text built without a heap.
struct Text<const N: usize> { buf: [u8; N], len: usize }
impl<const N: usize> Text<N> {
    fn new() -> Self { Self { buf: [0; N], len: 0 } }
    fn push(&mut self, s: &str) { let n = s.len().min(N - self.len); self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]); self.len += n; }
    fn number(&mut self, v: u64) { self.number_width(v, 1) }
    fn number_width(&mut self, mut v: u64, width: usize) {
        let mut digits = [0u8; 20]; let mut n = 0;
        while v > 0 || n < width { digits[n] = b'0' + (v % 10) as u8; v /= 10; n += 1; }
        for &d in digits[..n].iter().rev() { if self.len < N { self.buf[self.len] = d; self.len += 1; } }
    }
    fn bytes(&self) -> &[u8] { &self.buf[..self.len] }
}

struct Hex<'a>(&'a [u8]);
impl core::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for b in self.0 { write!(f, "{:02x}", b)?; } Ok(()) }
}

/// The service with each hello and refusal logged.
struct Logged(FileService<Store>);
impl Service for Logged {
    fn handle(&mut self, ch: u8, k: u8, p: &[u8], reply: &mut [u8]) -> Result<usize, u8> {
        if (ch, k) == (channel::CONTROL, kind::HELLO) {
            let name = p.get(4..).and_then(|n| core::str::from_utf8(n).ok()).unwrap_or("?");
            mind::println!("[LINK] HELLO FROM \"{}\"", name);
        }
        let r = self.0.handle(ch, k, p, reply);
        if let Err(c) = r { mind::println!("[LINK] REFUSED CHANNEL {} KIND {}: CODE {}", ch, k, c); }
        r
    }
}

/// The rate from the arguments (`baud=<rate>`), else 115200.
fn baud() -> u32 {
    mind::process::args_str().split_ascii_whitespace().find_map(|w| w.strip_prefix("baud=")?.parse().ok()).unwrap_or(FIRST_BAUD)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let endpoint = Endpoint(SLOT_DEV0);
    fs::use_scope(Endpoint(SLOT_VFS), DIR);
    let baud = baud();
    let mut said = false;
    loop {
        match serial::configure(endpoint, baud) {
            Ok(Ok(actual)) => { mind::println!("[LINK] SERVING data/link/ AND THE LOG AT {} BIT/S (THE CONVERTER MAKES {})", baud, actual); break; }
            Ok(Err(serial::Error::Invalid)) => { mind::println!("[LINK] {} BIT/S IS NOT A RATE THE CONVERTER MAKES", baud); return; }
            _ => { if !said { mind::println!("[LINK] WAITING FOR A SERIAL CONVERTER"); said = true; } mind::time::sleep(1000); }
        }
    }
    let mut service = Logged(FileService::new(Store { put: None, get: None }));
    service.0.set_name("MIND Core link");
    let mut slave = Slave::new(Port { endpoint, pending: [0; 4096], at: 0, len: 0 });
    loop {
        if slave.serve(&mut service, 1000).is_err() { mind::time::sleep(100); }
    }
}
