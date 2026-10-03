use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

pub fn exit() -> ! {
    call(SYSCALL_EXIT, 0, 0);
    loop { core::hint::spin_loop(); }
}

/// Writes bytes to the process log.
pub fn log(bytes: &[u8]) { for chunk in bytes.chunks(4096) { call(SYSCALL_LOG, chunk.as_ptr() as usize, chunk.len()); } }

/// Sink for `print!`/`println!`.
pub struct Log;
impl core::fmt::Write for Log {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { log(text.as_bytes()); Ok(()) }
}

/// Packs a name of up to NAME_MAX bytes into two message words (loader and init protocols).
pub fn pack_name(name: &[u8]) -> Option<[usize; 2]> {
    if name.is_empty() || name.len() > NAME_MAX { return None; }
    let mut packed = [0u8; NAME_MAX]; packed[..name.len()].copy_from_slice(name);
    Some([usize::from_le_bytes(packed[..8].try_into().unwrap()), usize::from_le_bytes(packed[8..].try_into().unwrap())])
}

/// Name from two message words (up to the first zero byte).
pub fn unpack_name(words: [usize; 2]) -> ([u8; NAME_MAX], usize) {
    let mut packed = [0u8; NAME_MAX];
    packed[..8].copy_from_slice(&words[0].to_le_bytes()); packed[8..].copy_from_slice(&words[1].to_le_bytes());
    let len = packed.iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
    (packed, len)
}

/// Starts a program from disk in the background (via the loader service); `grant` passes an IPC endpoint with a rights mask to the child's INIT slot.
pub fn spawn(name: &str, grant: Option<(usize, u8)>) -> Result<u64> {
    let words = pack_name(name.as_bytes()).ok_or(crate::sys::Error::Invalid)?;
    let (slot, rights) = grant.unwrap_or((0, 0));
    let reply = crate::ipc::Endpoint::LOADER.call(&crate::ipc::Message::new(words[0], words[1]).with_cap(slot, rights), 0)?;
    check(reply.data[0]).map(|pid| pid as u64)
}

/// Starts a program from disk with arguments (via the loader service); no endpoint can be passed to the child.
pub fn spawn_with_args(name: &str, args: &str) -> Result<u64> {
    if name.is_empty() || name.len() > NAME_MAX || args.len() > ARGS_MAX || name.contains('\0') || args.contains('\0') { return Err(crate::sys::Error::Invalid); }
    let mut page = crate::mem::Pages::new(4096).ok_or(crate::sys::Error::NoMemory)?;
    let bytes = page.as_mut_slice();
    bytes[..name.len()].copy_from_slice(name.as_bytes()); bytes[name.len()] = 0;
    bytes[name.len() + 1..name.len() + 1 + args.len()].copy_from_slice(args.as_bytes()); bytes[name.len() + 1 + args.len()] = 0;
    let cap = page.share()?;
    let reply = crate::ipc::Endpoint::LOADER.call(&crate::ipc::Message::new(0, LOADER_RUN).with_cap(cap, 0), 0);
    let _ = crate::ipc::drop_cap(cap);
    check(reply?.data[0]).map(|pid| pid as u64)
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

/// Grant for SPAWN: the child's slot `child` gets a copy of the caller's slot `own`, endpoints narrowed by `rights`.
pub const fn grant(child: usize, own: usize, rights: u8) -> Grant { Grant { child: child as u8, own: own as u8, rights, reserved: 0 } }

/// Starts a task with exactly the granted capabilities; `flags` are SPAWN_SERVICE / SPAWN_SCREEN.
/// `name` may be `name\0arguments`.
pub fn spawn_raw(name: &[u8], image: Image, grants: &[Grant], flags: usize) -> Result<u64> {
    let (source, len) = match image { Image::Memory { cap, len } => (cap, len), Image::Boot(index) => (SPAWN_BOOT | index, 0) };
    check(syscall(SYSCALL_SPAWN, name.as_ptr() as usize, name.len(), [source, len, grants.as_ptr() as usize, grants.len() | flags << 8]).result).map(|pid| pid as u64)
}

pub fn alive(pid: u64) -> bool { call(SYSCALL_TASK_ALIVE, pid as usize, 0) == 1 }
