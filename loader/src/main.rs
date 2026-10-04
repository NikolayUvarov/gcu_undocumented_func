#![no_std]
#![no_main]
// loader: reads application ELFs from disk via vfs_server and starts them (SPAWN) at the request of the shell or
// programs; the kernel stores no application images. The loader decides which client capabilities an application gets.
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{Mapping, Pages};
use mind::idl::{loader, wire};
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::Error;
use mind::util::FixedBuf;

const RECEIVED_CAP: usize = 9;
const APP_ENDPOINTS: u16 = 4; // endpoints an application may create
// Loader's own slots (granted by init): its endpoint, client endpoints passed on to applications, spawn privilege.
const OWN_RTC: usize = 2; const OWN_VFS: usize = 3; const OWN_AUDIO: usize = 4; const OWN_TTS: usize = 6;
const MAX_IMAGE: usize = 4 * 1024 * 1024;

// Program name -> file: "clock" -> clock.elf; a path with a dot or directory is used as is.
fn path_for(name: &[u8], path: &mut FixedBuf<64>) -> Result<(), Error> {
    let text = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_graphic()) { return Err(Error::Invalid); }
    let _ = if text.contains('.') || text.contains('/') { write!(path, "{}", text) } else { write!(path, "{}.elf", text) };
    Ok(())
}

// Task name for ps: file name without directory and .elf extension, lowercased.
fn task_name(path: &[u8]) -> FixedBuf<NAME_MAX> {
    let file = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    let stem = if file.len() > 4 && file[file.len() - 4..].eq_ignore_ascii_case(b".elf") { &file[..file.len() - 4] } else { file };
    let mut name = FixedBuf::new();
    for &byte in stem.iter().take(NAME_MAX) { let _ = name.write_char(byte.to_ascii_lowercase() as char); }
    name
}

// What the program asks for: the `.mind_request` section (`mind::request!`): magic, version, flags. It grants nothing.
fn request_flags(read: &mut dyn FnMut(usize, &mut [u8]) -> usize) -> u32 {
    let mut header = [0u8; 64];
    if read(0, &mut header) < 64 || &header[..4] != b"\x7fELF" { return 0; }
    let u16_at = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]) as usize;
    let u64_at = |b: &[u8], at: usize| u64::from_le_bytes(b[at..at + 8].try_into().unwrap()) as usize;
    let (shoff, shentsize, shnum, shstrndx) = (u64_at(&header, 0x28), u16_at(&header, 0x3A), u16_at(&header, 0x3C), u16_at(&header, 0x3E));
    if shentsize != 64 || shnum == 0 || shnum > 64 || shstrndx >= shnum { return 0; }
    let mut sections = [0u8; 64 * 64];
    if read(shoff, &mut sections[..shnum * 64]) < shnum * 64 { return 0; }
    let strtab = &sections[shstrndx * 64..shstrndx * 64 + 64];
    let (str_offset, str_size) = (u64_at(strtab, 0x18), u64_at(strtab, 0x20).min(4096));
    let mut names = [0u8; 4096];
    if read(str_offset, &mut names[..str_size]) < str_size { return 0; }
    for index in 0..shnum {
        let section = &sections[index * 64..index * 64 + 64];
        let name = u32::from_le_bytes(section[..4].try_into().unwrap()) as usize;
        if name >= str_size || !names[name..str_size].starts_with(b".mind_request\0") { continue; }
        let mut note = [0u8; 16];
        if u64_at(section, 0x20) < 16 || read(u64_at(section, 0x18), &mut note) < 16 { return 0; }
        return if &note[..8] == mind::process::REQUEST_MAGIC { u32::from_le_bytes(note[8..12].try_into().unwrap()) } else { 0 };
    }
    0
}

// The program file of `name` and its task name; boot services and the kernel are not applications.
fn open(name: &[u8]) -> Result<(File, FixedBuf<NAME_MAX>), Error> {
    let mut path = FixedBuf::<64>::new();
    path_for(name, &mut path)?;
    let task = task_name(path.as_bytes());
    // Services are started by init from bootloader images; the kernel itself is not runnable as a program.
    if task.as_bytes() == b"kernel" || BOOT_SERVICES.iter().any(|s| s.as_bytes() == task.as_bytes()) { return Err(Error::NotFound); }
    Ok((File::open(core::str::from_utf8(path.as_bytes()).unwrap())?, task))
}

