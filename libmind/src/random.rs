//! Random bytes from the processor (RDRAND, usable in ring 3). There is no fallback: without RDRAND, or when it keeps
//! failing, callers get `None` and must fail closed (issue 103).

/// Whether the processor has RDRAND (CPUID leaf 1, ECX bit 30).
pub fn available() -> bool {
    let leaf = core::arch::x86_64::__cpuid(1);
    leaf.ecx & (1 << 30) != 0
}

/// One 64-bit value; retries a few times as Intel recommends, refuses the all-zero and all-one values a broken unit gives.
pub fn u64() -> Option<u64> {
    if !available() { return None; }
    for _ in 0..16 {
        let (value, ok): (u64, u8);
        unsafe { core::arch::asm!("rdrand {v}", "setc {ok}", v = out(reg) value, ok = out(reg_byte) ok, options(nomem, nostack)); }
        if ok != 0 && value != 0 && value != u64::MAX { return Some(value); }
    }
    None
}

/// Fills `out` with random bytes; false (and `out` zeroed) when RDRAND is missing or fails.
pub fn fill(out: &mut [u8]) -> bool {
    for chunk in out.chunks_mut(8) {
        let Some(value) = u64() else { out.fill(0); return false };
        chunk.copy_from_slice(&value.to_le_bytes()[..chunk.len()]);
    }
    true
}
