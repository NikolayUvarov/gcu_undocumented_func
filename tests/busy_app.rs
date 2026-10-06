//! Test-only ELF: never sleeps or yields after its initial message. The shell
//! must still respond and be able to kill it via timer preemption.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;

#[cfg(target_arch = "x86_64")]
#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_: &abi::BootInfo, mailbox: *mut abi::SyscallMailbox) -> ! {
    unsafe {
        // AVX when the CPU has it and the kernel saves its state (OSXSAVE, XCR0 with SSE and AVX; issue 153).
        let ecx = core::arch::x86_64::__cpuid(1).ecx;
        let avx = ecx & (1 << 27) != 0 && ecx & (1 << 28) != 0 && {
            let (low, _high): (u32, u32);
            asm!("xgetbv", in("ecx") 0u32, out("eax") low, out("edx") _high);
            low & 6 == 6
        };
        let message: &[u8] = if avx { b"BUSY FIXTURE: NO WAIT/YIELD CALLS, AVX\r\n" } else { b"BUSY FIXTURE: NO WAIT/YIELD CALLS\r\n" };
        (*mailbox).syscall_num = 3;
        (*mailbox).arg1 = message.as_ptr() as usize;
        (*mailbox).arg2 = message.len();
        asm!("int 0x80");
        if avx {
            // Both halves of ymm0 hold values only this task wrote; another AVX task on the CPU writes its own.
            asm!(
                "rdtsc",
                "shl rdx, 32",
                "or rax, rdx",
                "mov r12, rax",
                "not rax",
                "mov r13, rax",
                "vmovq xmm0, r12",
                "vmovq xmm1, r13",
                "vinsertf128 ymm0, ymm0, xmm1, 1",
                "2:",
                "vmovq rax, xmm0",
                "cmp rax, r12",
                "jne 3f",
                "vextractf128 xmm2, ymm0, 1",
                "vmovq rax, xmm2",
                "cmp rax, r13",
                "jne 3f",
                "inc rdx",
                "jmp 2b",
                "3:",
                "ud2",
                options(noreturn)
            );
        }
        // Keep SIMD values live across arbitrary timer preemptions. Other tasks
        // start with zeroed SIMD registers, making missing FXRSTOR detectable.
        asm!(
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            "mov r12, rax",
            "movq xmm0, rax",
            "2:",
            "movq rax, xmm0",
            "cmp rax, r12",
            "jne 3f",
            "inc rdx",
            "jmp 2b",
            "3:",
            "ud2",
            options(noreturn)
        );
    }
}

// aarch64 (issue 203): no FP/SIMD state yet (soft-float), so the values kept across preemptions are in callee-saved
// general registers; another task that ran in between would have changed them.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(_: &abi::BootInfo, mailbox: *mut abi::SyscallMailbox) -> ! {
    unsafe {
        let message: &[u8] = b"BUSY FIXTURE: NO WAIT/YIELD CALLS\r\n";
        (*mailbox).syscall_num = abi::SYSCALL_LOG;
        (*mailbox).arg1 = message.as_ptr() as usize;
        (*mailbox).arg2 = message.len();
        asm!("svc #0");
        asm!(
            "mrs x19, cntvct_el0",
            "mvn x20, x19",
            "mov x21, x19",
            "mov x22, x20",
            "2:",
            "cmp x19, x21",
            "b.ne 3f",
            "cmp x20, x22",
            "b.ne 3f",
            "add x23, x23, #1",
            "b 2b",
            "3:",
            "udf #0",
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
