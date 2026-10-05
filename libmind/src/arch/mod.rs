//! Architecture layer of libmind (issue 200): the system-call instruction and the entropy instruction.
#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
