#![no_std]
#![no_main]
// loader: reads application ELFs from disk via vfs_server and starts them (SPAWN) at the request of the shell or
// programs; the kernel stores no application images. The loader decides which client capabilities an application gets.
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::idl::codec::{List, Text};
use mind::idl::{loader, wire};
use mind::ipc::{self, Endpoint};
use mind::mem::Pages;
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::Error;
use mind::util::FixedBuf;

const RECEIVED_CAP: usize = 9;
const APP_ENDPOINTS: u16 = 4; // endpoints an application may create: loader's policy for each program
const APP_MEMORY_MAX: u32 = 1024; // MiB of private memory a program may ask for (the frame pool limits it further)
const MEMORY_FILE: &str = "data/memory.conf"; // the owner's lines `<program> <MiB>` (262-KRN-0160, 0163)
const WORKERS_QUOTA: u16 = 255; // the task quota of a program named there: the kernel allows one worker per online CPU
// Loader's own slots (granted by init): its endpoint, client endpoints passed on to applications, spawn privilege.
const OWN_RTC: usize = 2; const OWN_VFS: usize = 3; const OWN_AUDIO: usize = 4; const OWN_TTS: usize = 6;
const OWN_DIRS: usize = 7; // vfs_server's client badged BADGE_OWNERS: it alone opens system/own/ (000-KRN-0170)
const OWN_ROOT: &str = "system/own"; // the programs' own directories, one by name
const MAX_IMAGE: usize = 4 * 1024 * 1024;
// Helpers (1.8, 109-KRN-0096): programs that serve one caller and hold nothing but its endpoint.
const HELPER_MEMORY_MIB: u16 = 2;
const HELPERS_EACH: usize = 4; // live helpers a caller may have at once
const HELPERS: usize = 32;
// Programs the loader started, by caller, that the caller may end (1.11, 800-KRN-0138): the last STARTED that live.
const STARTED: usize = 64;

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
fn request_flags(read: &mut dyn FnMut(usize, &mut [u8]) -> usize) -> (u32, u32) {
    let mut header = [0u8; 64];
    if read(0, &mut header) < 64 || &header[..4] != b"\x7fELF" { return (0, 0); }
    let u16_at = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]) as usize;
    let u64_at = |b: &[u8], at: usize| u64::from_le_bytes(b[at..at + 8].try_into().unwrap()) as usize;
    let (shoff, shentsize, shnum, shstrndx) = (u64_at(&header, 0x28), u16_at(&header, 0x3A), u16_at(&header, 0x3C), u16_at(&header, 0x3E));
    if shentsize != 64 || shnum == 0 || shnum > 64 || shstrndx >= shnum { return (0, 0); }
    let mut sections = [0u8; 64 * 64];
    if read(shoff, &mut sections[..shnum * 64]) < shnum * 64 { return (0, 0); }
    let strtab = &sections[shstrndx * 64..shstrndx * 64 + 64];
    let (str_offset, str_size) = (u64_at(strtab, 0x18), u64_at(strtab, 0x20).min(4096));
    let mut names = [0u8; 4096];
    if read(str_offset, &mut names[..str_size]) < str_size { return (0, 0); }
    for index in 0..shnum {
        let section = &sections[index * 64..index * 64 + 64];
        let name = u32::from_le_bytes(section[..4].try_into().unwrap()) as usize;
        if name >= str_size || !names[name..str_size].starts_with(b".mind_request\0") { continue; }
        let mut note = [0u8; 16];
        if u64_at(section, 0x20) < 16 || read(u64_at(section, 0x18), &mut note) < 16 { return (0, 0); }
        let word = |at: usize| u32::from_le_bytes(note[at..at + 4].try_into().unwrap());
        return if &note[..8] == mind::process::REQUEST_MAGIC { (word(8), word(12)) } else { (0, 0) };
    }
    (0, 0)
}

// Whether debug mode is on, asked at every launch; the log says when it changes.
fn debug_mode() -> bool {
    use core::sync::atomic::{AtomicU8, Ordering};
    static SEEN: AtomicU8 = AtomicU8::new(0); // 0: not looked at yet, 1: off, 2: on
    let on = mind::debug::check();
    let now = if on { 2 } else { 1 };
    let before = SEEN.swap(now, Ordering::Relaxed);
    if before != now && (on || before != 0) {
        mind::println!("[LOADER] DEBUG MODE {} ({}): {}", if on { "ON" } else { "OFF" }, mind::debug::FILE,
                       if on { "WHAT PROGRAMS PRINT GOES TO THE SYSTEM LOG" } else { "PROGRAMS LOG AS BEFORE" });
    }
    on
}

// A file error as the loader reports it: a drive that does not answer stays ERR_IO, not an invalid program (211-KRN-0050).
fn file_error(error: fs::Error) -> Error { if error == fs::Error::Io { Error::Other(ERR_IO) } else { error.into() } }