// Starts `name` with the standard client endpoints, an optional endpoint in the INIT slot and the capabilities `extra`
// (handles in this task, moved into the child's slots). A program that asked to be a console program gets no screen.
fn load(name: &[u8], args: &[u8], init: Option<usize>, extra: &[(u8, usize)]) -> Result<u64, Error> {
    let (file, task) = open(name)?;
    let size = file.size();
    if size < 64 || size > MAX_IMAGE { return Err(Error::Invalid); }
    let mut image = Pages::new(size).ok_or(Error::NoMemory)?;
    if file.read_at(0, &mut image.as_mut_slice()[..size])? != size || &image.as_slice()[..4] != b"\x7fELF" { return Err(Error::Invalid); }
    let flags = { let bytes = &image.as_slice()[..size]; request_flags(&mut |at, out: &mut [u8]| { let n = out.len().min(bytes.len().saturating_sub(at)); out[..n].copy_from_slice(&bytes[at..at + n]); n }) };
    let cap = image.share()?;
    let client = CAP_WRITE | CAP_GRANT;
    let mut grants = [Grant::default(); SPAWN_GRANTS_MAX];
    let standard = [grant(SLOT_RTC, OWN_RTC, client), grant(SLOT_VFS, OWN_VFS, client), grant(SLOT_AUDIO, OWN_AUDIO, client),
                    grant(SLOT_LOADER, SLOT_SERVICE, client), grant(SLOT_TTS, OWN_TTS, client)];
    grants[..5].copy_from_slice(&standard);
    let mut count = 5;
    if let Some(slot) = init { grants[count] = grant(SLOT_INIT, slot, CAP_READ | CAP_WRITE | CAP_GRANT); count += 1; }
    for &(child, handle) in extra { grants[count] = grant_moved(child as usize, handle, u8::MAX); count += 1; }
    // SPAWN takes `name\0arguments`.
    let mut text = [0u8; NAME_MAX + 1 + ARGS_MAX];
    let args = &args[..args.len().min(ARGS_MAX)];
    text[..task.as_bytes().len()].copy_from_slice(task.as_bytes());
    let mut len = task.as_bytes().len();
    if !args.is_empty() { text[len + 1..len + 1 + args.len()].copy_from_slice(args); len += 1 + args.len(); }
    let screen = if flags & mind::process::REQUEST_CONSOLE != 0 { 0 } else { SPAWN_SCREEN };
    // Each application may create a few endpoints (taken from loader's quota) and cannot spawn by itself.
    let result = mind::process::spawn_raw(&text[..len], Image::Memory { cap, len: size }, &grants[..count], screen, Quota { tasks: 0, endpoints: APP_ENDPOINTS });
    let _ = ipc::drop_cap(cap); // the kernel has already copied the image; the buffer is freed when the function returns
    result
}

// A launch in progress (idl/loader.wit): who started it, what to run, the capabilities lent for the program's slots.
struct Session { id: u32, owner: u64, name: [u8; 64], name_len: usize, args: [u8; ARGS_MAX], args_len: usize, grants: [(u8, usize); 5], count: usize }

impl Session {
    fn drop_grants(&mut self) { for &(_, handle) in &self.grants[..self.count] { let _ = ipc::drop_cap(handle); } self.count = 0; }
}

const SESSIONS: usize = 4;

struct Launcher { sessions: [Option<Session>; SESSIONS], next: u32 }

impl Launcher {
    fn find(&mut self, id: u32, owner: u64) -> Option<usize> { self.sessions.iter().position(|s| s.as_ref().is_some_and(|s| s.id == id && s.owner == owner)) }

