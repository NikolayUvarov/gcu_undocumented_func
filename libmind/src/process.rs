use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

pub fn exit() -> ! {
    call(SYSCALL_EXIT, 0, 0);
    loop { core::hint::spin_loop(); }
}

/// Пишет байты в журнал процесса.
pub fn log(bytes: &[u8]) { for chunk in bytes.chunks(4096) { call(SYSCALL_LOG, chunk.as_ptr() as usize, chunk.len()); } }

/// Приёмник для `print!`/`println!`.
pub struct Log;
impl core::fmt::Write for Log {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { log(text.as_bytes()); Ok(()) }
}

/// Запускает программу с диска в фоне (через сервис loader); `grant` передаёт ребёнку в слот INIT точку IPC с маской прав.
pub fn spawn(name: &str, grant: Option<(usize, u8)>) -> Result<u64> {
    if name.is_empty() || name.len() > NAME_MAX { return Err(crate::sys::Error::Invalid); }
    let mut packed = [0u8; NAME_MAX]; packed[..name.len()].copy_from_slice(name.as_bytes());
    let words = [usize::from_le_bytes(packed[..8].try_into().unwrap()), usize::from_le_bytes(packed[8..].try_into().unwrap())];
    let (slot, rights) = grant.unwrap_or((0, 0));
    let reply = crate::ipc::Endpoint::LOADER.call(&crate::ipc::Message::new(words[0], words[1]).with_cap(slot, rights), 0)?;
    check(reply.data[0]).map(|pid| pid as u64)
}

/// Только для loader: запуск ELF из блока памяти по мандату `image` (длина `len`); `request` — номер запроса шелла или 0.
pub fn spawn_image(name: &[u8], image: usize, len: usize, init: usize, mask: u8, request: usize) -> Result<u64> {
    check(syscall(SYSCALL_SPAWN_IMAGE, name.as_ptr() as usize, name.len(), [image, len, init, mask as usize | request << 16]).result).map(|pid| pid as u64)
}

/// Только для loader: завершает запрос шелла (длина ответа LIST или код ошибки запуска).
pub fn loader_done(request: usize, code: usize) -> Result<()> { check(call(SYSCALL_LOADER_DONE, request, code)).map(drop) }

pub fn alive(pid: u64) -> bool { call(SYSCALL_TASK_ALIVE, pid as usize, 0) == 1 }
