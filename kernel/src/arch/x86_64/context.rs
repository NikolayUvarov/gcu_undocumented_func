use core::arch::global_asm;
use core::sync::atomic::AtomicBool;

// Saved state: the x87/SSE/AVX area (XSAVE, or FXSAVE in its first 512 bytes), a pointer to the registers, the
// registers. AVX state is saved per task when the CPU has XSAVE and AVX (issue 153).
pub const AREA: usize = 1024; // XSAVE of x87, SSE and AVX: 832 bytes in the standard format
pub const SIZE: usize = AREA + 16 + 22 * 8;
pub static XSAVE: AtomicBool = AtomicBool::new(false);
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
    // PIC lines for ring 3 drivers (7 and 15 remain spurious interrupts).
    .irp n,1,3,4,5,6,9,10,11,12,13,14
        interrupt task_irq_\n, (32 + \n)
    .endr
    // MSI-X vectors for ring 3 drivers (PLATFORM_DEVICE_MSIX).
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
        interrupt task_msi_\n, (64 + \n)
    .endr
    interrupt task_ipi_entry, 48
    interrupt task_stop_entry, 49
    interrupt task_wake_entry, 50
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
    sub rsp, 1040
    and rsp, -64
    test byte ptr [rip + {xsave}], 1
    jz 4f
    // XSAVE writes only the saved components' bits of the header: stack bytes left there would make XRSTOR fault.
    xor eax, eax
    .irp off,512,520,528,536,544,552,560,568
        mov [rsp + \off], rax
    .endr
    mov eax, -1
    mov edx, -1
    xsave64 [rsp]
    jmp 5f
4:
    fxsave64 [rsp]
5:
    mov [rsp + 1024], rbx
    cld
    mov rdi, rsp
    call {handler}
    mov rsp, rax
    test byte ptr [rip + {xsave}], 1
    jz 6f
    mov eax, -1
    mov edx, -1
    xrstor64 [rsp]
    jmp 7f
6:
    fxrstor64 [rsp]
7:
    mov rsp, [rsp + 1024]
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
    .global irq_table
irq_table:
    .irp n,1,3,4,5,6,9,10,11,12,13,14
        .quad task_irq_\n - irq_table
    .endr
    .global msi_table
msi_table:
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
        .quad task_msi_\n - msi_table
    .endr
    .previous
"#, handler = sym crate::scheduler::interrupt, xsave = sym XSAVE);

unsafe extern "C" {
    pub fn task_timer_entry();
    pub fn task_ipi_entry();
    pub fn task_stop_entry();
    pub fn task_wake_entry();
    pub fn task_syscall_entry();
    pub static exception_table: [u64; 32];
    pub static irq_table: [u64; 11];
    pub static msi_table: [u64; 16];
}

pub const IRQ_LINES: [u8; 11] = [1, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14];

pub unsafe fn registers(sp: usize) -> &'static [u64; 22] {
    &*(*(sp.wrapping_add(AREA) as *const usize) as *const [u64; 22])
}
// `destination` is 64-byte aligned (XSAVE).
pub unsafe fn save(sp: usize, destination: usize) {
    core::ptr::copy_nonoverlapping(sp as *const u8, destination as *mut u8, AREA);
    core::ptr::copy_nonoverlapping(registers(sp).as_ptr(), (destination + AREA + 16) as *mut u64, 22);
    *((destination + AREA) as *mut usize) = destination + AREA + 16;
}
// A zero XSAVE header starts every component in its initial state; FCW and MXCSR are set in the legacy area.
pub unsafe fn initial(saved: usize, entry: usize, stack_top: usize) {
    let words = core::slice::from_raw_parts_mut((saved + AREA + 16) as *mut u64, 22);
    words.fill(0);
    words[8] = crate::paging::USER_INFO as u64;
    words[9] = crate::paging::USER_MAILBOX as u64;
    words[17] = entry as u64;
    words[18] = 0x23;
    words[19] = 0x202;
    words[20] = stack_top as u64;
    words[21] = 0x1b;
    *(saved as *mut u16) = 0x37f;
    *((saved + 24) as *mut u32) = 0x1f80;
    *((saved + AREA) as *mut usize) = saved + AREA + 16;
}