// Applications installed under data/apps/ (750-KRN-0169, 750-UPD-0019): only the version `data/apps/<name>/current`
// names runs, and only as its signed MANIFEST lists it under a key the owner trusts for that name.
const APPS: &str = "data/apps/"; // mind::effector::APPS_DIR and its separator
const _: () = assert!(APPS.len() == mind::effector::APPS_DIR.len() + 1);
#[cfg(target_arch = "x86_64")] const APP_ARCH: apppkg::Arch = apppkg::Arch::X86_64;
#[cfg(target_arch = "aarch64")] const APP_ARCH: apppkg::Arch = apppkg::Arch::Aarch64;
const APP_KEYS: usize = 16; // trusted keys taken for one name

// An installed program: its application's name, its file in the package, and the version's directory.
struct Installed { name: FixedBuf<32>, file: FixedBuf<64>, dir: FixedBuf<64>, version: u64 }

fn refused(path: &str, why: &str) -> Error { mind::println!("[LOADER] {}: NOT RUN: {}", path, why); Error::Rights }

// What a path under data/apps/ stands for: `<name>/<file>` is the current version's, `<name>/<version>/<file>` must be it.
fn installed(path: &str) -> Result<Option<Installed>, Error> {
    let Some(rest) = path.strip_prefix(APPS) else { return Ok(None) };
    let (name, rest) = rest.split_once('/').ok_or_else(|| refused(path, "NOT data/apps/<name>/<file>"))?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (version, file) = match rest.split_once('/') { Some((v, f)) if digits(v) => (Some(v), f), _ => (None, rest) };
    let name_ok = !name.is_empty() && name.len() <= apppkg::NAME_MAX && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !name_ok || file.is_empty() || file.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(refused(path, "NOT data/apps/<name>/<file>"));
    }
    let mut current_path = FixedBuf::<64>::new();
    let _ = write!(current_path, "{}{}/current", APPS, name);
    let mut text = [0u8; 24];
    let read = File::open(current_path.as_str()).ok().and_then(|f| f.read_at(0, &mut text).ok()).unwrap_or(0);
    let current = core::str::from_utf8(&text[..read]).unwrap_or("").trim();
    let number = current.parse::<u64>().ok().filter(|_| digits(current)).ok_or_else(|| refused(path, "NO current VERSION"))?;
    if version.is_some_and(|v| v != current) { return Err(refused(path, "NOT THE CURRENT VERSION")); }
    if APPS.len() + name.len() + current.len() + file.len() + 2 > 63 { return Err(refused(path, "PATH TOO LONG")); }
    let mut app = Installed { name: FixedBuf::new(), file: FixedBuf::new(), dir: FixedBuf::new(), version: number };
    let _ = (write!(app.name, "{}", name), write!(app.file, "{}", file), write!(app.dir, "{}{}/{}/", APPS, name, current));
    Ok(Some(app))
}

// A whole file of at most `max` bytes into pages, after `before` (the signed prefix); None if it cannot be read whole.
fn read_whole(path: &str, max: usize, before: &[u8]) -> Option<(Pages, usize)> {
    let file = File::open(path).ok()?;
    let size = file.size();
    if size == 0 || size > max { return None; }
    let mut pages = Pages::new(before.len() + size)?;
    pages.as_mut_slice()[..before.len()].copy_from_slice(before);
    (file.read_at(0, &mut pages.as_mut_slice()[before.len()..before.len() + size]).ok()? == size).then_some((pages, before.len() + size))
}

// The check before an installed program runs: its MANIFEST signed by a key the owner's policy trusts for its name, of
// this architecture and the current version, the program as its file line lists it, and no more asked than the line says.
fn check_installed(path: &str, app: &Installed, image: &[u8], flags: u32, memory: u32) -> Result<(), Error> {
    let at = |name: &str| { let mut p = FixedBuf::<64>::new(); let _ = write!(p, "{}{}", app.dir.as_str(), name); p };
    let mut signature = [0u8; 64];
    let signed_ok = File::open(at("MANIFEST.SIG").as_str()).ok().filter(|f| f.size() == 64).and_then(|f| f.read_at(0, &mut signature).ok()) == Some(64);
    if !signed_ok { return Err(refused(path, "NO MANIFEST.SIG OF 64 BYTES")); }
    let (signed, length) = read_whole(at("MANIFEST").as_str(), apppkg::MANIFEST_MAX, apppkg::PREFIX).ok_or_else(|| refused(path, "NO MANIFEST"))?;
    // The owner's policy, read whole or not at all: a policy that does not parse trusts no key.
    let policy = read_whole(mind::effector::POLICY_FILE, mind::effector::policy::FILE_MAX, &[])
        .and_then(|(pages, n)| mind::effector::policy::Policy::parse(&pages.as_slice()[..n]).ok())
        .ok_or_else(|| refused(path, "THE OWNER'S POLICY CANNOT BE READ: NO KEY IS TRUSTED"))?;
    let mut trusted = [apppkg::Trusted { key: [0; 32], name: app.name.as_str() }; APP_KEYS];
    let mut count = 0;
    for key in policy.keys_for(app.name.as_str()).take(APP_KEYS) { trusted[count].key = *key; count += 1; }
    let manifest = apppkg::verify_prefixed(&signed.as_slice()[..length], &signature, &trusted[..count], APP_ARCH, u64::MAX, None, false).map_err(|e| {
        let mut why = FixedBuf::<48>::new();
        let _ = write!(why, "MANIFEST REFUSED ({:?})", e);
        refused(path, why.as_str())
    })?;
    if manifest.name != app.name.as_str() || manifest.version != app.version { return Err(refused(path, "MANIFEST OF ANOTHER NAME OR VERSION")); }
    let line = manifest.files().find(|f| f.path == app.file.as_str()).ok_or_else(|| refused(path, "NOT IN ITS MANIFEST"))?;
    if !line.matches(image) { return Err(refused(path, "NOT AS ITS MANIFEST LISTS IT")); }
    if flags & !line.requests != 0 || memory as u64 > line.memory { return Err(refused(path, "ASKS FOR MORE THAN ITS MANIFEST LINE")); }
    Ok(())
}

