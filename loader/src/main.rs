#![no_std]
#![no_main]
// loader: reads application ELFs from disk via vfs_server and starts them (SPAWN) at the request of the shell or
// programs; the kernel stores no application images. The loader decides which client capabilities an application gets.
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::idl::codec::{List, Text};
use mind::idl::{loader, wire};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Pages;
use mind::process::{grant, Image, Quota};
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

fn load(name: &[u8], args: &[u8], init: Option<usize>) -> Result<u64, Error> {
    let mut path = FixedBuf::<64>::new();
    path_for(name, &mut path)?;
    let task = task_name(path.as_bytes());
    // Services are started by init from bootloader images; the kernel itself is not runnable as a program.
    if task.as_bytes() == b"kernel" || BOOT_SERVICES.iter().any(|s| s.as_bytes() == task.as_bytes()) { return Err(Error::NotFound); }
    let file = File::open(core::str::from_utf8(path.as_bytes()).unwrap())?;
    let size = file.size();
    if size < 64 || size > MAX_IMAGE { return Err(Error::Invalid); }
    let mut image = Pages::new(size).ok_or(Error::NoMemory)?;
    if file.read_at(0, &mut image.as_mut_slice()[..size])? != size || &image.as_slice()[..4] != b"\x7fELF" { return Err(Error::Invalid); }
    let cap = image.share()?;
    let client = CAP_WRITE | CAP_GRANT;
    let standard = [grant(SLOT_RTC, OWN_RTC, client), grant(SLOT_VFS, OWN_VFS, client), grant(SLOT_AUDIO, OWN_AUDIO, client),
                    grant(SLOT_LOADER, SLOT_SERVICE, client), grant(SLOT_TTS, OWN_TTS, client), grant(SLOT_INIT, init.unwrap_or(0), CAP_READ | CAP_WRITE | CAP_GRANT)];
    let grants = if init.is_some() { &standard[..] } else { &standard[..5] };
    // SPAWN takes `name\0arguments`.
    let mut text = [0u8; NAME_MAX + 1 + ARGS_MAX];
    let args = &args[..args.len().min(ARGS_MAX)];
    text[..task.as_bytes().len()].copy_from_slice(task.as_bytes());
    let mut len = task.as_bytes().len();
    if !args.is_empty() { text[len + 1..len + 1 + args.len()].copy_from_slice(args); len += 1 + args.len(); }
    // Each application may create a few endpoints (taken from loader's quota) and cannot spawn by itself.
    let result = mind::process::spawn_raw(&text[..len], Image::Memory { cap, len: size }, grants, SPAWN_SCREEN, Quota { tasks: 0, endpoints: APP_ENDPOINTS });
    let _ = ipc::drop_cap(cap); // the kernel has already copied the image; the buffer is freed when the function returns
    result
}

// The *.elf programs in the disk root, except the kernel; services are marked.
fn programs() -> List<loader::Program, 64> {
    let mut list = List::default();
    let _ = fs::list("", |entry| {
        if entry.is_dir || entry.name.len() < 5 || !entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".elf") { return; }
        let name = task_name(entry.name);
        if name.as_bytes() == b"kernel" { return; }
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes() == name.as_bytes());
        if let Some(name) = Text::new(core::str::from_utf8(name.as_bytes()).unwrap_or("?")) { list.push(loader::Program { name, size: entry.size as u64, service }); }
    });
    list
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let _ = fs::prepare();
    mind::println!("[LOADER] READY");
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if !request.is_call {
            if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
            continue;
        }
        // Requests in idl/loader.wit; the legacy start-with-endpoint request (a program name packed into the words, never
        // carrying the interface's version byte) remains as a bounded adapter until loader v1 (issue 046).
        if (request.data[0] >> 8) & 0xFF == loader::VERSION.0 as usize {
            let _ = match loader::decode(&request, RECEIVED_CAP) {
                Ok((loader::Request::List, call)) => loader::reply_list(call, programs().as_slice()),
                Ok((loader::Request::Run { name, args }, call)) => loader::reply_run(call, load(name.as_str().as_bytes(), args.as_str().as_bytes(), None)),
                Err(reason) => wire::reject(reason),
            };
            continue;
        }
        let (packed, len) = mind::process::unpack_name(request.data);
        let code = match load(&packed[..len], &[], request.cap_received.then_some(RECEIVED_CAP)) { Ok(pid) => pid as usize, Err(error) => error.code() };
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        let _ = ipc::reply(&Message::new(code, 0));
    }
}
