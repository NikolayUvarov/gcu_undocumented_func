//! Test-only ELF: never sleeps or yields after its initial message. The shell
//! must still respond and be able to kill it via timer preemption.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_: &abi::BootInfo, mailbox: *mut abi::SyscallMailbox) -> ! {
    unsafe {
        let message = b"BUSY FIXTURE: NO WAIT/YIELD CALLS\r\n";
        (*mailbox).syscall_num = 3;
        (*mailbox).arg1 = message.as_ptr() as usize;
        (*mailbox).arg2 = message.len();
        asm!("int 0x80");
        // Keep SIMD values live across arbitrary timer preemptions. Other tasks
        // start with zeroed SIMD registers, making missing FXRSTOR detectable.
        asm!(
            "pcmpeqd xmm0, xmm0",
            "2:",
            "pmovmskb eax, xmm0",
            "cmp eax, 65535",
            "jne 3f",
            "inc rdx",
            "jmp 2b",
            "3:",
            "ud2",
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
