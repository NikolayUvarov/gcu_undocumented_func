use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

pub fn exit() -> ! { exit_with(0) }

/// Ends the program with `code` (0: success; issue 166): its launcher reads it with `control::exit_status`.
pub fn exit_with(code: u32) -> ! {
    call(SYSCALL_EXIT, code as usize & 0xFF_FFFF, 0);
    loop { core::hint::spin_loop(); }
}

/// How a program ended (a watch's reason, `control::exit_status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit { Code(u32), Killed, Fault(u8) }

impl Exit {
    pub fn from_reason(reason: usize) -> Self {
        match reason & 0xFF { EXIT_KILLED => Exit::Killed, EXIT_FAULT => Exit::Fault((reason >> 8) as u8), _ => Exit::Code((reason >> 8) as u32 & 0xFF_FFFF) }
    }
    pub fn success(self) -> bool { self == Exit::Code(0) }
}

/// Writes bytes to the process log (and, line by line, to the system log if the process holds a client: `mind::log`;
/// and to the program that started it if it lent an endpoint for that: `mind::output`).
pub fn log(bytes: &[u8]) {
    for chunk in bytes.chunks(4096) { call(SYSCALL_LOG, chunk.as_ptr() as usize, chunk.len()); }
    crate::log::capture(bytes);
    crate::output::send(bytes);
}

/// Sink for `print!`/`println!`.
pub struct Log;
impl core::fmt::Write for Log {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { log(text.as_bytes()); Ok(()) }
}

/// Starts a program from disk (via the loader service, a launch session of idl/loader.wit). `grant` puts a copy of an
/// endpoint (handle, rights) in the child's INIT slot.
pub fn spawn(name: &str, grant: Option<(usize, u8)>) -> Result<u64> {
    use crate::idl::loader;
    use crate::sys::Error;
    let failed = |error: loader::Error| match error {
        loader::Error::NotFound => Error::NotFound, loader::Error::NoMemory => Error::NoMemory, loader::Error::Rights => Error::Rights,
        loader::Error::Limit => Error::Other(ERR_LIMIT), loader::Error::Busy | loader::Error::Sessions => Error::Other(ERR_BUSY), loader::Error::Invalid => Error::Invalid,
    };
    let endpoint = crate::ipc::Endpoint::LOADER;
    let session = loader::begin(endpoint, name, "")?.map_err(failed)?;
    if let Some((handle, rights)) = grant {
        // The loader keeps a copy of what it is lent: lend one with exactly the rights asked for.
        let lent = crate::ipc::mint(handle, rights, 0, 0).and_then(|copy| {
            let result = loader::grant(endpoint, session, SLOT_INIT as u8, copy);
            let _ = crate::ipc::drop_cap(copy);
            result?.map_err(failed)
        });
        if let Err(error) = lent { let _ = loader::abort(endpoint, session); return Err(error); }
    }
    loader::commit(endpoint, session)?.map_err(failed)
}

/// Starts a program from disk with arguments (via the loader service, idl/loader.wit); no endpoint can be passed to the
/// child.
pub fn spawn_with_args(name: &str, args: &str) -> Result<u64> {
    if name.is_empty() || name.len() > NAME_MAX || args.len() > ARGS_MAX || name.contains('\0') || args.contains('\0') { return Err(crate::sys::Error::Invalid); }
    crate::idl::loader::run(crate::ipc::Endpoint::LOADER, name, args)
}

/// Arguments the program was started with (the text after the program name), possibly empty.
pub fn args() -> &'static [u8] {
    let page = (crate::sys::info_address() + ARGS_OFFSET) as *const u8;
    unsafe { let len = u16::from_le_bytes([*page, *page.add(1)]) as usize; core::slice::from_raw_parts(page.add(2), len.min(ARGS_MAX)) }
}

/// Arguments as UTF-8 (empty if they are not valid UTF-8).
pub fn args_str() -> &'static str { core::str::from_utf8(args()).unwrap_or("") }

