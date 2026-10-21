#![no_std]
#![no_main]
// link (800-LNK-0003): MIND Core's end of MIND Link, the slave of linkproto (docs/link/protocol.md) over usb_serial's
// port (idl/serial.wit, SLOT_DEV0). Its authority is what init grants (MC-2.3): files only in data/link/ (a vfs client
// confined there, SLOT_VFS), the system log through a reading log client (SLOT_LOG), an application's loader client
// (SLOT_LOADER) and no console. init starts it when the boot volume holds link.conf at its root and passes that file's
// words as arguments: `baud=<rate>`, and `run`, without which it starts no program (800-LNK-0009). Files may lie in
// directories below data/link/ (800-LNK-0010). Everything from the line is checked by linkproto before use (MC-11.4);
// hellos, files, programs and refusals are logged (MC-10.2).
use linkproto::app::{FileService, Files, RunOutput, EXIT_UNKNOWN, RUN_ENDED, RUN_RUNNING};
use linkproto::endpoint::{channel, code, kind};
use linkproto::{Line, Service, Slave};
use mind::abi::{BootInfo, CAP_WRITE, SLOT_CONSOLE, SLOT_DEV0, SLOT_VFS};
use mind::fs::{self, File};
use mind::idl::{loader, serial};
use mind::ipc::{self, Endpoint};

const DIR: &str = "data/link";
/// Where a put is written until its SHA-256 matches; left out of listings.
const PART: &str = "data/link/link-part.tmp";
const FIRST_BAUD: u32 = 115_200;
/// The longest path `loader` takes (idl/loader.wit `begin`).
const LOADER_PATH_MAX: usize = 64;

/// The port as linkproto's line: bytes through usb_serial, waiting with its `wait`.
struct Port { endpoint: Endpoint, pending: [u8; 4096], at: usize, len: usize }

impl Line for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        bytes.chunks(4096).all(|chunk| matches!(serial::write(self.endpoint, chunk), Ok(Ok(()))))
    }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        if self.at == self.len {
            // While a program runs, its output is taken between short waits, so it is not kept waiting.
            let end = mind::time::uptime_ms() as u64 + timeout_ms.min(1000) as u64;
            loop {
                let running = programs().pump();
                let left = end.saturating_sub(mind::time::uptime_ms() as u64) as u32;
                let waiting = serial::wait(self.endpoint, if running { left.min(20) } else { left }).unwrap_or(0);
                if waiting > 0 { break; }
                if mind::time::uptime_ms() as u64 >= end { return 0; }
            }
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
    match e {
        fs::Error::NotFound => code::NOT_FOUND,
        fs::Error::Exists => code::EXISTS,
        fs::Error::NotEmpty => code::NOT_EMPTY,
        fs::Error::Denied | fs::Error::ReadOnly | fs::Error::Name | fs::Error::NotDirectory | fs::Error::IsDirectory | fs::Error::Invalid => code::DENIED,
        _ => code::IO,
    }
}

/// A listing's own entries, which FAT keeps in each directory, and the put in progress.
fn hidden(name: &str, top: bool) -> bool { name == "." || name == ".." || top && name == "link-part.tmp" }

/// Removes directory `top` and everything in it, without a heap: the first entry each time, a file removed, a directory
/// entered, and each emptied directory removed on the way back up.
fn remove_tree(top: &str) -> Result<(), u8> {
    let mut at = [0u8; 320];
    let mut len = top.len();
    at[..len].copy_from_slice(top.as_bytes());
    loop {
        let current = core::str::from_utf8(&at[..len]).map_err(|_| code::IO)?;
        let mut first: Option<([u8; 255], usize, bool)> = None;
        fs::list(current, |e| {
            if first.is_some() || hidden(e.name_str(), false) { return; }
            let mut name = [0u8; 255];
            let n = e.name.len().min(255);
            name[..n].copy_from_slice(&e.name[..n]);
            first = Some((name, n, e.is_dir));
        }).map_err(io)?;
        let Some((name, n, is_dir)) = first else {
            fs::remove(current).map_err(io)?;
            if len == top.len() { return Ok(()); }
            len = at[..len].iter().rposition(|&b| b == b'/').unwrap_or(0);
            continue;
        };
        if len + 1 + n > at.len() { return Err(code::IO); }
        at[len] = b'/';
        at[len + 1..len + 1 + n].copy_from_slice(&name[..n]);
        if is_dir { len += 1 + n; } else { fs::remove(core::str::from_utf8(&at[..len + 1 + n]).map_err(|_| code::IO)?).map_err(io)?; }
    }
}

