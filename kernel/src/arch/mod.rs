// Architecture layer (issue 200): the processor and its platform. The rest of the kernel reaches them only through
// these modules, so another architecture adds a sibling of x86_64 with the same module names and functions.
#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