// The program file of `name` and its task name; boot services and the kernel are not applications. A bare name is the
// booted slot's when that slot holds applications (351-KRN-0151); a path names its file as it is, an installed
// application's (data/apps/) the current version's.
fn open(name: &[u8]) -> Result<(File, FixedBuf<NAME_MAX>, Option<Installed>), Error> {
    let mut path = FixedBuf::<64>::new();
    if !name.contains(&b'/') { let _ = write!(path, "{}", apps_dir()); }
    path_for(name, &mut path)?;
    let app = installed(path.as_str())?;
    if let Some(app) = &app { path = FixedBuf::new(); let _ = write!(path, "{}{}", app.dir.as_str(), app.file.as_str()); }
    let task = task_name(path.as_bytes());
    // Services are started by init from bootloader images; the kernel itself is not runnable as a program.
    if task.as_bytes() == b"kernel" || BOOT_SERVICES.iter().any(|s| s.as_bytes() == task.as_bytes()) { return Err(Error::NotFound); }
    Ok((File::open(core::str::from_utf8(path.as_bytes()).unwrap()).map_err(file_error)?, task, app))
}

// Starts `name` with the standard client endpoints and the capabilities `extra` (handles in this task, moved into the
// child's slots). A program that asked to be a console program gets no screen, nor one its launcher starts in a window
// (a broker client in SLOT_WINDOW, issue 088): it draws into the window's surface; a window manager keeps its screen.
// With `front`, a program with a screen starts in front for that task, which must have the focus (issue 160). With `fed`,
// it gets a text input whose writer's handle comes back beside its PID (1.9, 000-KRN-0094), else 0.
fn load(name: &[u8], args: &[u8], extra: &[(u8, usize)], front: Option<u64>, fed: bool) -> Result<(u64, usize), Error> {
    let (file, task, app) = open(name)?;
    // A program run by its name (the booted slot's or the root's) or installed in data/apps/ owns system/own/<name>/;
    // one run by any other path has no identity to own a directory by (000-KRN-0170).
    let own = if !name.contains(&b'/') || app.is_some() { own_dir(task.as_str()) } else { None };
    let (image, size) = read_image(&file)?;
    let (flags, memory) = { let bytes = &image.as_slice()[..size]; request_flags(&mut |at, out: &mut [u8]| { let n = out.len().min(bytes.len().saturating_sub(at)); out[..n].copy_from_slice(&bytes[at..at + n]); n }) };
    if let Some(app) = &app { check_installed(core::str::from_utf8(name).unwrap_or("?"), app, &image.as_slice()[..size], flags, memory)?; }
    let cap = image.share()?;
    let client = CAP_WRITE | CAP_GRANT;
    let mut grants = [Grant::default(); SPAWN_GRANTS_MAX];
    let standard = [grant(SLOT_RTC, OWN_RTC, client), grant(SLOT_VFS, OWN_VFS, client), grant(SLOT_AUDIO, OWN_AUDIO, client),
                    grant(SLOT_LOADER, SLOT_SERVICE, client), grant(SLOT_TTS, OWN_TTS, client)];
    grants[..5].copy_from_slice(&standard);
    let mut count = 5;
    for &(child, handle) in extra { grants[count] = grant_moved(child as usize, handle, u8::MAX); count += 1; }
    // Debug mode (log:debug.txt, 211-KRN-0053): a program its launcher gave no log client gets a copy of the loader's,
    // so what it prints reaches the system log and this boot's log file.
    if debug_mode() && !extra.iter().any(|&(child, _)| child as usize == SLOT_LOG) && count < grants.len() { grants[count] = grant(SLOT_LOG, SLOT_LOG, client); count += 1; }
    if let Some((handle, _)) = own.filter(|_| count < grants.len()) { grants[count] = grant_moved(SLOT_OWN, handle, u8::MAX); count += 1; }
    // SPAWN takes `name\0arguments`.
    let mut text = [0u8; NAME_MAX + 1 + ARGS_MAX];
    let args = &args[..args.len().min(ARGS_MAX)];
    text[..task.as_bytes().len()].copy_from_slice(task.as_bytes());
    let mut len = task.as_bytes().len();
    if !args.is_empty() { text[len + 1..len + 1 + args.len()].copy_from_slice(args); len += 1 + args.len(); }
    let windowed = extra.iter().any(|&(slot, _)| slot as usize == SLOT_WINDOW) && flags & mind::process::REQUEST_WINDOW_MANAGER == 0;
    let console = mind::process::console_run(flags, core::str::from_utf8(args).unwrap_or("")); // `clock --line` too (issue u016)
    let screen = if console || windowed { 0 } else { SPAWN_SCREEN };
    // Each application may create a few endpoints (taken from loader's quota) and cannot spawn by itself.
    // The memory a program asks for, up to APP_MEMORY_MAX MiB; its use is charged to loader's quota too (issue 150).
    // A program the owner names in data/memory.conf gets its line's MiB instead, and may start workers (262-KRN-0163).
    let named = owners_memory(task.as_bytes());
    let memory_mib = named.unwrap_or(memory.min(APP_MEMORY_MAX) as u16);
    let (tasks, workers) = if named.is_some() { (WORKERS_QUOTA, SPAWN_WORKERS) } else { (0, 0) };
    let image = Image::Memory { cap, len: size };
    let spawn = |memory_mib: u16| {
        let quota = Quota { tasks, endpoints: APP_ENDPOINTS, memory_mib };
        match (front.filter(|_| screen != 0), fed) {
            (front, true) => mind::process::spawn_fed(&text[..len], image, &grants[..count], (if front.is_some() { 0 } else { screen }) | workers, quota, front.unwrap_or(0)),
            (Some(owner), false) => mind::process::spawn_in_front_with(&text[..len], image, &grants[..count], workers, quota, owner).map(|pid| (pid, 0)),
            (None, false) => mind::process::spawn_raw(&text[..len], image, &grants[..count], screen | workers, quota).map(|pid| (pid, 0)),
        }
    };
    let mut result = spawn(memory_mib);
    // More than loader's quota (the frame pool) asked: the kernel refuses before it moves a grant, and the program gets
    // all of loader's quota instead, which the frame pool limits further anyway (000-KRN-0134).
    if memory_mib as usize > HEAP_MAX_BYTES >> 20 && matches!(result, Err(Error::Other(ERR_LIMIT))) {
        if let Ok(started) = spawn(SPAWN_MEMORY_ALL as u16) {
            mind::println!("[LOADER] {}: {} MiB ASKED, MORE THAN LOADER'S QUOTA: GIVEN ALL OF IT", core::str::from_utf8(task.as_bytes()).unwrap_or("?"), memory_mib);
            result = Ok(started);
        }
    }
    let _ = ipc::drop_cap(cap); // the kernel has already copied the image; the buffer is freed when the function returns
    // The scope is the program's alone for as long as it runs, whenever it first opens it; unused, it ends in a minute.
    if let Some((handle, badge)) = own {
        match result {
            Ok((pid, _)) => { if !matches!(mind::idl::vfs::adopt(Endpoint(OWN_DIRS), badge, pid), Ok(Ok(()))) { mind::println!("[LOADER] {}: ITS OWN DIRECTORY NOT KEPT FOR PID {}", task.as_str(), pid); } }
            Err(_) => { let _ = ipc::drop_cap(handle); }
        }
    }
    result
}

