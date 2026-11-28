// Architecture layer (issue 200): the processor and its platform. The rest of the kernel reaches them only through
// these modules, so each architecture is a sibling directory with the same module names and functions.
#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;
// A page or Node drives the kernel; the tasks' A64 runs on the `a64` emulator (215-PRT-0017).
#[cfg(target_arch = "wasm64")]
mod wasm;
#[cfg(target_arch = "wasm64")]
pub use wasm::*;
