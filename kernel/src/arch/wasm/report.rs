// The processor's part of the hardware report (215-PRT-0017): the emulator, not a CPU.
use alloc::string::String;
use core::fmt::Write;

pub fn resume_after_fault(_pc: u64) -> Option<u64> { None }
pub fn cpu(out: &mut String) {
    let _ = writeln!(out, "CPU\n  the a64 emulator  user-mode A64, integer only, in a wasm64 module (215-PRT-0017)\n  CPUs started      1");
}
pub fn kernel(out: &mut String) {
    let _ = writeln!(out, "KERNEL\n  arch              wasm64, driven by its page\n  protection        the emulator's page-table checks");
}
