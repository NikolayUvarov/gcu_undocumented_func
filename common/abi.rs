#![allow(dead_code)]

pub const PROGRAM_COUNT: usize = 6; 

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ProgramImage { pub data: *const u8, pub len: usize }

#[derive(Clone, Copy)]
#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize,
    pub programs: [ProgramImage; PROGRAM_COUNT],
    pub heap_ptr: *mut u8, pub heap_len: usize,
    pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SyscallMailbox {
    pub syscall_num: usize,
    pub arg1: usize, pub arg2: usize, pub result: usize,
    pub msg: [usize; 4],
}

impl SyscallMailbox {
    pub const EMPTY: Self = Self { syscall_num: 0, arg1: 0, arg2: 0, result: 0, msg: [0; 4] };
}

pub const SYSCALL_RTC_TIME: usize = 4;
pub const RTC_UNAVAILABLE: usize = usize::MAX;
pub const SYSCALL_WAIT: usize = 5;
pub const SYSCALL_UPTIME: usize = 6;
pub const SYSCALL_EXIT: usize = 7;
pub const SYSCALL_ALLOC: usize = 8;
pub const SYSCALL_FREE: usize = 9;

pub const SYSCALL_IPC_SEND: usize = 10;
pub const SYSCALL_IPC_RECV: usize = 11;
pub const SYSCALL_ENDPOINT_CREATE: usize = 12;
pub const SYSCALL_SPAWN: usize = 13;
pub const SYSCALL_CAP_DROP: usize = 14;

// Новые вызовы для разделяемой памяти
pub const SYSCALL_MEM_SHARE: usize = 15; // Создать мандат из кучи
pub const SYSCALL_MEM_MAP: usize = 16;   // Отобразить мандат в кучу

pub const CAP_READ: u8  = 1 << 0;
pub const CAP_WRITE: u8 = 1 << 1;
pub const CAP_GRANT: u8 = 1 << 2;

pub const HEAP_PAGE_SIZE: usize = 4096;
pub const HEAP_MAX_BLOCKS: usize = 32;
pub const HEAP_MAX_BYTES: usize = 16 * 1024 * 1024;