/// The programs link started for the master (800-LNK-0009): their output arrives at `inbox` (each holds a send copy in
/// SLOT_CONSOLE, `mind::output`'s 15-byte messages) and the last KEEP bytes of each are kept for OUTPUT.
struct Programs { inbox: Option<Endpoint>, output: usize, allowed: bool, next: u32, runs: [Run; RUNS], checked: u64 }
struct Run { id: u32, pid: u64, state: u8, name: [u8; LOADER_PATH_MAX], name_len: usize, total: u64, kept: [u8; KEEP] }
const RUNS: usize = 8;
const KEEP: usize = 16 * 1024;
const NO_RUN: Run = Run { id: 0, pid: 0, state: RUN_ENDED, name: [0; LOADER_PATH_MAX], name_len: 0, total: 0, kept: [0; KEEP] };

fn loader_code(error: loader::Error) -> u8 {
    match error {
        loader::Error::NotFound => code::NOT_FOUND,
        loader::Error::Invalid => code::BAD_REQUEST,
        loader::Error::Rights => code::DENIED,
        loader::Error::Unreadable => code::IO,
        loader::Error::NoMemory | loader::Error::Limit | loader::Error::Busy | loader::Error::Sessions => code::BUSY,
    }
}

impl Run {
    fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("?") }
    fn print(&mut self, bytes: &[u8]) {
        for &b in bytes { self.kept[(self.total % KEEP as u64) as usize] = b; self.total += 1; }
    }
}

impl Programs {
    /// Output taken from the inbox, and the ends of programs seen; whether one still runs.
    fn pump(&mut self) -> bool {
        let Some(inbox) = self.inbox else { return false };
        let mut chunk = [0u8; mind::output::CHUNK];
        while let Ok(m) = inbox.recv_timeout(0, 1) {
            let n = mind::output::unpack(m.data, &mut chunk);
            if let Some(run) = self.runs.iter_mut().find(|r| r.id != 0 && r.pid == m.sender) { run.print(&chunk[..n]); }
        }
        let now = mind::time::uptime_ms() as u64;
        if now >= self.checked + 200 {
            self.checked = now;
            for run in self.runs.iter_mut().filter(|r| r.id != 0 && r.state == RUN_RUNNING && !mind::process::alive(r.pid)) {
                run.state = RUN_ENDED;
                mind::println!("[LINK] RUN {} ENDED: {} (PID {}, {} BYTES OF OUTPUT)", run.id, run.name(), run.pid, run.total);
            }
        }
        self.runs.iter().any(|r| r.id != 0 && r.state == RUN_RUNNING)
    }

    fn find(&mut self, id: u32) -> Result<&mut Run, u8> { self.runs.iter_mut().find(|r| r.id != 0 && r.id == id).ok_or(code::NOT_FOUND) }

    fn start(&mut self, full: &str, args: &str) -> Result<(u32, u64), u8> {
        if !self.allowed { mind::println!("[LINK] RUN {} REFUSED: link.conf HAS NO \"run\"", full); return Err(code::DENIED); }
        if self.inbox.is_none() { return Err(code::UNSUPPORTED); }
        if full.len() > LOADER_PATH_MAX { return Err(code::DENIED); }
        self.pump();
        // A free entry, else the one that ended longest ago.
        let index = self.runs.iter().position(|r| r.id == 0)
            .or_else(|| self.runs.iter().enumerate().filter(|(_, r)| r.state != RUN_RUNNING).min_by_key(|(_, r)| r.id.wrapping_sub(self.next)).map(|(i, _)| i))
            .ok_or(code::BUSY)?;
        let lost = |_| code::BUSY;
        let session = loader::begin(Endpoint::LOADER, full, args).map_err(lost)?.map_err(loader_code)?;
        if !matches!(loader::grant(Endpoint::LOADER, session, SLOT_CONSOLE as u8, self.output), Ok(Ok(()))) {
            let _ = loader::abort(Endpoint::LOADER, session);
            return Err(code::IO);
        }
        let pid = loader::commit(Endpoint::LOADER, session).map_err(lost)?.map_err(loader_code)?;
        let id = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        let run = &mut self.runs[index];
        (run.id, run.pid, run.state, run.total, run.name_len) = (id, pid, RUN_RUNNING, 0, full.len());
        run.name[..full.len()].copy_from_slice(full.as_bytes());
        mind::println!("[LINK] RUN {}: STARTED {} AS PID {}", id, full, pid);
        Ok((id, pid))
    }

