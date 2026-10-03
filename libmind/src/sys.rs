//! Единственное место с `int 0x80`: всё остальное в libmind строится поверх `syscall`.
use crate::abi::*;
use core::arch::asm;
use core::sync::atomic::{AtomicPtr, Ordering};

static MAILBOX: AtomicPtr<SyscallMailbox> = AtomicPtr::new(core::ptr::null_mut());

/// # Safety
/// `mailbox` — mailbox этого процесса, выданный ядром в `_start` (делает `entry!`).
pub unsafe fn init(mailbox: *mut SyscallMailbox) { MAILBOX.store(mailbox, Ordering::Relaxed); }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Invalid, NoSlot, Rights, NotFound, Peer, NoMemory, Other(usize) }
pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    pub fn code(self) -> usize {
        match self { Self::Invalid => ERR_INVALID, Self::NoSlot => ERR_NO_SLOT, Self::Rights => ERR_RIGHTS, Self::NotFound => ERR_NOT_FOUND, Self::Peer => ERR_PEER, Self::NoMemory => ERR_NO_MEMORY, Self::Other(code) => code }
    }
}

/// Преобразует ответ ядра или сервиса (`usize::MAX - n`) в `Result`.
pub fn check(value: usize) -> Result<usize> {
    match value {
        ERR_INVALID => Err(Error::Invalid), ERR_NO_SLOT => Err(Error::NoSlot), ERR_RIGHTS => Err(Error::Rights),
        ERR_NOT_FOUND => Err(Error::NotFound), ERR_PEER => Err(Error::Peer), ERR_NO_MEMORY => Err(Error::NoMemory),
        v if v >= ERR_FIRST => Err(Error::Other(v)),
        v => Ok(v),
    }
}

/// Содержимое mailbox после вызова.
#[derive(Clone, Copy, Debug)]
pub struct Raw { pub result: usize, pub arg1: usize, pub arg2: usize, pub msg: [usize; 4] }

pub fn syscall(number: usize, arg1: usize, arg2: usize, msg: [usize; 4]) -> Raw {
    let mb = MAILBOX.load(Ordering::Relaxed);
    if mb.is_null() { loop { core::hint::spin_loop(); } } // без entry! вызывать ядро нечем
    unsafe {
        core::ptr::write_volatile(mb, SyscallMailbox { syscall_num: number, arg1, arg2, result: 0, msg });
        asm!("int 0x80", options(nostack));
        let out = core::ptr::read_volatile(mb);
        Raw { result: out.result, arg1: out.arg1, arg2: out.arg2, msg: out.msg }
    }
}
pub fn call(number: usize, arg1: usize, arg2: usize) -> usize { syscall(number, arg1, arg2, [0; 4]).result }
