//! Bootstrap authority (holder of the platform capability, i.e. init): validated platform resources and devices.
use crate::abi::*;
use crate::sys::{check, syscall, Result};

/// Mints a capability over a platform resource into a new slot (`PLATFORM_*` kinds in the ABI).
pub fn cap(kind: usize, a: usize, b: usize) -> Result<usize> { check(syscall(SYSCALL_PLATFORM_CAP, kind, a, [b, 0, 0, 0]).result) }

/// The boot is good (platform privilege, ABI 4): on a trial boot the kernel no longer restarts the machine at the
/// deadline. Whether the boot was on trial (351-KRN-0014).
pub fn confirm_boot() -> Result<bool> { check(syscall(SYSCALL_BOOT_CONFIRM, 0, 0, [0; 4]).result).map(|trial| trial != 0) }

/// Keeps `bytes` of the frame pool for the system band: applications' allocations stop above it (MC-6.5, issue 169).
pub fn reserve_memory(bytes: usize) -> Result<()> { check(syscall(SYSCALL_MEMORY_RESERVE, bytes, 0, [0; 4]).result).map(|_| ()) }

/// Index of the `nth` PCI function whose class code matches `class` under `mask`.
pub fn find_device(class: u32, mask: u32, nth: usize) -> Result<usize> { find_device_id(class, mask, 0, nth) }
/// As `find_device`, also matching the PCI `vendor | device << 16` identifier (0: any).
pub fn find_device_id(class: u32, mask: u32, id: u32, nth: usize) -> Result<usize> { check(syscall(SYSCALL_DEVICE_FIND, class as usize, mask as usize, [nth, id as usize, 0, 0]).result) }

/// Stops a PCI device's decoding and DMA before its driver is restarted (platform privilege or a BAR capability).
pub fn quiesce(device: usize) -> Result<()> { check(syscall(SYSCALL_DEVICE_STATE, device, DEVICE_STOP, [0; 4]).result).map(drop) }
/// Turns a stopped device's decoding and DMA on again for its next driver.
pub fn resume(device: usize) -> Result<()> { check(syscall(SYSCALL_DEVICE_STATE, device, DEVICE_START, [0; 4]).result).map(drop) }