// A writable scope of system/own/<task>/, made if missing, and its badge; None (logged) when vfs_server refuses.
fn own_dir(task: &str) -> Option<(usize, u16)> {
    use mind::idl::vfs;
    let files = Endpoint(OWN_DIRS);
    let made = (|| -> core::result::Result<(usize, u16), &'static str> {
        let root = vfs::root(files, "").ok().and_then(|r| r.ok()).ok_or("NO BOOT VOLUME")?;
        let parent = vfs::open_dir(files, root, OWN_ROOT, true);
        let _ = vfs::close(files, root);
        let parent = parent.ok().and_then(|r| r.ok()).ok_or("system/own NOT OPENED")?;
        let dir = vfs::open_dir(files, parent, task, true);
        let _ = vfs::close(files, parent);
        let dir = dir.ok().and_then(|r| r.ok()).ok_or("ITS DIRECTORY NOT MADE")?;
        let scoped = vfs::scope(files, dir, true, RECEIVED_CAP);
        let _ = vfs::close(files, dir);
        scoped.ok().and_then(|r| r.ok()).ok_or("NO SCOPE")?;
        let handle = ipc::mint(RECEIVED_CAP, u8::MAX, 0, 0);
        let _ = ipc::drop_cap(RECEIVED_CAP);
        let handle = handle.map_err(|_| "NO SLOT")?;
        Ok((handle, mind::dev::cap_info(handle).1 as u16))
    })();
    made.inspect_err(|why| mind::println!("[LOADER] {}: NO OWN DIRECTORY ({})", task, why)).ok()
}