    fn begin(&mut self, owner: u64, name: &str, args: &str) -> Result<u32, loader::Error> {
        if name.is_empty() || args.len() > ARGS_MAX { return Err(loader::Error::Invalid); }
        // Sessions of clients that died meanwhile are dropped when the table is full.
        if self.sessions.iter().all(Option::is_some) {
            for slot in self.sessions.iter_mut() { if slot.as_ref().is_some_and(|s| !mind::process::alive(s.owner)) { if let Some(mut s) = slot.take() { s.drop_grants(); } } }
        }
        let index = self.sessions.iter().position(Option::is_none).ok_or(loader::Error::Sessions)?;
        self.next = self.next.wrapping_add(1).max(1);
        let mut session = Session { id: self.next, owner, name: [0; 64], name_len: name.len(), args: [0; ARGS_MAX], args_len: args.len(), grants: [(0, 0); 5], count: 0 };
        session.name[..name.len()].copy_from_slice(name.as_bytes()); session.args[..args.len()].copy_from_slice(args.as_bytes());
        self.sessions[index] = Some(session);
        Ok(self.next)
    }

    fn grant(&mut self, owner: u64, id: u32, slot: u8) -> Result<(), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        if !(7..=12).contains(&slot) { return Err(loader::Error::Invalid); }
        // The capability arrived in the receive slot; keep a copy in a slot of our own until the program starts.
        let handle = ipc::mint(RECEIVED_CAP, u8::MAX, 0, 0).map_err(|_| loader::Error::NoMemory)?;
        let session = self.sessions[index].as_mut().unwrap();
        if let Some(existing) = session.grants[..session.count].iter_mut().find(|g| g.0 == slot) { let _ = ipc::drop_cap(existing.1); existing.1 = handle; return Ok(()); }
        if session.count == session.grants.len() { let _ = ipc::drop_cap(handle); return Err(loader::Error::Limit); }
        session.grants[session.count] = (slot, handle); session.count += 1;
        Ok(())
    }

    fn commit(&mut self, owner: u64, id: u32) -> Result<u64, loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        let mut session = self.sessions[index].take().unwrap();
        let result = load(&session.name[..session.name_len], &session.args[..session.args_len], None, &session.grants[..session.count]);
        if result.is_err() { session.drop_grants(); }
        result.map_err(|error| match error {
            Error::NotFound => loader::Error::NotFound, Error::NoMemory => loader::Error::NoMemory, Error::Rights => loader::Error::Rights,
            Error::Other(ERR_LIMIT) => loader::Error::Limit, Error::Other(ERR_BUSY) => loader::Error::Busy, _ => loader::Error::Invalid,
        })
    }

    fn abort(&mut self, owner: u64, id: u32) -> Result<(), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        self.sessions[index].take().unwrap().drop_grants();
        Ok(())
    }
}

fn inspect(name: &str) -> Result<loader::Needs, loader::Error> {
    let (file, _) = open(name.as_bytes()).map_err(|error| if error == Error::NotFound { loader::Error::NotFound } else { loader::Error::Invalid })?;
    let flags = request_flags(&mut |at, out: &mut [u8]| file.read_at(at, out).unwrap_or(0));
    use mind::process::*;
    Ok(loader::Needs { console: flags & REQUEST_CONSOLE != 0, sysinfo: flags & REQUEST_SYSINFO != 0, file: flags & REQUEST_FILE != 0, lifecycle: flags & REQUEST_LIFECYCLE != 0, log: flags & REQUEST_LOG != 0 })
}