/// Where SPAWN takes the ELF from.
#[derive(Clone, Copy)]
pub enum Image {
    /// Memory capability holding `len` bytes of ELF (spawn privilege).
    Memory { cap: usize, len: usize },
    /// Boot image by index in `BOOT_FILES` (platform privilege, i.e. init).
    Boot(usize),
}

/// Grant for SPAWN: the child's fixed slot `child` gets a copy of the caller's capability `own` (a handle),
/// endpoints narrowed by `rights`.
pub const fn grant(child: usize, own: usize, rights: u8) -> Grant { Grant { own: own as u32, child: child as u8, rights, flags: 0 } }

/// Same, but the capability is moved into the child (the caller's handle becomes invalid once the child exists).
pub const fn grant_moved(child: usize, own: usize, rights: u8) -> Grant { Grant { own: own as u32, child: child as u8, rights, flags: GRANT_MOVE } }

/// Quotas delegated to a child at SPAWN, taken from the spawner's own: live child tasks and endpoints, and private
/// memory in MiB (0: the default HEAP_MAX_BYTES, SPAWN_MEMORY_ALL: the spawner's whole quota; issue 150).
#[derive(Clone, Copy, Debug, Default)]
pub struct Quota { pub tasks: u16, pub endpoints: u16, pub memory_mib: u16 }

/// Starts a task with exactly the granted capabilities and quotas; `flags` are SPAWN_SERVICE / SPAWN_SCREEN.
/// `name` may be `name\0arguments`.
pub fn spawn_raw(name: &[u8], image: Image, grants: &[Grant], flags: usize, quota: Quota) -> Result<u64> {
    let (source, len) = match image { Image::Memory { cap, len } => (cap, len), Image::Boot(index) => (SPAWN_BOOT | index, 0) };
    let packed = grants.len() | flags << 8 | (quota.tasks as usize) << 16 | (quota.endpoints as usize) << 32 | (quota.memory_mib as usize) << 48;
    check(syscall(SYSCALL_SPAWN, name.as_ptr() as usize, name.len(), [source, len, grants.as_ptr() as usize, packed]).result).map(|pid| pid as u64)
}

pub fn alive(pid: u64) -> bool { call(SYSCALL_TASK_ALIVE, pid as usize, 0) == 1 }

/// Sends the exit notice of `pid` (a task this process spawned) to `endpoint`, which this process can receive on.
pub fn watch(pid: u64, endpoint: crate::ipc::Endpoint) -> Result<()> { crate::sys::check(crate::sys::call(SYSCALL_TASK_WATCH, pid as usize, endpoint.0)).map(drop) }

/// What a program may ask its launcher for (`request!`); the launcher decides, the request grants nothing (MC-3.11).
pub const REQUEST_CONSOLE: u32 = 1; // no screen: output goes to the shell's console
pub const REQUEST_SYSINFO: u32 = 2; // a sysmon client in SLOT_SYSINFO
pub const REQUEST_FILE: u32 = 4; // the directory of the file named in the arguments (a client confined to it)
pub const REQUEST_LIFECYCLE: u32 = 8; // service lifecycle control in SLOT_LIFECYCLE
pub const REQUEST_LOG: u32 = 16; // the system log
pub const REQUEST_FILES: u32 = 32; // the user's files: everything the shell may change (ram:, data/), for fm
pub const REQUEST_NETWORK: u32 = 64; // a flow grant from the network policy broker in SLOT_NETWORK (issue 102)
pub const REQUEST_AUTHORITY: u32 = 128; // the sysmon client with the authority badge in SLOT_SYSINFO: who holds what (issue 081)
pub const REQUEST_WINDOW: u32 = 256; // a client of the window broker in SLOT_WINDOW: the program shows itself in a window (issue 157)
pub const REQUEST_WINDOW_MANAGER: u32 = 512; // the broker's manager client in SLOT_WINDOW: a window manager (issue 157)
pub const REQUEST_DISPLAY: u32 = 1024; // the compositor's client in SLOT_DISPLAY: what is on the screen (`record`, issue 093)
pub const REQUEST_GPIO: u32 = 2048; // the pin controller service's client with the control badge in SLOT_GPIO (issue 207)
pub const REQUEST_MAGIC: &[u8; 8] = b"MINDREQ1";

