//! Ownership wrapper for the page-block ABI. This is not a sub-page allocator.
#![allow(dead_code)]
use crate::abi::{SyscallMailbox, SYSCALL_ALLOC, SYSCALL_FREE};
use core::arch::asm;
use core::ptr::NonNull;

pub struct Pages {
    pointer: NonNull<u8>,
    length: usize,
    mailbox: *mut SyscallMailbox,
}

impl Pages {
    /// The mailbox must be this process's kernel-provided mailbox and remain
    /// valid until Drop. The caller must not free the block through another API.
    pub unsafe fn new(mailbox: *mut SyscallMailbox, bytes: usize) -> Option<Self> {
        (*mailbox).syscall_num = SYSCALL_ALLOC;
        (*mailbox).arg1 = bytes;
        (*mailbox).arg2 = 0;
        asm!("int 0x80");
        Some(Self {
            pointer: NonNull::new((*mailbox).result as *mut u8)?,
            length: bytes,
            mailbox,
        })
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.length) }
    }
}

impl Drop for Pages {
    fn drop(&mut self) {
        unsafe {
            (*self.mailbox).syscall_num = SYSCALL_FREE;
            (*self.mailbox).arg1 = self.pointer.as_ptr() as usize;
            (*self.mailbox).arg2 = 0;
            asm!("int 0x80");
        }
    }
}
