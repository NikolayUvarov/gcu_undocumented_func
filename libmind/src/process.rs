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

/// Запускает программу в фоне; `grant` передаёт ребёнку в слот INIT точку IPC с маской прав.
pub fn spawn(name: &str, grant: Option<(usize, u8)>) -> Result<u64> {
    let (slot, rights) = grant.unwrap_or((0, 0));
    check(syscall(SYSCALL_SPAWN, name.as_ptr() as usize, name.len(), [slot, rights as usize, 0, 0]).result).map(|pid| pid as u64)
}

pub fn alive(pid: u64) -> bool { call(SYSCALL_TASK_ALIVE, pid as usize, 0) == 1 }
