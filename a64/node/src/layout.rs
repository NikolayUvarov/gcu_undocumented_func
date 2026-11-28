// Where the stand-in puts a program, and BootInfo's bytes it fills (215-PRT-0016). `tests/layout.rs` checks the
// offsets against common/abi.rs as a 64-bit program sees it.
/// USER_IMAGE on aarch64 (kernel/src/arch/aarch64/mmu.rs): a static PIE is loaded and relocated here.
pub const IMAGE: u64 = 255 << 39;
pub const IMAGE_BYTES: usize = 16 << 20;
/// A program linked at a fixed address (the differential program) is loaded here.
pub const LOW: u64 = 0x10_0000;
pub const LOW_BYTES: usize = 4 << 20;
pub const STACK_BYTES: usize = 1 << 20;
pub const STACK: u64 = IMAGE + (1 << 30);
/// The information page (BootInfo), then the mailbox.
pub const PAGES: u64 = IMAGE + (2 << 30);
/// What ALLOC hands out, page by page; FREE gives nothing back.
pub const HEAP: u64 = IMAGE + (3 << 30);
pub const HEAP_BYTES: usize = 16 << 20;
pub const BOOT_INFO_ABI_VERSION: usize = 720;
pub const ABI_VERSION: u32 = 40;
/// SyscallMailbox: syscall_num, arg1, arg2, result, then four message words.
pub const MAILBOX_NUMBER: usize = 0;
pub const MAILBOX_ARG1: usize = 8;
pub const MAILBOX_ARG2: usize = 16;
pub const MAILBOX_RESULT: usize = 24;
pub const SYSCALL_LOG: u64 = 3;
pub const SYSCALL_UPTIME: u64 = 6;
pub const SYSCALL_EXIT: u64 = 7;
pub const SYSCALL_ALLOC: u64 = 8;
pub const SYSCALL_FREE: u64 = 9;
pub const ERR_INVALID: u64 = u64::MAX;