// The MiB the owner's data/memory.conf gives program `task` (a line `<program> <MiB>`; `#` starts a comment), read at
// each run; None without the file or a line for it (262-KRN-0160, 0163).
fn owners_memory(task: &[u8]) -> Option<u16> {
    let file = File::open(MEMORY_FILE).ok()?;
    let mut text = [0u8; 2048];
    let n = file.read_at(0, &mut text).ok()?;
    core::str::from_utf8(&text[..n]).ok()?.lines().find_map(|line| {
        let mut words = line.split('#').next()?.split_whitespace();
        (words.next()?.as_bytes().eq_ignore_ascii_case(task)).then(|| words.next()?.parse::<u32>().ok())?
    }).map(|mib| mib.min(QUOTA_MAX as u32 - 1) as u16)
}

// A program file read whole into memory, if it is an ELF of a size the loader takes.
fn read_image(file: &File) -> Result<(Pages, usize), Error> {
    let size = file.size();
    if size < 64 || size > MAX_IMAGE { return Err(Error::Invalid); }
    let mut image = Pages::new(size).ok_or(Error::NoMemory)?;
    if file.read_at(0, &mut image.as_mut_slice()[..size]).map_err(file_error)? != size || &image.as_slice()[..4] != b"\x7fELF" { return Err(Error::Invalid); }
    Ok((image, size))
}

// Starts helper `name` (1.8, 109-KRN-0096) with the endpoint in RECEIVED_CAP moved into its SLOT_INIT and nothing else:
// no standard clients, no screen, no log even in debug mode, no tasks or endpoints of its own. `--session` tells it so.
// A helper is a boot service: on a disk with boot slots its file is the booted slot's, the copy the bootloader verified
// against the manifest (109-KRN-0133).
fn spawn_helper(name: &str) -> Result<u64, Error> {
    let mut path = FixedBuf::<64>::new();
    let _ = write!(path, "{}", slot_dir());
    path_for(name.as_bytes(), &mut path)?;
    let file = File::open(core::str::from_utf8(path.as_bytes()).unwrap()).map_err(file_error)?;
    let (image, size) = read_image(&file)?;
    let cap = image.share()?;
    let mut text = [0u8; NAME_MAX + 1 + 16];
    let len = name.len() + 1 + b"--session".len();
    text[..name.len()].copy_from_slice(name.as_bytes());
    text[name.len() + 1..len].copy_from_slice(b"--session");
    let quota = Quota { tasks: 0, endpoints: 0, memory_mib: HELPER_MEMORY_MIB };
    let result = mind::process::spawn_raw(&text[..len], Image::Memory { cap, len: size }, &[grant_moved(SLOT_INIT, RECEIVED_CAP, u8::MAX)], 0, quota);
    let _ = ipc::drop_cap(cap);
    result
}

// A launch in progress (idl/loader.wit): who started it, what to run, the capabilities lent for the program's slots,
// and where its exit notice goes (`watch`, 1.11).
struct Session { id: u32, owner: u64, name: [u8; 64], name_len: usize, args: [u8; ARGS_MAX], args_len: usize, grants: [(u8, usize); SESSION_GRANTS], count: usize, notices: Option<usize> }

// Every launch slot and SLOT_FIRMWARE at once (000-KRN-0101): with the standard five and the debug log, 24 of SPAWN's 32.
const SESSION_GRANTS: usize = LAUNCH_SLOTS.len() + 1;
const _: () = assert!(5 + SESSION_GRANTS + 1 <= SPAWN_GRANTS_MAX);

impl Session {
    fn drop_grants(&mut self) {
        for &(_, handle) in &self.grants[..self.count] { let _ = ipc::drop_cap(handle); }
        self.count = 0;
        if let Some(handle) = self.notices.take() { let _ = ipc::drop_cap(handle); }
    }
}

const SESSIONS: usize = 4;

