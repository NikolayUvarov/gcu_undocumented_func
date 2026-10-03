#![no_std]
#![no_main]
// loader: reads application ELFs from disk via vfs_server and starts them (SPAWN) at the request of the shell or
// programs; the kernel stores no application images. The loader decides which client capabilities an application gets.
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{Mapping, Pages};
use mind::process::{grant, Image};
use mind::sys::Error;
use mind::util::FixedBuf;

const RECEIVED_CAP: usize = 9;
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

fn load(name: &[u8], init: Option<usize>) -> Result<u64, Error> {
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
    let result = mind::process::spawn_raw(task.as_bytes(), Image::Memory { cap, len: size }, grants, SPAWN_SCREEN);
    let _ = ipc::drop_cap(cap); // the kernel has already copied the image; the buffer is freed when the function returns
    result
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
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if !request.is_call {
            if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
            continue;
        }
        let code = if request.data == [0, LOADER_LIST] {
            // Program list into the caller's memory page.
            match Mapping::new(RECEIVED_CAP) { Ok(mut page) => listing(page.as_mut_slice()), Err(error) => error.code() }
        } else {
            // Spawn: name in two message words, optional endpoint for the child's INIT slot.
            let (packed, len) = mind::process::unpack_name(request.data);
            match load(&packed[..len], request.cap_received.then_some(RECEIVED_CAP)) { Ok(pid) => pid as usize, Err(error) => error.code() }
        };
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        let _ = ipc::reply(&Message::new(code, 0));
    }
}
