use core::arch::global_asm;

// Every entry uses [15 GPRs, vector, error, RIP, CS, RFLAGS, RSP, SS].
// Ring-3 interrupts enter the CPU's private TSS.RSP0 stack. Contexts saved in a
// task are copied out of that stack before dispatching another task.
pub const SIZE: usize = 704;
global_asm!(r#"
    .macro exception n
    .global exception_\n
    exception_\n:
    .if (\n != 8) && (\n != 10) && (\n != 11) && (\n != 12) && (\n != 13) && (\n != 14) && (\n != 17) && (\n != 21) && (\n != 29) && (\n != 30)
        push 0
    .endif
        push \n
        jmp context_entry
    .endm
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
        exception \n
    .endr
    .macro interrupt name, vector
    .global \name
    \name:
        push 0
        push \vector
        jmp context_entry
    .endm
    interrupt task_timer_entry, 32
    interrupt task_ipi_entry, 48
    interrupt task_stop_entry, 49
    interrupt task_syscall_entry, 128
context_entry:
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
    call {handler}
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
    add rsp, 16
    iretq
    .section .rodata.exceptions,"a"
    .global exception_table
exception_table:
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
        .quad exception_\n - exception_table
    .endr
    .previous
"#, handler = sym crate::scheduler::interrupt);

unsafe extern "C" {
    pub fn task_timer_entry();
    pub fn task_ipi_entry();
    pub fn task_stop_entry();
    pub fn task_syscall_entry();
    pub static exception_table: [u64; 32];
}

pub unsafe fn registers(sp: usize) -> &'static [u64; 22] {
    &*(*(sp.wrapping_add(512) as *const usize) as *const [u64; 22])
}
pub unsafe fn save(sp: usize, destination: usize) {
    core::ptr::copy_nonoverlapping(sp as *const u8, destination as *mut u8, 512);
    core::ptr::copy_nonoverlapping(registers(sp).as_ptr(), (destination + 528) as *mut u64, 22);
    *((destination + 512) as *mut usize) = destination + 528;
}
pub unsafe fn initial(saved: usize, entry: usize, stack_top: usize) {
    let words = core::slice::from_raw_parts_mut((saved + 528) as *mut u64, 22);
    words.fill(0);
    words[8] = crate::paging::USER_INFO as u64; // RDI
    words[9] = crate::paging::USER_MAILBOX as u64; // RSI
    words[17] = entry as u64;
    words[18] = 0x23; // user code, RPL=3
    words[19] = 0x202; // IF=1, IOPL=0
    words[20] = stack_top as u64;
    words[21] = 0x1b; // user data, RPL=3
    *(saved as *mut u16) = 0x37f;
    *((saved + 24) as *mut u32) = 0x1f80;
    *((saved + 512) as *mut usize) = saved + 528;
}