    fn output(&mut self, id: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> {
        self.pump();
        let run = self.find(id)?;
        let first = run.total.saturating_sub(KEEP as u64);
        let from = cursor.clamp(first, run.total);
        let n = ((run.total - from) as usize).min(out.len());
        for (i, b) in out[..n].iter_mut().enumerate() { *b = run.kept[((from + i as u64) % KEEP as u64) as usize]; }
        Ok(RunOutput { state: run.state, exit: EXIT_UNKNOWN, from, len: n })
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        self.pump();
        let (mut used, mut index) = (0usize, 0u32);
        for run in self.runs.iter().filter(|r| r.id != 0) {
            index += 1;
            if index - 1 < cursor { continue; }
            let mut line = Text::<160>::new();
            line.number(run.id as u64); line.push("\t"); line.number(run.pid); line.push("\t");
            line.push(if run.state == RUN_RUNNING { "running" } else { "ended" }); line.push("\t"); line.push(run.name()); line.push("\n");
            if used + line.len > out.len() { return Ok((used, Some(index - 1))); }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }
        Ok((used, None))
    }
}

static mut PROGRAMS: Programs = Programs { inbox: None, output: 0, allowed: false, next: 1, runs: [NO_RUN; RUNS], checked: 0 };
fn programs() -> &'static mut Programs { unsafe { &mut *core::ptr::addr_of_mut!(PROGRAMS) } }

/// The files of data/link/ and the system log. A put's data is gathered and written 16 KiB at a time, a get's read 16
/// KiB ahead: each request carries at most 1 KiB, and a write to the volume costs far more than its bytes.
struct Store { put: Option<(File, [u8; 255], usize)>, get: Option<File>, buffer: [u8; CHUNK], start: usize, len: usize, known: [Known; KNOWN] }

/// The SHA-256 of files received here, so `stat` answers at once for them; the oldest is forgotten first.
#[derive(Clone, Copy)]
struct Known { name: [u8; 255], len: usize, size: u64, digest: [u8; 32] }
const KNOWN: usize = 8;
const UNKNOWN: Known = Known { name: [0; 255], len: 0, size: 0, digest: [0; 32] };

const CHUNK: usize = 16 * 1024;

impl Store {
    fn forget(&mut self, name: &str) {
        for k in self.known.iter_mut().filter(|k| &k.name[..k.len] == name.as_bytes()) { *k = UNKNOWN; }
    }

    fn remember(&mut self, name: &str, size: u64, digest: &[u8; 32]) {
        self.forget(name);
        self.known.rotate_right(1);
        let k = &mut self.known[0];
        k.name[..name.len()].copy_from_slice(name.as_bytes());
        (k.len, k.size, k.digest) = (name.len(), size, *digest);
    }

    // The gathered bytes of a put written out.
    fn drain(&mut self) -> Result<(), u8> {
        let Some((file, _, _)) = self.put.as_mut() else { return Ok(()) };
        if self.len > 0 {
            match file.write_at(self.start, &self.buffer[..self.len]) { Ok(n) if n == self.len => {}, Ok(_) => return Err(code::IO), Err(e) => return Err(io(e)) }
        }
        self.start += self.len;
        self.len = 0;
        Ok(())
    }
}

impl Files for Store {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        if size > u32::MAX as u64 { return Err(code::IO); }
        self.put = None;
        // A put into a directory that is missing would fail only at its end.
        if let Some((parent, _)) = name.rsplit_once('/') {
            let mut buf = [0u8; 320];
            if !fs::metadata(path(parent, &mut buf)).is_ok_and(|m| m.is_dir) { return Err(code::NOT_FOUND); }
        }
        let file = File::create(PART).map_err(io)?;
        (self.start, self.len) = (0, 0);
        let mut kept = [0u8; 255];
        kept[..name.len()].copy_from_slice(name.as_bytes());
        mind::println!("[LINK] RECEIVING {} ({} BYTES)", name, size);
        self.put = Some((file, kept, name.len()));
        Ok(())
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> {
        if self.put.is_none() || offset as usize != self.start + self.len { return Err(code::BAD_REQUEST); }
        let mut rest = data;
        while !rest.is_empty() {
            let n = rest.len().min(CHUNK - self.len);
            self.buffer[self.len..self.len + n].copy_from_slice(&rest[..n]);
            self.len += n;
            rest = &rest[n..];
            if self.len == CHUNK { self.drain()?; }
        }
        Ok(())
    }

    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8> {
        let drained = self.drain();
        let (file, kept, len) = self.put.take().ok_or(code::BAD_REQUEST)?;
        drained?;
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
        self.forget(name);
        fs::rename(PART, target).map_err(io)?;
        self.remember(name, self.start as u64, digest);
        mind::println!("[LINK] RECEIVED {}: SHA-256 {}", name, Hex(digest));
        Ok(())
    }

