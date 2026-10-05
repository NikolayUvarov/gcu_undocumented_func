//! Test-only aarch64 service (issue 201): booted in the place of `rtc`, it logs a line and then faults in one of
//! four ways chosen at build time (`--cfg 'case="..."'`); the kernel must end it and init restart it, the rest of
//! the system unaffected.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;

unsafe fn log(mb: *mut abi::SyscallMailbox, text: &[u8]) {
    (*mb).syscall_num = abi::SYSCALL_LOG; (*mb).arg1 = text.as_ptr() as usize; (*mb).arg2 = text.len();
    asm!("svc #0");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(_info: &abi::BootInfo, mailbox: *mut abi::SyscallMailbox) -> ! {
    unsafe {
        log(mailbox, b"[FAULTTEST] RUNNING\n");
        // Kernel memory: the identity map is EL1 only.
        #[cfg(case = "kernel_read")]
        { let value = core::ptr::read_volatile(0x4000_0000 as *const u64); log(mailbox, if value == 0 { b"0" } else { b"1" }); }
        // Own code: mapped read-only and executable.
        #[cfg(case = "text_write")]
        core::ptr::write_volatile(_start as *const () as *mut u32, 0);
        // The stack: writable, never executable.
        #[cfg(case = "stack_exec")]
        { let code = [0xD65F_03C0u32; 4]; let f: extern "C" fn() = core::mem::transmute(code.as_ptr()); f(); }
        // An undefined instruction.
        #[cfg(case = "undefined")]
        asm!("udf #0");
        log(mailbox, b"[FAULTTEST] NOT STOPPED\n");
        loop { asm!("wfi"); }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
