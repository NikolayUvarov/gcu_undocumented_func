use crate::abi::{SyscallMailbox, SYSCALL_WAIT};
use core::arch::asm;

// Sleep only this task. Other runnable tasks execute; when all programs sleep,
// the shell/idle task halts the CPU. Foreground input can wake the task early.
pub fn wait(mailbox: *mut SyscallMailbox, milliseconds: usize) {
    unsafe {
        (*mailbox).syscall_num = SYSCALL_WAIT;
        (*mailbox).arg1 = milliseconds;
        asm!("int 0x80");
    }
}
