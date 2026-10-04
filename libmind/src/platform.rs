//! Bootstrap authority (holder of the platform capability, i.e. init): validated platform resources and devices.
use crate::abi::*;
use crate::sys::{check, syscall, Result};

/// Mints a capability over a platform resource into a new slot (`PLATFORM_*` kinds in the ABI).
pub fn cap(kind: usize, a: usize, b: usize) -> Result<usize> { check(syscall(SYSCALL_PLATFORM_CAP, kind, a, [b, 0, 0, 0]).result) }

/// Index of the `nth` PCI function whose class code matches `class` under `mask`.
pub fn find_device(class: u32, mask: u32, nth: usize) -> Result<usize> { check(syscall(SYSCALL_DEVICE_FIND, class as usize, mask as usize, [nth, 0, 0, 0]).result) }

/// Stops a PCI device's decoding and DMA before its driver is restarted (platform privilege or a BAR capability).
pub fn quiesce(device: usize) -> Result<()> { check(syscall(SYSCALL_DEVICE_STATE, device, DEVICE_STOP, [0; 4]).result).map(drop) }
/// Turns a stopped device's decoding and DMA on again for its next driver.
pub fn resume(device: usize) -> Result<()> { check(syscall(SYSCALL_DEVICE_STATE, device, DEVICE_START, [0; 4]).result).map(drop) }