struct Launcher { sessions: [Option<Session>; SESSIONS], next: u32, helpers: [Option<(u64, u64)>; HELPERS], started: [Option<(u64, u64)>; STARTED], oldest: usize }

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
        let mut session = Session { id: self.next, owner, name: [0; 64], name_len: name.len(), args: [0; ARGS_MAX], args_len: args.len(), grants: [(0, 0); SESSION_GRANTS], count: 0, notices: None };
        session.name[..name.len()].copy_from_slice(name.as_bytes()); session.args[..args.len()].copy_from_slice(args.as_bytes());
        self.sessions[index] = Some(session);
        Ok(self.next)
    }

    fn grant(&mut self, owner: u64, id: u32, slot: u8) -> Result<(), loader::Error> {
        // The fixed slots a launcher may fill: an endpoint for the program's INIT slot (a ping/pong pair), its file
        // client, its window broker client, where its output goes (issue 162), sysinfo, lifecycle control, the system log,
        // a flow grant, the compositor's client (what is on the screen, issue 165) or, by `grant-memory`, the read-only
        // surface of one window (issue u014), the pin controller's client (issue 207), the video gateway's (issue 158) or the
        // TLS service's (351-KRN-0034), the shell's command endpoint or the clipboard's (211-KRN-0058).
        // The standard grants (2..6) cannot be replaced.
        if !LAUNCH_SLOTS.contains(&(slot as usize)) { return Err(loader::Error::Invalid); }
        self.keep(owner, id, slot)
    }

    // The capability arrived in the receive slot; keep a copy in a slot of our own until the program starts.
    fn keep(&mut self, owner: u64, id: u32, slot: u8) -> Result<(), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        let handle = ipc::mint(RECEIVED_CAP, u8::MAX, 0, 0).map_err(|_| loader::Error::NoMemory)?;
        let session = self.sessions[index].as_mut().unwrap();
        if let Some(existing) = session.grants[..session.count].iter_mut().find(|g| g.0 == slot) { let _ = ipc::drop_cap(existing.1); existing.1 = handle; return Ok(()); }
        if session.count == session.grants.len() { let _ = ipc::drop_cap(handle); return Err(loader::Error::Limit); }
        session.grants[session.count] = (slot, handle); session.count += 1;
        Ok(())
    }

    // A read-only memory capability (1.3): only in SLOT_DISPLAY, where a window manager lends the surface of the window
    // a program records (issue u014). A writable one would let the program change another's window.
    fn grant_memory(&mut self, owner: u64, id: u32, slot: u8) -> Result<(), loader::Error> {
        if slot as usize != SLOT_DISPLAY { return Err(loader::Error::Invalid); }
        if mind::dev::cap_info(RECEIVED_CAP).1 & CAP_WRITE as usize != 0 { return Err(loader::Error::Rights); }
        self.grant(owner, id, slot)
    }

    // The firmware variable privilege (1.6): only in SLOT_FIRMWARE; the IDL has checked its kind (351-KRN-0027).
    fn grant_firmware(&mut self, owner: u64, id: u32) -> Result<(), loader::Error> { self.keep(owner, id, SLOT_FIRMWARE as u8) }

    // The endpoint the program's exit notice goes to (1.11): it must carry the right to receive, as TASK_WATCH asks.
    fn watch(&mut self, owner: u64, id: u32) -> Result<(), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        if mind::dev::cap_info(RECEIVED_CAP).2 & CAP_READ as usize == 0 { return Err(loader::Error::Rights); }
        let handle = ipc::mint(RECEIVED_CAP, u8::MAX, 0, 0).map_err(|_| loader::Error::NoMemory)?;
        if let Some(old) = self.sessions[index].as_mut().unwrap().notices.replace(handle) { let _ = ipc::drop_cap(old); }
        Ok(())
    }

    // A program started for `owner`, which it may end (`stop`): a free or ended entry, else the oldest one.
    fn record(&mut self, owner: u64, pid: u64) {
        let index = self.started.iter().position(|e| e.is_none_or(|(_, pid)| !mind::process::alive(pid))).unwrap_or_else(|| { let at = self.oldest; self.oldest = (at + 1) % STARTED; at });
        self.started[index] = Some((owner, pid));
    }

    // `run` (no session): the program gets only the standard grants.
    fn run(&mut self, owner: u64, name: &str, args: &str) -> Result<u64, Error> {
        let (pid, _) = load(name.as_bytes(), args.as_bytes(), &[], None, false)?;
        self.record(owner, pid);
        Ok(pid)
    }

    // Ends a program the loader started for `owner` (1.11): the kernel lets a task end its descendants.
    fn stop(&mut self, owner: u64, pid: u64) -> Result<(), loader::Error> {
        let index = self.started.iter().position(|&e| e == Some((owner, pid))).ok_or(loader::Error::NotFound)?;
        self.started[index] = None;
        mind::control::kill(pid).map_err(|error| if error == Error::NotFound { loader::Error::NotFound } else { loader::Error::Invalid })
    }

    // `front`: in front for the owner (commit-in-front); refused while the owner is not in front, the session kept.
    // `fed`: with a text input (commit-with-input); the PID and the writer's handle.
    fn commit(&mut self, owner: u64, id: u32, front: bool, fed: bool) -> Result<(u64, usize), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        let mut session = self.sessions[index].take().unwrap();
        let result = load(&session.name[..session.name_len], &session.args[..session.args_len], &session.grants[..session.count], front.then_some(owner), fed);
        if let Err(Error::Other(ERR_FOCUS)) = result { self.sessions[index] = Some(session); return Err(loader::Error::Rights); }
        match result {
            Ok((pid, _)) => {
                self.record(owner, pid);
                // Set at once: the kernel keeps a notice for a child that ended before it is watched (800-KRN-0138).
                if let Some(handle) = session.notices.take() { let _ = mind::process::watch(pid, Endpoint(handle)); let _ = ipc::drop_cap(handle); }
            }
            Err(_) => session.drop_grants(),
        }
        result.map_err(|error| match error {
            Error::NotFound => loader::Error::NotFound, Error::NoMemory => loader::Error::NoMemory, Error::Rights => loader::Error::Rights,
            Error::Other(ERR_LIMIT) => loader::Error::Limit, Error::Other(ERR_BUSY) => loader::Error::Busy, Error::Other(ERR_IO) => loader::Error::Unreadable,
            _ => loader::Error::Invalid,
        })
    }

    // A helper for `owner` (1.8): the endpoint it serves must carry the right to receive; at most HELPERS_EACH live at once.
    fn helper(&mut self, owner: u64, which: loader::HelperKind) -> Result<u64, loader::Error> {
        let name = match which { loader::HelperKind::Parse => "parse" };
        if mind::dev::cap_info(RECEIVED_CAP).2 & CAP_READ as usize == 0 { return Err(loader::Error::Rights); } // an endpoint's rights
        for entry in self.helpers.iter_mut() { if entry.is_some_and(|(_, pid)| !mind::process::alive(pid)) { *entry = None; } }
        if self.helpers.iter().flatten().filter(|&&(o, _)| o == owner).count() >= HELPERS_EACH { return Err(loader::Error::Limit); }
        let free = self.helpers.iter().position(Option::is_none).ok_or(loader::Error::Limit)?;
        let pid = spawn_helper(name).map_err(|error| match error {
            Error::NotFound => loader::Error::NotFound, Error::NoMemory => loader::Error::NoMemory,
            Error::Other(ERR_LIMIT) => loader::Error::Limit, Error::Other(ERR_IO) => loader::Error::Unreadable, _ => loader::Error::Invalid,
        })?;
        self.helpers[free] = Some((owner, pid));
        Ok(pid)
    }

    fn abort(&mut self, owner: u64, id: u32) -> Result<(), loader::Error> {
        let index = self.find(id, owner).ok_or(loader::Error::NotFound)?;
        self.sessions[index].take().unwrap().drop_grants();
        Ok(())
    }
}

