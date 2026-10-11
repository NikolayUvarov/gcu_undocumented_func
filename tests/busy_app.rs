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
        let xcr0 = if ecx & (1 << 27) != 0 {
            let (low, _high): (u32, u32);
            asm!("xgetbv", in("ecx") 0u32, out("eax") low, out("edx") _high);
            low
        } else { 0 };
        let avx = ecx & (1 << 28) != 0 && xcr0 & 6 == 6;
        // AVX-512 when the kernel saves opmask, ZMM_Hi256 and Hi16_ZMM too (174-KRN-0037).
        let avx512 = avx && xcr0 & 0xE0 == 0xE0;
        let message: &[u8] = if avx512 { b"BUSY FIXTURE: NO WAIT/YIELD CALLS, AVX-512\r\n" } else if avx { b"BUSY FIXTURE: NO WAIT/YIELD CALLS, AVX\r\n" } else { b"BUSY FIXTURE: NO WAIT/YIELD CALLS\r\n" };
        (*mailbox).syscall_num = 3;
        (*mailbox).arg1 = message.as_ptr() as usize;
        (*mailbox).arg2 = message.len();
        asm!("int 0x80");
        if avx512 {
            // zmm0's upper half, zmm31 (Hi16_ZMM) and k1 hold values only this task wrote.
            asm!(
                "rdtsc",
                "shl rdx, 32",
                "or rax, rdx",
                "mov r12, rax",
                "not rax",
                "mov r13, rax",
                "vpbroadcastq zmm0, r12",
                "vpbroadcastq zmm1, r13",
                "vinserti64x4 zmm0, zmm0, ymm1, 1",
                "vmovdqa64 zmm31, zmm0",
                "mov eax, r12d",
                "kmovw k1, eax",
                "2:",
                "vmovq rax, xmm0",
                "cmp rax, r12",
                "jne 3f",
                "vextracti64x4 ymm2, zmm0, 1",
                "vmovq rax, xmm2",
                "cmp rax, r13",
                "jne 3f",
                "vextracti64x4 ymm3, zmm31, 1",
                "vmovq rax, xmm3",
                "cmp rax, r13",
                "jne 3f",
                "kmovw eax, k1",
                "cmp ax, r12w",
                "jne 3f",
                "inc rdx",
                "jmp 2b",
                "3:",
                "ud2",
                options(noreturn)
            );
        }
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

// aarch64 (issue 203, 250-KRN-0056): the values kept across preemptions are in callee-saved general registers, in both
// halves of V8 and V31, and in FPCR's rounding mode; another task that ran in between would have changed them (tasks
// start with zeroed vector registers and FPCR).
#[cfg(target_arch = "aarch64")]
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(_: &abi::BootInfo, mailbox: *mut abi::SyscallMailbox) -> ! {
    unsafe {
        let message: &[u8] = b"BUSY FIXTURE: NO WAIT/YIELD CALLS, FP/SIMD\r\n";
        (*mailbox).syscall_num = abi::SYSCALL_LOG;
        (*mailbox).arg1 = message.as_ptr() as usize;
        (*mailbox).arg2 = message.len();
        asm!("svc #0");
        asm!(
            ".arch_extension fp",
            ".arch_extension simd",
            "mrs x19, cntvct_el0",
            "orr x19, x19, #1",
            "mvn x20, x19",
            "mov x21, x19",
            "mov x22, x20",
            "fmov d8, x19",
            "mov v8.d[1], x20",
            "fmov d31, x20",
            "mov v31.d[1], x19",
            "mov x24, #1 << 22",
            "msr fpcr, x24",
            "2:",
            "cmp x19, x21",
            "b.ne 3f",
            "cmp x20, x22",
            "b.ne 3f",
            "fmov x25, d8",
            "cmp x25, x19",
            "b.ne 3f",
            "mov x25, v8.d[1]",
            "cmp x25, x20",
            "b.ne 3f",
            "fmov x25, d31",
            "cmp x25, x20",
            "b.ne 3f",
            "mov x25, v31.d[1]",
            "cmp x25, x19",
            "b.ne 3f",
            "mrs x25, fpcr",
            "cmp x25, x24",
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
