//! aarch64: `svc #0` and RNDR (issue 201).
use core::arch::asm;

/// Enters the kernel; the request and the result are in the mailbox.
///
/// # Safety
/// The mailbox must hold a complete request.
#[inline(always)]
pub unsafe fn trap() { asm!("svc #0", options(nostack)); }

/// Whether the processor has RNDR; the kernel reads ID_AA64ISAR0_EL1 (EL0 cannot) and says so in the info page.
pub fn entropy_available() -> bool {
    let info = crate::sys::info_address() as *const crate::abi::BootInfo;
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*info).cpu_features)) & crate::abi::FEATURE_ENTROPY != 0 }
}

/// One attempt of RNDR: the value, or None when it had none ready (Z set).
pub fn entropy() -> Option<u64> {
    let (value, ok): (u64, u64);
    unsafe { asm!("mrs {v}, s3_3_c2_c4_0", "cset {ok}, ne", v = out(reg) value, ok = out(reg) ok, options(nomem, nostack)); }
    (ok != 0).then_some(value)
}
