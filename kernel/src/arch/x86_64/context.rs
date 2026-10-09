use core::arch::global_asm;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Saved state: a pointer to the registers, the vector area at offset 64 (XSAVE, or FXSAVE's 512 bytes), the registers.
// The area holds every state component the kernel enabled (XCR0): AVX since issue 153, AVX-512 and AMX (174-KRN-0037).
pub static XSAVE: AtomicBool = AtomicBool::new(false);
// The enabled components (XCR0) and the area's size, set by the boot CPU before any entry; the other CPUs follow.
pub static XCR0: AtomicU64 = AtomicU64::new(0);
static AREA: AtomicUsize = AtomicUsize::new(512);
pub fn area() -> usize { AREA.load(Ordering::Relaxed) }
/// A task's saved state in bytes.
pub fn size() -> usize { 64 + area() + 22 * 8 }
// The size CPUID 0xD reports for the enabled components, in 64-byte units; the test kernel pads it to AMX's.
pub fn set_area(bytes: usize) {
    let bytes = if cfg!(feature = "xsave-pad-test") { bytes.max(11 * 1024) } else { bytes };
    AREA.store(bytes.div_ceil(64) * 64, Ordering::Release);
}
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
    sub rsp, [rip + {area}]
    sub rsp, 64
    and rsp, -64
    mov [rsp], rbx
    test byte ptr [rip + {xsave}], 1
    jz 4f
    // XSAVE writes only the saved components' bits of the header: stack bytes left there would make XRSTOR fault.
    xor eax, eax
    .irp off,576,584,592,600,608,616,624,632
        mov [rsp + \off], rax
    .endr
    mov eax, -1
    mov edx, -1
    xsave64 [rsp + 64]
    jmp 5f
4:
    fxsave64 [rsp + 64]
5:
    cld
    mov rdi, rsp
    call {handler}
    mov rsp, rax
    test byte ptr [rip + {xsave}], 1
    jz 6f
    mov eax, -1
    mov edx, -1
    xrstor64 [rsp + 64]
    jmp 7f
6:
    fxrstor64 [rsp + 64]
7:
    mov rsp, [rsp]
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
"#, handler = sym crate::scheduler::interrupt, xsave = sym XSAVE, area = sym AREA);

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
    &*(*(sp as *const usize) as *const [u64; 22])
}
// The interrupted kernel code resumes at `pc` with RAX and RDX zero (a checked MSR read that faulted).
pub unsafe fn resume_at(sp: usize, pc: u64) {
    let registers = *(sp as *const usize) as *mut u64;
    *registers.add(17) = pc; *registers.add(14) = 0; *registers.add(11) = 0;
}
// `destination` is 64-byte aligned (XSAVE) and `size()` long.
pub unsafe fn save(sp: usize, destination: usize) {
    let area = area();
    core::ptr::copy_nonoverlapping((sp + 64) as *const u8, (destination + 64) as *mut u8, area);
    core::ptr::copy_nonoverlapping(registers(sp).as_ptr(), (destination + 64 + area) as *mut u64, 22);
    *(destination as *mut usize) = destination + 64 + area;
}
// A zero XSAVE header starts every component in its initial state; FCW and MXCSR are set in the legacy area.
pub unsafe fn initial(saved: usize, entry: usize, stack_top: usize) {
    let area = area();
    let words = core::slice::from_raw_parts_mut((saved + 64 + area) as *mut u64, 22);
    words.fill(0);
    words[8] = crate::paging::USER_INFO as u64;
    words[9] = crate::paging::USER_MAILBOX as u64;
    words[17] = entry as u64;
    words[18] = 0x23;
    words[19] = 0x202;
    words[20] = stack_top as u64;
    words[21] = 0x1b;
    *((saved + 64) as *mut u16) = 0x37f;
    *((saved + 64 + 24) as *mut u32) = 0x1f80;
    *(saved as *mut usize) = saved + 64 + area;
}

// What an entry into the kernel was, decoded from the saved state (the generic kernel knows no vector numbers).
pub enum Event {
    Stop, // another CPU stops this one
    KernelFault { code: u64, pc: u64, error: u64 },
    Tick, // the timer, on the BSP or forwarded to this CPU
    Irq(usize), // a device line 1-15 or an MSI-X vector (MSI_FIRST + n), already acknowledged at the controller
    Wake, // a CPU woke this one to reschedule
    Fault { code: u64, error: u64, pc: u64, address: u64 }, // the task faulted; `code` is the exception vector
    Syscall,
}
pub const MSI_FIRST: usize = 16;

// Decodes the entry at `sp` and acknowledges it at the interrupt controllers (a device line stays masked until its
// driver acknowledges it).
pub unsafe fn event(sp: usize) -> Event {
    use super::{cpu, interrupts};
    let r = registers(sp); let vector = r[15];
    match vector {
        0x31 => Event::Stop,
        0..=31 if r[18] & 3 == 0 => Event::KernelFault { code: vector, pc: r[17], error: r[16] },
        0..=31 => Event::Fault { code: vector, error: r[16], pc: r[17], address: if vector == 14 { cpu::fault_address() } else { 0 } },
        32 => { interrupts::advance(); if interrupts::tick_from_pit() { interrupts::pic_eoi(0); } cpu::eoi(); cpu::tick_others(); Event::Tick }
        33..=47 => { let line = vector as u8 - 32; interrupts::set_irq_masked(line, true); interrupts::pic_eoi(line); cpu::eoi(); Event::Irq(line as usize) }
        0x40..=0x4F => { cpu::eoi(); Event::Irq(MSI_FIRST + vector as usize - 0x40) }
        48 => { cpu::eoi(); Event::Tick }
        50 => { cpu::eoi(); Event::Wake }
        _ => Event::Syscall,
    }
}

// The code a task returns into when its entry function returns: EXIT with code 0 through the mailbox at `mailbox`.
pub fn exit_stub(mailbox: u64) -> [u8; 29] {
    let mut code = [0u8; 29];
    code[0..2].copy_from_slice(&[0x48, 0xb8]); // mov rax, mailbox
    code[2..10].copy_from_slice(&mailbox.to_le_bytes());
    code[10..17].copy_from_slice(&[0x48, 0xc7, 0x00, 7, 0, 0, 0]); // mov qword [rax], 7 (EXIT)
    code[17..25].copy_from_slice(&[0x48, 0xc7, 0x40, 8, 0, 0, 0, 0]); // mov qword [rax + 8], 0 (arg1: the code, issue 166)
    code[25..29].copy_from_slice(&[0xcd, 0x80, 0x0f, 0x0b]); // int 0x80; ud2
    code
}

// The task's first stack: the return address of its entry function is the exit stub. `stack` is the kernel's view of
// the stack memory; returns the task's stack pointer.
pub unsafe fn prepare_stack(stack: usize, size: usize) -> usize {
    ((stack + size - 8) as *mut usize).write(crate::paging::USER_EXIT);
    crate::paging::USER_STACK + size - 8
}

// The state components saved per task with XSAVE (XCR0), 0 with FXSAVE (STAT_CPUS).
pub fn saved_state() -> u64 { if XSAVE.load(Ordering::Acquire) { XCR0.load(Ordering::Acquire) } else { 0 } }