// A request in the MIND IDL protocol (idl/loader.wit): method 1..5 and major version 1 in the low bytes. The older
// protocol's first word is a program name (printable bytes) or 0.
fn idl_request(launcher: &mut Launcher, request: &ipc::Received) {
    let decoded = loader::decode(request, RECEIVED_CAP);
    let mut mapping = match decoded { Ok(loader::Request::Begin { .. } | loader::Request::Inspect { .. }) => Mapping::new(RECEIVED_CAP).ok(), _ => None };
    let mut empty = [0u8; 0];
    let bytes: &mut [u8] = match mapping.as_mut() { Some(m) => m.as_mut_slice(), None => &mut empty };
    let _ = match decoded {
        Ok(loader::Request::Begin { payload, .. }) => match loader::args_begin(bytes, payload) {
            Ok((name, args)) => { let (mut n, mut a) = ([0u8; 64], [0u8; ARGS_MAX]); n[..name.len()].copy_from_slice(name.as_bytes()); a[..args.len()].copy_from_slice(args.as_bytes());
                loader::reply_begin(launcher.begin(request.sender, core::str::from_utf8(&n[..name.len()]).unwrap_or(""), core::str::from_utf8(&a[..args.len()]).unwrap_or(""))) }
            Err(reason) => wire::reject(reason),
        },
        Ok(loader::Request::Grant { session, slot, .. }) => loader::reply_grant(launcher.grant(request.sender, session, slot)),
        Ok(loader::Request::Commit { session }) => loader::reply_commit(launcher.commit(request.sender, session)),
        Ok(loader::Request::Abort { session }) => loader::reply_abort(launcher.abort(request.sender, session)),
        Ok(loader::Request::Inspect { payload, .. }) => match loader::args_inspect(bytes, payload) {
            Ok(name) => { let mut n = [0u8; 64]; n[..name.len()].copy_from_slice(name.as_bytes()); let result = inspect(core::str::from_utf8(&n[..name.len()]).unwrap_or("")); loader::reply_inspect(bytes, result) }
            Err(reason) => wire::reject(reason),
        },
        Err(reason) => wire::reject(reason),
    };
    drop(mapping);
    if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
}

// Text for LIST: *.elf programs in the disk root, except the kernel; services are marked.
fn listing(out: &mut [u8]) -> usize {
    let mut at = 0;
    let _ = fs::list("", |entry| {
        if entry.is_dir || entry.name.len() < 5 || !entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".elf") { return; }
        let name = task_name(entry.name);
        if name.as_bytes() == b"kernel" { return; }
        let mut line = FixedBuf::<64>::new();
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes() == name.as_bytes());
        let _ = writeln!(line, "  {:<12} {} BYTES{}", core::str::from_utf8(name.as_bytes()).unwrap_or("?"), entry.size, if service { " (SERVICE)" } else { "" });
        if at + line.as_bytes().len() <= out.len() { out[at..at + line.as_bytes().len()].copy_from_slice(line.as_bytes()); at += line.as_bytes().len(); }
    });
    at
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let _ = fs::prepare();
    mind::println!("[LOADER] READY");
    let mut launcher = Launcher { sessions: [const { None }; SESSIONS], next: 0 };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if !request.is_call {
            if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
            continue;
        }
        if (1..=5).contains(&(request.data[0] & 0xFF)) && (request.data[0] >> 8) & 0xFF == 1 { idl_request(&mut launcher, &request); continue; }
        let code = if request.data == [0, LOADER_LIST] {
            // Program list into the caller's memory page.
            match Mapping::new(RECEIVED_CAP) { Ok(mut page) => listing(page.as_mut_slice()), Err(error) => error.code() }
        } else if request.data == [0, LOADER_RUN] {
            // Start with arguments: the page holds `name\0arguments\0`.
            match Mapping::new(RECEIVED_CAP) {
                Ok(page) => {
                    let bytes = page.as_slice();
                    let name_end = bytes.iter().position(|&b| b == 0).unwrap_or(0);
                    let rest = &bytes[(name_end + 1).min(bytes.len())..];
                    let args = &rest[..rest.iter().position(|&b| b == 0).unwrap_or(rest.len()).min(ARGS_MAX)];
                    if name_end == 0 || name_end > NAME_MAX { ERR_INVALID } else { match load(&bytes[..name_end], args, None, &[]) { Ok(pid) => pid as usize, Err(error) => error.code() } }
                }
                Err(error) => error.code(),
            }
        } else {
            // Spawn: name in two message words, optional endpoint for the child's INIT slot.
            let (packed, len) = mind::process::unpack_name(request.data);
            match load(&packed[..len], &[], request.cap_received.then_some(RECEIVED_CAP), &[]) { Ok(pid) => pid as usize, Err(error) => error.code() }
        };
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        let _ = ipc::reply(&Message::new(code, 0));
    }
}
