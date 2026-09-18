use core::arch::{asm, global_asm};

// 64-bit interrupt entry always pushes SS, RSP, RFLAGS, CS, RIP. Save every
// general register and the x87/SSE state before entering Rust. Kernel and apps
// use x86_64-unknown-none (no red zone, softfloat, no AVX).
// Intel SDM vol. 3A, 6.14; https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html
global_asm!(
    r#"
    .macro task_interrupt name, handler
    .global \name
    \name:
        push rax
        push rbx
        push rcx
        push rdx
        push rbp
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15
        mov rbx, rsp
        sub rsp, 528
        and rsp, -16
        fxsave64 [rsp]
        mov [rsp + 512], rbx
        cld
        mov rdi, rsp
        call \handler
        mov rsp, rax
        fxrstor64 [rsp]
        mov rsp, [rsp + 512]
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rdx
        pop rcx
        pop rbx
        pop rax
        iretq
    .endm
    task_interrupt task_timer_entry, {timer}
    task_interrupt task_syscall_entry, {syscall}
    "#,
    timer = sym crate::scheduler::timer_interrupt,
    syscall = sym crate::scheduler::syscall_interrupt,
);

unsafe extern "C" {
    pub fn task_timer_entry();
    pub fn task_syscall_entry();
}

pub unsafe fn initial(stack_top: usize, entry: usize) -> usize {
    let cs: u16;
    let ss: u16;
    asm!("mov {0:x}, cs", out(reg) cs);
    asm!("mov {0:x}, ss", out(reg) ss);
    let regs = ((stack_top - 8) & !15) - 20 * 8;
    let words = core::slice::from_raw_parts_mut(regs as *mut u64, 20);
    words.fill(0);
    words[15] = entry as u64;
    words[16] = cs as u64;
    words[17] = 0x202;
    words[18] = (stack_top - 8) as u64;
    words[19] = ss as u64;
    let saved = (regs - 528) & !15;
    // Architectural initial x87 control word and MXCSR. All registers are zero.
    *(saved as *mut u16) = 0x37f;
    *((saved + 24) as *mut u32) = 0x1f80;
    *((saved + 512) as *mut usize) = regs;
    saved
}
