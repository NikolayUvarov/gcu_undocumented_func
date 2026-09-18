// Shared layout for the bootloader, kernel, and applications.
// Each binary uses only the parts of the ABI it needs.
#![allow(dead_code)]

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ProgramImage {
    pub data: *const u8,
    pub len: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub programs: [ProgramImage; 3],
    pub heap_ptr: *mut u8,
    pub heap_len: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SyscallMailbox {
    pub syscall_num: usize,
    pub arg1: usize,
    pub arg2: usize,
    pub result: usize,
}

impl SyscallMailbox {
    pub const EMPTY: Self = Self {
        syscall_num: 0,
        arg1: 0,
        arg2: 0,
        result: 0,
    };
}

// Returns seconds since midnight in the RTC's time zone, or RTC_UNAVAILABLE.
pub const SYSCALL_RTC_TIME: usize = 4;
pub const RTC_UNAVAILABLE: usize = usize::MAX;

// Wait up to arg1 milliseconds (maximum 60 s), waking early for input.
pub const SYSCALL_WAIT: usize = 5;
// Milliseconds since the kernel timer started, at 10 ms resolution.
pub const SYSCALL_UPTIME: usize = 6;
pub const SYSCALL_EXIT: usize = 7;
