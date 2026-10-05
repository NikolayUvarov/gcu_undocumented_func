//! x86-64: `int 0x80` and RDRAND.
use core::arch::asm;

/// Enters the kernel; the request and the result are in the mailbox.
///
/// # Safety
/// The mailbox must hold a complete request.
#[inline(always)]
pub unsafe fn trap() { asm!("int 0x80", options(nostack)); }

/// Whether the processor has a hardware entropy instruction (RDRAND: CPUID leaf 1, ECX bit 30).
pub fn entropy_available() -> bool { core::arch::x86_64::__cpuid(1).ecx & (1 << 30) != 0 }

/// One attempt of the entropy instruction: the value, or None when the unit had none ready.
pub fn entropy() -> Option<u64> {
    let (value, ok): (u64, u8);
    unsafe { asm!("rdrand {v}", "setc {ok}", v = out(reg) value, ok = out(reg_byte) ok, options(nomem, nostack)); }
    (ok != 0).then_some(value)
}
