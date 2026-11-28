// What the page gives the kernel (215-PRT-0017): imports of the module `mind`, each a plain call that returns at once.
#[link(wasm_import_module = "mind")]
unsafe extern "C" {
    /// One byte of the kernel's console.
    pub fn console_write(byte: u32);
    /// Nanoseconds since the page started the kernel, never going back.
    pub fn now_ns() -> u64;
    /// The system stopped (0), asked to restart (1) or stopped on a fault (2): the page ends the kernel.
    pub fn halt(why: u32);
    /// A random 64-bit word from the page's cryptographic source, for RNDR.
    pub fn random() -> u64;
    /// Seconds since 1970, UTC, for the PL031.
    pub fn wall_s() -> u64;
}

pub fn stop(why: u32) -> ! {
    unsafe { halt(why); }
    core::arch::wasm64::unreachable()
}