// The raw request flags of program `name` (REQUEST_*).
fn requests(name: &str) -> Result<u32, loader::Error> {
    let (file, _, _) = open(name.as_bytes()).map_err(|error| match error { Error::NotFound => loader::Error::NotFound, Error::Other(ERR_IO) => loader::Error::Unreadable, _ => loader::Error::Invalid })?;
    Ok(request_flags(&mut |at, out: &mut [u8]| file.read_at(at, out).unwrap_or(0)).0)
}

fn inspect(name: &str) -> Result<loader::Needs, loader::Error> {
    let flags = requests(name)?;
    use mind::process::*;
    Ok(loader::Needs { console: flags & REQUEST_CONSOLE != 0, sysinfo: flags & REQUEST_SYSINFO != 0, file: flags & REQUEST_FILE != 0, lifecycle: flags & REQUEST_LIFECYCLE != 0, log: flags & REQUEST_LOG != 0, files: flags & REQUEST_FILES != 0 })
}

// The *.elf programs where applications are (the disk root, or the booted slot), except the kernel; services are marked.
fn programs() -> List<loader::Program, 128> {
    let mut list = List::default();
    let _ = fs::list(apps_dir().trim_end_matches('/'), |entry| {
        if entry.is_dir || entry.name.len() < 5 || !entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".elf") { return; }
        let name = task_name(entry.name);
        if name.as_bytes() == b"kernel" { return; }
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes() == name.as_bytes());
        if let Some(name) = Text::new(core::str::from_utf8(name.as_bytes()).unwrap_or("?")) { list.push(loader::Program { name, size: entry.size as u64, service }); }
    });
    list
}

mind::entry!(main);
// The slot this boot runs from (BootInfo.boot_slot), for the helpers' files.
static BOOT_SLOT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(BOOT_SLOT_ROOT);
// Whether the booted slot holds applications, so that they come from it and not the root (351-KRN-0151, 351-UPD-0023).
static SLOT_APPS: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// The booted slot's directory, with its slash; "" on a disk without slots.
fn slot_dir() -> &'static str {
    match BOOT_SLOT.load(core::sync::atomic::Ordering::Relaxed) { BOOT_SLOT_A => "MIND/A/", BOOT_SLOT_B => "MIND/B/", _ => "" }
}
// Where applications are: the booted slot when it holds any, else the disk root.
fn apps_dir() -> &'static str { if SLOT_APPS.load(core::sync::atomic::Ordering::Relaxed) { slot_dir() } else { "" } }

// A slot holds applications when it holds any *.elf that is no boot file: the updater's own test before `apply`. A slot
// staged before 351-UPD-0023 holds none, and the root's run.
fn find_slot_apps() {
    let dir = slot_dir();
    if dir.is_empty() { return; }
    let mut found = false;
    let _ = fs::list(dir.trim_end_matches('/'), |entry| {
        let elf = !entry.is_dir && entry.name.len() > 4 && entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".elf");
        let boot = entry.name.eq_ignore_ascii_case(b"kernel.elf") || BOOT_FILES.iter().any(|f| f.as_bytes().eq_ignore_ascii_case(entry.name));
        found |= elf && !boot;
    });
    SLOT_APPS.store(found, core::sync::atomic::Ordering::Relaxed);
    mind::println!("[LOADER] APPLICATIONS FROM {}", if found { dir } else { "THE ROOT: THE BOOTED SLOT HOLDS NONE" });
}