    fn open(&mut self, name: &str) -> Result<u64, u8> {
        let mut buf = [0u8; 320];
        let file = File::open(path(name, &mut buf)).map_err(io)?;
        let size = file.size() as u64;
        mind::println!("[LINK] SENDING {} ({} BYTES)", name, size);
        self.get = Some(file);
        (self.start, self.len) = (0, 0);
        Ok(size)
    }

    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> {
        let file = self.get.as_ref().ok_or(code::BAD_REQUEST)?;
        let offset = offset as usize;
        if offset < self.start || offset >= self.start + self.len {
            self.len = match file.read_at(offset, &mut self.buffer) { Ok(0) => return Err(code::IO), Ok(n) => n, Err(e) => return Err(io(e)) };
            self.start = offset;
        }
        let at = offset - self.start;
        let n = out.len().min(self.len - at);
        out[..n].copy_from_slice(&self.buffer[at..at + n]);
        Ok(n)
    }

    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> {
        let mut buf = [0u8; 320];
        let file = File::open(path(name, &mut buf)).map_err(io)?;
        if let Some(k) = self.known.iter().find(|k| k.len > 0 && &k.name[..k.len] == name.as_bytes() && k.size == file.size() as u64) { return Ok((k.size, k.digest)); }
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
        self.forget(name);
        mind::println!("[LINK] DELETED {}", name);
        Ok(())
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let (mut used, mut index, mut next) = (0usize, 0u32, None);
        fs::list(DIR, |e| {
            if e.is_dir || next.is_some() || hidden(e.name_str(), true) { return; }
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

    fn mkdir(&mut self, name: &str) -> Result<(), u8> {
        let mut buf = [0u8; 320];
        let target = path(name, &mut buf);
        if fs::metadata(target).is_ok_and(|m| !m.is_dir) { return Err(code::EXISTS); }
        fs::mkdir(target).map_err(io)?;
        mind::println!("[LINK] MADE {}/", name);
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str, replace: bool) -> Result<(), u8> {
        if to.len() > from.len() && to.starts_with(from) && to.as_bytes()[from.len()] == b'/' { return Err(code::DENIED); }
        let (mut a, mut b) = ([0u8; 320], [0u8; 320]);
        let (source, target) = (path(from, &mut a), path(to, &mut b));
        fs::metadata(source).map_err(io)?;
        match fs::metadata(target) {
            Ok(m) if m.is_dir || !replace => return Err(code::EXISTS),
            Ok(_) => fs::remove(target).map_err(io)?,
            Err(_) => {}
        }
        fs::rename(source, target).map_err(io)?;
        self.known = [UNKNOWN; KNOWN];
        mind::println!("[LINK] MOVED {} TO {}", from, to);
        Ok(())
    }

    fn rmdir(&mut self, name: &str, recursive: bool) -> Result<(), u8> {
        let mut buf = [0u8; 320];
        let target = path(name, &mut buf);
        if !fs::metadata(target).map_err(io)?.is_dir { return Err(code::DENIED); }
        if recursive { remove_tree(target)?; } else { fs::remove(target).map_err(io)?; }
        self.known = [UNKNOWN; KNOWN];
        mind::println!("[LINK] REMOVED {}/{}", name, if recursive { " AND ALL IN IT" } else { "" });
        Ok(())
    }

    fn list_dir(&mut self, name: &str, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let mut buf = [0u8; 320];
        let (dir, top) = if name.is_empty() { (DIR, true) } else { (path(name, &mut buf), false) };
        let (mut used, mut index, mut next) = (0usize, 0u32, None);
        fs::list(dir, |e| {
            if next.is_some() || hidden(e.name_str(), top) { return; }
            index += 1;
            if index - 1 < cursor { return; }
            let mut line = Text::<300>::new();
            line.push(e.name_str());
            if e.is_dir { line.push("/\t0\n"); } else { line.push("\t"); line.number(e.size as u64); line.push("\n"); }
            if used + line.len > out.len() { next = Some(index - 1); return; }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }).map_err(io)?;
        Ok((used, next))
    }

    fn run(&mut self, name: &str, args: &str) -> Result<(u32, u64), u8> {
        let mut buf = [0u8; 320];
        programs().start(path(name, &mut buf), args)
    }

    fn output(&mut self, run: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> { programs().output(run, cursor, out) }

    /// Ending a program needs a grant init does not give yet (800-KRN-0132's point 3).
    fn stop(&mut self, run: u32) -> Result<(), u8> { programs().find(run).map(|_| ())?; Err(code::UNSUPPORTED) }

    fn runs(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> { programs().list(cursor, out) }

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

static mut STORE: Store = Store { put: None, get: None, buffer: [0; CHUNK], start: 0, len: 0, known: [UNKNOWN; KNOWN] };

/// The service with each hello and refusal logged.
struct Logged(FileService<&'static mut Store>);
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

/// Programs may be started only with the word `run` in link.conf; their output comes to an endpoint of link's.
fn prepare_programs() {
    let p = programs();
    p.allowed = mind::process::args_str().split_ascii_whitespace().any(|w| w == "run");
    p.next = (mind::time::rdtsc() as u32 & 0xFFFF_0000) | 1;
    if !p.allowed { return; }
    match Endpoint::create().and_then(|inbox| Ok((inbox, ipc::mint(inbox.0, CAP_WRITE, 0, 0)?))) {
        Ok((inbox, output)) => { (p.inbox, p.output) = (Some(inbox), output); mind::println!("[LINK] PROGRAMS IN data/link/ MAY BE STARTED (link.conf: run)"); }
        Err(e) => mind::println!("[LINK] NO ENDPOINT FOR PROGRAMS' OUTPUT ({:?}): NONE WILL BE STARTED", e),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("link — MIND Link's end on this machine: files in data/link/ and its directories, the system log, and with the word run in link.conf the programs in data/link/, for mindlink on another machine, over a serial converter; init starts it when the boot volume holds link.conf (800-LNK-0003).");
    let endpoint = Endpoint(SLOT_DEV0);
    fs::use_scope(Endpoint(SLOT_VFS), DIR);
    let baud = baud();
    let mut said = false;
    loop {
        match serial::configure(endpoint, baud) {
            Ok(Ok(actual)) => { mind::println!("[LINK] SERVING data/link/ AND THE LOG AT {} BIT/S (THE CONVERTER MAKES {})", baud, actual); break; }
            Ok(Err(serial::Error::Invalid)) => { mind::println!("[LINK] {} BIT/S IS NOT A RATE THE CONVERTER MAKES", baud); return; }
            Ok(Err(_)) => { if !said { mind::println!("[LINK] WAITING FOR A SERIAL CONVERTER"); said = true; } mind::time::sleep(1000); }
            // No usb_serial behind the slot: started by hand, not by init.
            Err(_) => { mind::println!("link: no serial port was given to it; init starts it when the boot volume holds link.conf"); return; }
        }
    }
    prepare_programs();
    let store: &'static mut Store = unsafe { &mut *core::ptr::addr_of_mut!(STORE) };
    let mut service = Logged(FileService::new(store));
    service.0.set_name("MIND Core link");
    let mut slave = Slave::new(Port { endpoint, pending: [0; 4096], at: 0, len: 0 });
    let mut dropped = 0;
    loop {
        if slave.serve(&mut service, 1000).is_err() { mind::time::sleep(100); }
        // Damaged frames say the line is noisy or the rates differ.
        if slave.stats.dropped != dropped {
            dropped = slave.stats.dropped;
            mind::println!("[LINK] {} DAMAGED FRAMES DROPPED SO FAR", dropped);
        }
    }
}
