// Shared layout for the bootloader, kernel, and applications.
// Each binary uses only the parts of the ABI it needs.
#![allow(dead_code)]

#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub app_entry: u64,
    pub app2_entry: u64,
    pub clock_entry: u64,
}

#[repr(C)]
pub struct SyscallMailbox {
    pub syscall_num: usize,
    pub arg1: usize,
    pub arg2: usize,
    pub result: usize,
}

// Returns seconds since midnight in the RTC's time zone, or RTC_UNAVAILABLE.
pub const SYSCALL_RTC_TIME: usize = 4;
pub const RTC_UNAVAILABLE: usize = usize::MAX;