/// Contents of the `.mind_request` section: magic, flags, reserved.
pub const fn request_note(flags: u32, memory_mib: u32) -> [u8; 16] {
    let (f, m) = (flags.to_le_bytes(), memory_mib.to_le_bytes());
    [b'M', b'I', b'N', b'D', b'R', b'E', b'Q', b'1', f[0], f[1], f[2], f[3], m[0], m[1], m[2], m[3]]
}

/// Contents of the `.mind_about` section (`about!`): the text, UTF-8.
pub const fn about_note<const N: usize>(text: &str) -> [u8; N] {
    let bytes = text.as_bytes();
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N && i < bytes.len() { out[i] = bytes[i]; i += 1; }
    out
}

/// What the program does: the first line `name — what it does` (`list -l` shows it), then how to run it and its keys.
/// The first statement of `main`: with the argument `--help` the program prints the text and exits. The text is also
/// kept in the program file (`.mind_about`; the linker script keeps it), where the shell's `help <program>` reads it
/// without starting the program (`section`).
#[macro_export]
macro_rules! about {
    ($text:expr) => {{
        const MIND_ABOUT_TEXT: &str = $text;
        #[used]
        #[link_section = ".mind_about"]
        static MIND_ABOUT: [u8; MIND_ABOUT_TEXT.len()] = $crate::process::about_note::<{ MIND_ABOUT_TEXT.len() }>(MIND_ABOUT_TEXT);
        if $crate::process::args_str().trim() == "--help" { $crate::println!("{}", MIND_ABOUT_TEXT); $crate::process::exit(); }
    }};
}

/// Where the section `name` (e.g. `.mind_about`) is in an ELF64 file read through `read(offset, buffer) -> bytes
/// read`: its offset and size. At most 64 sections and 4 KiB of section names, as the loader reads them.
pub fn section(read: &mut dyn FnMut(usize, &mut [u8]) -> usize, name: &str) -> Option<(usize, usize)> {
    let mut header = [0u8; 64];
    if read(0, &mut header) < 64 || &header[..4] != b"\x7fELF" { return None; }
    let u16_at = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]) as usize;
    let u64_at = |b: &[u8], at: usize| u64::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3], b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
    let (offset, entry, count, names_index) = (u64_at(&header, 0x28), u16_at(&header, 0x3A), u16_at(&header, 0x3C), u16_at(&header, 0x3E));
    if entry != 64 || count == 0 || count > 64 || names_index >= count { return None; }
    let mut sections = [0u8; 64 * 64];
    if read(offset, &mut sections[..count * 64]) < count * 64 { return None; }
    let table = &sections[names_index * 64..names_index * 64 + 64];
    let size = u64_at(table, 0x20).min(4096);
    let mut names = [0u8; 4096];
    if read(u64_at(table, 0x18), &mut names[..size]) < size { return None; }
    (0..count).map(|index| &sections[index * 64..index * 64 + 64]).find_map(|section| {
        let at = u32::from_le_bytes([section[0], section[1], section[2], section[3]]) as usize;
        let wanted = names.get(at..size)?.strip_prefix(name.as_bytes())?.first() == Some(&0);
        wanted.then(|| (u64_at(section, 0x18), u64_at(section, 0x20)))
    })
}

/// Declares what the program asks its launcher for, e.g. `mind::request!(REQUEST_CONSOLE | REQUEST_SYSINFO);`, and
/// optionally a memory quota beyond the default 16 MiB: `mind::request!(REQUEST_CONSOLE, memory: 160);` (MiB). The
/// program's linker script keeps the `.mind_request` section (`KEEP`).
#[macro_export]
macro_rules! request {
    ($flags:expr) => { $crate::request!($flags, memory: 0); };
    ($flags:expr, memory: $mib:expr) => {
        #[used]
        #[link_section = ".mind_request"]
        static MIND_REQUEST: [u8; 16] = { use $crate::process::*; $crate::process::request_note($flags, $mib) };
    };
}
