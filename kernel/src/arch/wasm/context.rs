// A task's saved state (215-PRT-0017): the frame of arch/aarch64/context.rs, word for word, so the scheduler saves and
// resumes it as there. `run` fills it from the emulator and fills ESR, FAR and the kind as an exception entry would.
pub const SIZE: usize = 832; // 37 words, then 32 V registers, FPCR and FPSR (the emulator keeps FPCR and FPSR only)
pub fn size() -> usize { SIZE }
pub const X0: usize = 0; pub const SP_EL0: usize = 31; pub const ELR: usize = 32; pub const SPSR: usize = 33; pub const ESR: usize = 34; pub const FAR: usize = 35; pub const KIND: usize = 36;
pub const MSI_FIRST: usize = 16;
// Kinds, as the aarch64 vectors push them.
pub const SYNC_KERNEL: u64 = 0; pub const IRQ_KERNEL: u64 = 1; pub const SYNC_TASK: u64 = 2; pub const IRQ_TASK: u64 = 3;
pub const EC_SVC64: u64 = 0x15;

pub unsafe fn frame(sp: usize) -> &'static mut [u64; 37] { &mut *(sp as *mut [u64; 37]) }

/// A task's data abort on a translation fault (EC 0x24, DFSC 0b0001xx): whether it was a write (WnR).
pub fn missing_page(code: u64, error: u64) -> Option<bool> { (code == 0x24 && error & 0x3C == 0x04).then_some(error & 1 << 6 != 0) }
/// Whether a data abort was a write refused by a page's permissions (DFSC 0b0011xx, WnR).
pub fn write_protected(code: u64, error: u64) -> bool { code == 0x24 && error & 0x3C == 0x0C && error & 1 << 6 != 0 }

pub enum Event {
    #[allow(dead_code)] // no other CPU stops this one
    Stop,
    KernelFault { code: u64, pc: u64, error: u64 },
    Tick,
    Clock,
    Irq(usize),
    Wake,
    Fault { code: u64, error: u64, pc: u64, address: u64 },
    Syscall,
}

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

// The code a task returns into when its entry function returns: EXIT with code 0 through the mailbox, as on aarch64.
pub fn exit_stub(mailbox: u64) -> [u8; 36] {
    let movz = |hw: u32, imm: u64| 0xD280_0000u32 | hw << 21 | ((imm >> (16 * hw)) as u32 & 0xFFFF) << 5;
    let movk = |hw: u32, imm: u64| 0xF280_0000u32 | hw << 21 | ((imm >> (16 * hw)) as u32 & 0xFFFF) << 5;
    let words = [movz(0, mailbox), movk(1, mailbox), movk(2, mailbox), movk(3, mailbox), 0xD280_00E1, 0xF900_0001, 0xF900_041F, 0xD400_0001, 0x0000_0000];
    let mut code = [0u8; 36];
    for (bytes, word) in code.chunks_exact_mut(4).zip(words) { bytes.copy_from_slice(&word.to_le_bytes()); }
    code
}

pub unsafe fn prepare_stack(_stack: usize, size: usize) -> usize { crate::paging::USER_STACK + size }
/// No checked reads fault here.
pub unsafe fn resume_at(_sp: usize, _pc: u64) {}
pub unsafe fn save(sp: usize, destination: usize) { core::ptr::copy(sp as *const u8, destination as *mut u8, SIZE); }

pub unsafe fn initial(saved: usize, entry: usize, stack_top: usize) {
    core::ptr::write_bytes(saved as *mut u8, 0, SIZE);
    let f = frame(saved);
    f[X0] = crate::paging::USER_INFO as u64;
    f[X0 + 1] = crate::paging::USER_MAILBOX as u64;
    f[30] = crate::paging::USER_EXIT as u64;
    f[SP_EL0] = stack_top as u64;
    f[ELR] = entry as u64;
    f[SPSR] = 0; // EL0t
    f[KIND] = SYNC_TASK;
}

// STAT_CPUS: 1, every task's FP state is saved (FPCR and FPSR; the emulator has no V registers).
pub fn saved_state() -> u64 { 1 }