fn main(info: &'static BootInfo) {
    BOOT_SLOT.store(info.boot_slot.slot, core::sync::atomic::Ordering::Relaxed);
    let _ = fs::prepare();
    find_slot_apps();
    mind::println!("[LOADER] READY");
    let mut launcher = Launcher { sessions: [const { None }; SESSIONS], next: 0, helpers: [None; HELPERS], started: [None; STARTED], oldest: 0 };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        let owner = request.sender;
        // A helpers-only client (the updater's, 109-KRN-0117) may list, inspect and start helpers, and start no program.
        let refuse = request.badge == mind::process::BADGE_HELPERS_ONLY;
        let denied = loader::Error::Rights;
        // Requests in idl/loader.wit; a message that is not a call is dropped with its capability.
        let _ = match loader::decode(&request, RECEIVED_CAP) {
            Ok((loader::Request::Run { .. }, call)) if refuse => loader::reply_run(call, Err(Error::Rights)),
            Ok((loader::Request::Begin { .. }, call)) if refuse => loader::reply_begin(call, Err(denied)),
            Ok((loader::Request::Grant { .. }, call)) if refuse => { let _ = ipc::drop_cap(RECEIVED_CAP); loader::reply_grant(call, Err(denied)) }
            Ok((loader::Request::GrantMemory { .. }, call)) if refuse => { let _ = ipc::drop_cap(RECEIVED_CAP); loader::reply_grant_memory(call, Err(denied)) }
            Ok((loader::Request::GrantFirmware { .. }, call)) if refuse => { let _ = ipc::drop_cap(RECEIVED_CAP); loader::reply_grant_firmware(call, Err(denied)) }
            Ok((loader::Request::Commit { .. }, call)) if refuse => loader::reply_commit(call, Err(denied)),
            Ok((loader::Request::CommitInFront { .. }, call)) if refuse => loader::reply_commit_in_front(call, Err(denied)),
            Ok((loader::Request::CommitWithInput { .. }, call)) if refuse => loader::reply_commit_with_input(call, Err(denied)),
            Ok((loader::Request::Abort { .. }, call)) if refuse => loader::reply_abort(call, Err(denied)),
            Ok((loader::Request::Watch { .. }, call)) if refuse => { let _ = ipc::drop_cap(RECEIVED_CAP); loader::reply_watch(call, Err(denied)) }
            Ok((loader::Request::Stop { .. }, call)) if refuse => loader::reply_stop(call, Err(denied)),
            Ok((loader::Request::List, call)) => loader::reply_list(call, programs().as_slice()),
            Ok((loader::Request::Run { name, args }, call)) => loader::reply_run(call, launcher.run(owner, name.as_str(), args.as_str())),
            Ok((loader::Request::Begin { name, args }, call)) => loader::reply_begin(call, launcher.begin(owner, name.as_str(), args.as_str())),
            Ok((loader::Request::Grant { session, slot, .. }, call)) => loader::reply_grant(call, launcher.grant(owner, session, slot)),
            Ok((loader::Request::GrantMemory { session, slot, .. }, call)) => loader::reply_grant_memory(call, launcher.grant_memory(owner, session, slot)),
            Ok((loader::Request::GrantFirmware { session, .. }, call)) => loader::reply_grant_firmware(call, launcher.grant_firmware(owner, session)),
            Ok((loader::Request::Commit { session }, call)) => loader::reply_commit(call, launcher.commit(owner, session, false, false).map(|(pid, _)| pid)),
            Ok((loader::Request::CommitInFront { session }, call)) => loader::reply_commit_in_front(call, launcher.commit(owner, session, true, false).map(|(pid, _)| pid)),
            Ok((loader::Request::CommitWithInput { session, front }, call)) => {
                let started = launcher.commit(owner, session, front, true);
                let sent = loader::reply_commit_with_input(call, started.map(|(_, writer)| writer));
                // Moved with the reply; one the caller did not take is dropped, so the program's input ends.
                if let Ok((_, writer)) = started { let _ = ipc::drop_cap(writer); }
                sent
            }
            Ok((loader::Request::Abort { session }, call)) => loader::reply_abort(call, launcher.abort(owner, session)),
            Ok((loader::Request::Watch { session, .. }, call)) => loader::reply_watch(call, launcher.watch(owner, session)),
            Ok((loader::Request::Stop { pid }, call)) => loader::reply_stop(call, launcher.stop(owner, pid)),
            Ok((loader::Request::Inspect { name }, call)) => loader::reply_inspect(call, inspect(name.as_str()).as_ref().map_err(|e| *e)),
            Ok((loader::Request::InspectRequests { name }, call)) => loader::reply_inspect_requests(call, requests(name.as_str())),
            Ok((loader::Request::Helper { which, .. }, call)) => {
                let started = launcher.helper(owner, which);
                if started.is_err() { let _ = ipc::drop_cap(RECEIVED_CAP); } // not moved into a helper
                loader::reply_helper(call, started)
            }
            Err(reason) => if request.is_call { wire::reject(reason) } else { Ok(()) },
        };
    }
}
