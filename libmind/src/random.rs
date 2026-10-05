//! Random bytes from the processor (RDRAND, usable in ring 3). There is no fallback: without RDRAND, or when it keeps
//! failing, callers get `None` and must fail closed (issue 103).

/// The processor's random number instruction, for messages.
pub const SOURCE: &str = if cfg!(target_arch = "aarch64") { "RNDR" } else { "RDRAND" };

/// Whether the processor has RDRAND (CPUID leaf 1, ECX bit 30) or RNDR.
pub fn available() -> bool {
    crate::arch::entropy_available()
}

/// One 64-bit value; retries a few times as Intel recommends, refuses the all-zero and all-one values a broken unit gives.
pub fn u64() -> Option<u64> {
    if !available() { return None; }
    for _ in 0..16 {
        if let Some(value) = crate::arch::entropy().filter(|&v| v != 0 && v != u64::MAX) { return Some(value); }
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
