#![no_std]

pub mod abi {
    pub const PROGRAM_COUNT: usize = 9; 
    #[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
    #[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; PROGRAM_COUNT], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], }
    #[derive(Clone, Copy)] #[repr(C)] pub struct SyscallMailbox { pub syscall_num: usize, pub arg1: usize, pub arg2: usize, pub result: usize, pub msg: [usize; 4], }
    impl SyscallMailbox { pub const EMPTY: Self = Self { syscall_num: 0, arg1: 0, arg2: 0, result: 0, msg: [0; 4] }; }

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
    pub const SYSCALL_MEM_SHARE: usize = 15;
    pub const SYSCALL_MEM_MAP: usize = 16;
    
    pub const CAP_READ: u8  = 1 << 0; pub const CAP_WRITE: u8 = 1 << 1; pub const CAP_GRANT: u8 = 1 << 2;
}

pub mod sys {
    use super::abi::*;
    use core::arch::asm;

    pub fn wait(mb: *mut SyscallMailbox, ms: usize) {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_WAIT); core::ptr::write_volatile(&mut (*mb).arg1, ms); asm!("int 0x80", options(nostack)); }
    }

    pub fn endpoint_create(mb: *mut SyscallMailbox) -> Result<usize, usize> {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_ENDPOINT_CREATE); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res >= usize::MAX - 10 { Err(usize::MAX - res) } else { Ok(res) }
    }

    pub fn spawn(mb: *mut SyscallMailbox, name: &[u8], ep_slot: usize, rights: u8) -> Result<usize, usize> {
        unsafe { 
            core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_SPAWN); 
            core::ptr::write_volatile(&mut (*mb).arg1, name.as_ptr() as usize); 
            core::ptr::write_volatile(&mut (*mb).arg2, name.len()); 
            core::ptr::write_volatile(&mut (*mb).msg[0], ep_slot); 
            core::ptr::write_volatile(&mut (*mb).msg[1], rights as usize); 
            asm!("int 0x80", options(nostack)); 
        }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res >= usize::MAX - 10 { Err(usize::MAX - res) } else { Ok(res) }
    }

    pub fn ipc_send(mb: *mut SyscallMailbox, target_ep: usize, cap_to_transfer: usize, rights: u8, data: usize) -> Result<(), usize> {
        unsafe { 
            core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_SEND); 
            core::ptr::write_volatile(&mut (*mb).arg1, target_ep); 
            core::ptr::write_volatile(&mut (*mb).arg2, 0); 
            core::ptr::write_volatile(&mut (*mb).msg[0], cap_to_transfer); 
            core::ptr::write_volatile(&mut (*mb).msg[1], rights as usize); 
            core::ptr::write_volatile(&mut (*mb).msg[2], data); 
            asm!("int 0x80", options(nostack)); 
        }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res != 0 { Err(usize::MAX - res) } else { Ok(()) }
    }

    pub fn ipc_recv(mb: *mut SyscallMailbox, listen_ep: usize, recv_cap_slot: usize) -> Result<usize, usize> {
        unsafe { 
            core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_RECV); 
            core::ptr::write_volatile(&mut (*mb).arg1, listen_ep); 
            core::ptr::write_volatile(&mut (*mb).arg2, recv_cap_slot); 
            asm!("int 0x80", options(nostack)); 
        }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res != 0 { Err(usize::MAX - res) } else { Ok(unsafe { core::ptr::read_volatile(&(*mb).msg[2]) }) }
    }

    pub fn mem_alloc(mb: *mut SyscallMailbox, size: usize) -> Result<usize, usize> {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_ALLOC); core::ptr::write_volatile(&mut (*mb).arg1, size); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res == 0 { Err(1) } else { Ok(res) }
    }

    pub fn mem_share(mb: *mut SyscallMailbox, vaddr: usize, size: usize) -> Result<usize, usize> {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_MEM_SHARE); core::ptr::write_volatile(&mut (*mb).arg1, vaddr); core::ptr::write_volatile(&mut (*mb).arg2, size); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res >= usize::MAX - 10 { Err(usize::MAX - res) } else { Ok(res) }
    }

    pub fn mem_map(mb: *mut SyscallMailbox, cap_slot: usize) -> Result<usize, usize> {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_MEM_MAP); core::ptr::write_volatile(&mut (*mb).arg1, cap_slot); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res >= usize::MAX - 10 { Err(usize::MAX - res) } else { Ok(res) }
    }

    pub fn mem_free(mb: *mut SyscallMailbox, vaddr: usize) -> Result<(), usize> {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_FREE); core::ptr::write_volatile(&mut (*mb).arg1, vaddr); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res != 0 { Err(usize::MAX - res) } else { Ok(()) }
    }

    pub fn check_keys_and_wait(mb: *mut SyscallMailbox, ms: usize) {
        loop { 
            unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 2); asm!("int 0x80", options(nostack)); } 
            let key = unsafe { core::ptr::read_volatile(&(*mb).result) }; 
            if key == 0 { break; } 
            if key == 0x01 || key == 0x1B { 
                unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_EXIT); asm!("int 0x80", options(nostack)); } 
            } 
        }
        wait(mb, ms);
    }
}
