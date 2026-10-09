// EL1 exception vectors and the saved state of a task (issue 201). An entry pushes a frame (x0-x30, SP_EL0, ELR,
// SPSR, ESR, FAR and the vector) on the current stack: the CPU's exception stack for an entry from a task (TPIDR_EL1
// holds its top), the idle stack for one from the kernel. The handler returns the frame to resume; a task's frame
// lives in its context record. After the frame's words come V0-V31, FPCR and FPSR (250-KRN-0056): the common entry
// saves them and the exit loads them from the frame it resumes. The kernel itself is soft-float and never touches them.
use core::arch::global_asm;

pub const SIZE: usize = 832; // 37 words (304 bytes, 16-byte aligned), then 32 V registers, FPCR and FPSR
pub fn size() -> usize { SIZE }
const X0: usize = 0; const SP_EL0: usize = 31; const ELR: usize = 32; const SPSR: usize = 33; const ESR: usize = 34; const FAR: usize = 35; const KIND: usize = 36;
pub const MSI_FIRST: usize = 16;
// Vector kinds (the pushed KIND word).
const SYNC_KERNEL: u64 = 0; const IRQ_KERNEL: u64 = 1; const SYNC_TASK: u64 = 2; const IRQ_TASK: u64 = 3;

global_asm!(r#"
    .arch_extension fp
    .arch_extension simd
    .macro save kind
        sub sp, sp, #832
        stp x0, x1, [sp, #0]
        stp x2, x3, [sp, #16]
        stp x4, x5, [sp, #32]
        stp x6, x7, [sp, #48]
        stp x8, x9, [sp, #64]
        stp x10, x11, [sp, #80]
        stp x12, x13, [sp, #96]
        stp x14, x15, [sp, #112]
        stp x16, x17, [sp, #128]
        stp x18, x19, [sp, #144]
        stp x20, x21, [sp, #160]
        stp x22, x23, [sp, #176]
        stp x24, x25, [sp, #192]
        stp x26, x27, [sp, #208]
        stp x28, x29, [sp, #224]
        mrs x0, sp_el0
        stp x30, x0, [sp, #240]
        mrs x0, elr_el1
        mrs x1, spsr_el1
        stp x0, x1, [sp, #256]
        mrs x0, esr_el1
        mrs x1, far_el1
        stp x0, x1, [sp, #272]
        mov x0, #\kind
        str x0, [sp, #288]
        b context_common
    .endm
    .section .text.vectors, "ax"
    .balign 2048
    .global exception_vectors
exception_vectors:
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 0
    .balign 128
    save 1
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 2
    .balign 128
    save 3
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
    .balign 128
    save 4
context_common:
    add x0, sp, #304
    stp q0, q1, [x0, #0]
    stp q2, q3, [x0, #32]
    stp q4, q5, [x0, #64]
    stp q6, q7, [x0, #96]
    stp q8, q9, [x0, #128]
    stp q10, q11, [x0, #160]
    stp q12, q13, [x0, #192]
    stp q14, q15, [x0, #224]
    stp q16, q17, [x0, #256]
    stp q18, q19, [x0, #288]
    stp q20, q21, [x0, #320]
    stp q22, q23, [x0, #352]
    stp q24, q25, [x0, #384]
    stp q26, q27, [x0, #416]
    stp q28, q29, [x0, #448]
    stp q30, q31, [x0, #480]
    mrs x1, fpcr
    str x1, [x0, #512]
    mrs x1, fpsr
    str x1, [x0, #520]
    mov x0, sp
    bl {handler}
    mov sp, x0
    add x0, sp, #304
    ldp q0, q1, [x0, #0]
    ldp q2, q3, [x0, #32]
    ldp q4, q5, [x0, #64]
    ldp q6, q7, [x0, #96]
    ldp q8, q9, [x0, #128]
    ldp q10, q11, [x0, #160]
    ldp q12, q13, [x0, #192]
    ldp q14, q15, [x0, #224]
    ldp q16, q17, [x0, #256]
    ldp q18, q19, [x0, #288]
    ldp q20, q21, [x0, #320]
    ldp q22, q23, [x0, #352]
    ldp q24, q25, [x0, #384]
    ldp q26, q27, [x0, #416]
    ldp q28, q29, [x0, #448]
    ldp q30, q31, [x0, #480]
    ldr x1, [x0, #512]
    msr fpcr, x1
    ldr x1, [x0, #520]
    msr fpsr, x1
    ldp x0, x1, [sp, #256]
    msr elr_el1, x0
    msr spsr_el1, x1
    ldp x30, x0, [sp, #240]
    msr sp_el0, x0
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    // Back to a task: the stack becomes the CPU's exception stack; back to the kernel: the frame is popped.
    ldr x0, [sp, #264]
    tst x0, #0xf
    mrs x0, tpidr_el1
    add x1, sp, #832
    csel x1, x0, x1, eq
    mov x0, sp
    mov sp, x1
    ldp x0, x1, [x0]
    eret
    .previous
"#, handler = sym crate::scheduler::interrupt);

unsafe extern "C" {
    pub static exception_vectors: u8;
}

unsafe fn frame(sp: usize) -> &'static mut [u64; 37] { &mut *(sp as *mut [u64; 37]) }

// What an entry into the kernel was, decoded from the frame (the generic kernel knows no vector numbers).
pub enum Event {
    Stop,
    KernelFault { code: u64, pc: u64, error: u64 },
    Tick,
    Irq(usize),
    Wake,
    Fault { code: u64, error: u64, pc: u64, address: u64 },
    Syscall,
}

const EC_SVC64: u64 = 0x15;

pub unsafe fn event(sp: usize) -> Event {
    let f = frame(sp);
    let class = f[ESR] >> 26 & 0x3F;
    match f[KIND] {
        SYNC_TASK if class == EC_SVC64 => Event::Syscall,
        SYNC_TASK => Event::Fault { code: class, error: f[ESR], pc: f[ELR], address: f[FAR] },
        SYNC_KERNEL if class == EC_SVC64 => Event::Syscall, // the idle loop asks to reschedule
        IRQ_KERNEL | IRQ_TASK => super::interrupts::acknowledge(),
        _ => Event::KernelFault { code: class, pc: f[ELR], error: f[ESR] },
    }
}

// The code a task returns into when its entry function returns: EXIT with code 0 through the mailbox at `mailbox`.
pub fn exit_stub(mailbox: u64) -> [u8; 36] {
    let movz = |hw: u32, imm: u64| 0xD280_0000u32 | hw << 21 | ((imm >> (16 * hw)) as u32 & 0xFFFF) << 5; // movz x0
    let movk = |hw: u32, imm: u64| 0xF280_0000u32 | hw << 21 | ((imm >> (16 * hw)) as u32 & 0xFFFF) << 5; // movk x0
    let words = [movz(0, mailbox), movk(1, mailbox), movk(2, mailbox), movk(3, mailbox),
                 0xD280_00E1, // mov x1, #7 (EXIT)
                 0xF900_0001, // str x1, [x0]
                 0xF900_041F, // str xzr, [x0, #8] (arg1: the code, issue 166)
                 0xD400_0001, // svc #0
                 0x0000_0000]; // udf #0
    let mut code = [0u8; 36];
    for (bytes, word) in code.chunks_exact_mut(4).zip(words) { bytes.copy_from_slice(&word.to_le_bytes()); }
    code
}

// The task's stack pointer: the return address is in x30 (initial), not on the stack.
pub unsafe fn prepare_stack(_stack: usize, size: usize) -> usize { crate::paging::USER_STACK + size }

// `destination` is a task's context record.
/// Never needed on aarch64: no checked read faults (arch/aarch64/report.rs).
pub unsafe fn resume_at(_sp: usize, _pc: u64) {}
pub unsafe fn save(sp: usize, destination: usize) { core::ptr::copy(sp as *const u8, destination as *mut u8, SIZE); }

pub unsafe fn initial(saved: usize, entry: usize, stack_top: usize) {
    core::ptr::write_bytes(saved as *mut u8, 0, SIZE); // the vector registers and FPCR start at zero too
    let f = frame(saved);
    f[X0] = crate::paging::USER_INFO as u64;
    f[X0 + 1] = crate::paging::USER_MAILBOX as u64;
    f[30] = crate::paging::USER_EXIT as u64;
    f[SP_EL0] = stack_top as u64;
    f[ELR] = entry as u64;
    f[SPSR] = 0; // EL0t, interrupts enabled
    f[KIND] = SYNC_TASK;
}

// STAT_CPUS: 1, every task's FP/SIMD state (V0-V31, FPCR, FPSR) is saved.
pub fn saved_state() -> u64 { 1 }
