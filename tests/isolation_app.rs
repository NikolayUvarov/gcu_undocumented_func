//! Test-only ring-3 adversary. A key selects a fault; it is never packaged into
//! the normal OS. Tests keep other processes running while each case terminates.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;
use abi::SyscallMailbox;

unsafe fn call(mb: *mut SyscallMailbox, number: usize, a: usize, b: usize) -> usize {
    (*mb).syscall_num = number;
    (*mb).arg1 = a;
    (*mb).arg2 = b;
    asm!("int 0x80");
    (*mb).result
}
unsafe fn print(mb: *mut SyscallMailbox, message: &[u8]) {
    call(mb, 3, message.as_ptr() as usize, message.len());
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_: &abi::BootInfo, mb: *mut SyscallMailbox) {
    unsafe {
        let cs: u16;
        let flags: usize;
        asm!("mov {0:x}, cs", out(reg) cs);
        asm!("pushfq", "pop {}", out(reg) flags);
        if cs & 3 != 3 || flags & 0x3000 != 0 {
            asm!("ud2", options(noreturn));
        }
        print(mb, b"RING3 IOPL0 READY\r\n");
        let mode = loop {
            let key = call(mb, 2, 0, 0) as u8;
            if key.is_ascii_alphabetic() {
                break key;
            }
            call(mb, 5, 10, 0);
        };
        match mode {
            b'r' => {
                let _ = core::ptr::read_volatile(0x100000 as *const u64);
            }
            b'd' => {
                // A detached block leaves the address space.
                let block = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                if call(mb, abi::SYSCALL_MEM_DETACH, block, 0) & abi::HANDLE_SLOT_MASK < abi::SLOT_DYNAMIC { asm!("ud2", options(noreturn)); }
                let _ = core::ptr::read_volatile(block as *const u64);
            }
            b'm' | b'v' => {
                // 'm': a read-only mint maps read-only, so a write faults. 'v': revoking a lease unmaps it, so a read faults.
                let mint = |handle: usize, mask: u8| { (*mb).msg[0] = 0; (*mb).msg[1] = 0; call(mb, abi::SYSCALL_CAP_MINT, handle, mask as usize) };
                let memory = call(mb, abi::SYSCALL_MEM_SHARE, call(mb, abi::SYSCALL_ALLOC, 4096, 0), 0);
                let lease = mint(memory, if mode == b'm' { abi::CAP_READ } else { abi::CAP_READ | abi::CAP_WRITE });
                let address = call(mb, abi::SYSCALL_MEM_MAP, lease, 0);
                let _ = core::ptr::read_volatile(address as *const u64);
                if mode == b'm' { core::ptr::write_volatile(address as *mut u64, 42); }
                core::ptr::write_volatile(address as *mut u64, 42);
                if call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0) != 1 { asm!("ud2", options(noreturn)); }
                let _ = core::ptr::read_volatile(address as *const u64);
            }
            b'w' => {
                core::ptr::write_volatile(0x100000 as *mut u64, 42);
            }
            b't' => {
                core::ptr::write_volatile(_start as *const () as *mut u8, 0xcc);
            }
            b'n' => {
                let code = [0xc3u8; 16];
                asm!("call {target}", target = in(reg) code.as_ptr());
            }
            b'c' => {
                asm!("cli");
            }
            b'o' => {
                asm!("out dx, al", in("dx") 0x3f8u16, in("al") b'!');
            }
            b'u' => {
                asm!("ud2");
            }
            b'g' => {
                let _ = core::ptr::read_volatile(0x80_0100_0000 as *const u8);
            }
            b's' => {
                asm!("mov rsp, 1", "ud2", options(noreturn));
            }
            b'y' => {
                asm!("syscall");
            }
            b'e' => {
                asm!("sysenter");
            }
            b'h' => {
                asm!("int 0x20");
            }
            b'p' => {
                for (pointer, size) in [
                    (0x100000, 16),
                    (usize::MAX - 1, 8),
                    (0x80_0400_1ffe, 4),
                    (0x80_0100_0000, 1),
                    (0x8000_0000_0000, 1),
                ] {
                    if call(mb, 3, pointer, size) != usize::MAX {
                        asm!("ud2", options(noreturn));
                    }
                }
                print(mb, b"POINTER VALIDATION OK\r\n");
                return;
            }
            b'k' => {
                // A regular application cannot use others' privileges without capabilities.
                let image = _start as *const () as usize;
                let checks = [
                    (abi::SYSCALL_INPUT_EVENT, b'x' as usize, b'x' as usize, abi::ERR_RIGHTS),
                    (abi::SYSCALL_COMPOSITOR_PULL, 9, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, abi::SLOT_RTC, 0x70, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, 31, 0x60, abi::ERR_RIGHTS),
                    (abi::SYSCALL_IRQ_WAIT, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_MAP, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_PHYS, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_SHARE, image, 4096, abi::ERR_INVALID), // code is not shareable
                    (abi::SYSCALL_MEM_SHARE, 0x80_0100_1000, 4096, abi::ERR_INVALID), // nor is the stack
                    (abi::SYSCALL_IPC_RECV, abi::SLOT_RTC, 0, abi::ERR_RIGHTS), // write-only access to another's service
                    (abi::SYSCALL_IPC_REPLY, 0, 0, abi::ERR_INVALID),
                    (abi::SYSCALL_SPAWN, image, 4, abi::ERR_RIGHTS), // spawning is for holders of the spawn privilege
                    (abi::SYSCALL_PLATFORM_CAP, abi::PLATFORM_PORTS, 0x60, abi::ERR_RIGHTS), // bootstrap authority is init's
                    (abi::SYSCALL_DEVICE_FIND, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_TASK_KILL, 1, 0, abi::ERR_RIGHTS), // process control is the shell's
                    (abi::SYSCALL_FOCUS, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_HALT, 0, 0, abi::ERR_RIGHTS),
                ];
                for (number, a, b, expected) in checks {
                    if call(mb, number, a, b) != expected {
                        asm!("ud2", options(noreturn));
                    }
                }
                // Derivation: a mint is never wider than its source; revoking a capability removes its descendants only.
                let mint = |handle: usize, mask: usize, offset: usize, length: usize| { (*mb).msg[0] = offset; (*mb).msg[1] = length; call(mb, abi::SYSCALL_CAP_MINT, handle, mask) };
                let rights = |handle: usize| { if call(mb, abi::SYSCALL_CAP_INFO, handle, 0) == abi::CAP_KIND_ENDPOINT { (*mb).msg[2] } else { usize::MAX } };
                let writer = mint(abi::SLOT_RTC, abi::CAP_WRITE as usize, 0, 0);
                let wider = mint(writer, (abi::CAP_READ | abi::CAP_WRITE | abi::CAP_GRANT) as usize, 0, 0);
                if rights(writer) != abi::CAP_WRITE as usize || rights(wider) != abi::CAP_WRITE as usize
                    || call(mb, abi::SYSCALL_CAP_REVOKE, abi::SLOT_RTC, 0) != 2
                    || call(mb, abi::SYSCALL_CAP_INFO, writer, 0) != abi::CAP_KIND_NONE || call(mb, abi::SYSCALL_CAP_INFO, wider, 0) != abi::CAP_KIND_NONE
                    || rights(abi::SLOT_RTC) != (abi::CAP_WRITE | abi::CAP_GRANT) as usize {
                    asm!("ud2", options(noreturn));
                }
                let pages = call(mb, abi::SYSCALL_ALLOC, 8192, 0);
                let memory = call(mb, abi::SYSCALL_MEM_SHARE, pages, 0);
                let half = mint(memory, abi::CAP_READ as usize, 4096, 4096);
                let size = |handle: usize| { if call(mb, abi::SYSCALL_CAP_INFO, handle, 0) == abi::CAP_KIND_MEMORY { (*mb).msg[2] } else { 0 } };
                if size(memory) != 8192 || size(half) != 4096 || mint(memory, abi::CAP_READ as usize, 4096, 8192) != abi::ERR_INVALID || mint(memory, abi::CAP_READ as usize, 100, 4096) != abi::ERR_INVALID {
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, half, 0); call(mb, abi::SYSCALL_CAP_DROP, memory, 0); call(mb, abi::SYSCALL_FREE, pages, 0);
                // Memory objects: a shared block cannot be detached; an object is move-only, mints read-only children
                // and is sealed once its writable capability is gone.
                let sealed = |handle: usize| { call(mb, abi::SYSCALL_CAP_INFO, handle, 0) == abi::CAP_KIND_MEMORY && (*mb).msg[3] == 1 };
                let info = |handle: usize| { call(mb, abi::SYSCALL_CAP_INFO, handle, 0); (*mb).arg2 };
                let block = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                core::ptr::write_volatile(block as *mut u64, 0x5EA1);
                let shared = call(mb, abi::SYSCALL_MEM_SHARE, block, 0);
                if call(mb, abi::SYSCALL_MEM_DETACH, block, 0) != abi::ERR_BUSY || sealed(shared) { asm!("ud2", options(noreturn)); }
                call(mb, abi::SYSCALL_CAP_DROP, shared, 0);
                let object = call(mb, abi::SYSCALL_MEM_DETACH, block, 0);
                let reader = mint(object, (abi::CAP_READ | abi::CAP_WRITE | abi::CAP_GRANT) as usize, 0, 0);
                let raw = mb; (*raw).msg[0] = object; (*raw).msg[1] = 0;
                if info(object) != (abi::CAP_READ | abi::CAP_WRITE) as usize || info(reader) != abi::CAP_READ as usize
                    || call(mb, abi::SYSCALL_FREE, block, 0) != abi::ERR_INVALID || sealed(reader)
                    || call(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_RTC, 0) != abi::ERR_RIGHTS { // copying the owner is refused
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, object, 0);
                let view = call(mb, abi::SYSCALL_MEM_MAP, reader, 0);
                if !sealed(reader) || core::ptr::read_volatile(view as *const u64) != 0x5EA1 { asm!("ud2", options(noreturn)); }
                call(mb, abi::SYSCALL_FREE, view, 0); call(mb, abi::SYSCALL_CAP_DROP, reader, 0);
                // Endpoint quota delegated by loader: four endpoints, the fifth is refused; dropping them frees the quota later.
                let mut endpoints = [0usize; 4];
                for handle in endpoints.iter_mut() {
                    *handle = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                    if *handle & abi::HANDLE_SLOT_MASK < abi::SLOT_DYNAMIC || *handle & abi::HANDLE_SLOT_MASK >= abi::CAP_SLOTS { asm!("ud2", options(noreturn)); }
                }
                if call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0) != abi::ERR_LIMIT { asm!("ud2", options(noreturn)); }
                // A keeper (no read right) cannot receive but can mint a receiver of the same endpoint.
                let keeper = mint(endpoints[0], (abi::CAP_KEEP | abi::CAP_WRITE) as usize, 0, 0);
                let reader = mint(keeper, abi::CAP_READ as usize, 0, 0);
                if rights(endpoints[0]) != (abi::CAP_READ | abi::CAP_WRITE | abi::CAP_GRANT | abi::CAP_KEEP) as usize
                    || rights(keeper) != (abi::CAP_KEEP | abi::CAP_WRITE) as usize || rights(reader) != abi::CAP_READ as usize
                    || call(mb, abi::SYSCALL_IPC_RECV, keeper, 0) != abi::ERR_RIGHTS {
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, reader, 0); call(mb, abi::SYSCALL_CAP_DROP, keeper, 0);
                for handle in endpoints { call(mb, abi::SYSCALL_CAP_DROP, handle, 0); }
                // A dropped handle stays dead when its slot is reused: same slot, new generation.
                let block = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                let fresh = call(mb, abi::SYSCALL_MEM_SHARE, block, 0);
                let stale = endpoints[0];
                if fresh & abi::HANDLE_SLOT_MASK != stale & abi::HANDLE_SLOT_MASK || fresh == stale
                    || call(mb, abi::SYSCALL_CAP_INFO, stale, 0) != abi::CAP_KIND_NONE
                    || call(mb, abi::SYSCALL_CAP_INFO, fresh, 0) != abi::CAP_KIND_MEMORY
                    || call(mb, abi::SYSCALL_CAP_DROP, stale, 0) != abi::ERR_INVALID
                    || call(mb, abi::SYSCALL_CAP_INFO, fresh & abi::HANDLE_SLOT_MASK, 0) != abi::CAP_KIND_NONE {
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, fresh, 0); call(mb, abi::SYSCALL_FREE, block, 0);
                let block = call(mb, abi::SYSCALL_ALLOC, 8192, 0);
                if block == 0 || call(mb, abi::SYSCALL_MEM_SHARE, block + 4096, 4096) != abi::ERR_INVALID || call(mb, abi::SYSCALL_MEM_SHARE, block, 3 * 4096) != abi::ERR_INVALID {
                    asm!("ud2", options(noreturn));
                }
                let slot = call(mb, abi::SYSCALL_MEM_SHARE, block, 0);
                if slot & abi::HANDLE_SLOT_MASK < abi::SLOT_DYNAMIC || call(mb, abi::SYSCALL_FREE, block, 0) != 0 || call(mb, abi::SYSCALL_CAP_DROP, slot, 0) != 0 {
                    asm!("ud2", options(noreturn));
                }
                print(mb, b"CAPABILITY CHECKS OK\r\n");
                return;
            }
            _ => {
                return;
            }
        }
        print(mb, b"ISOLATION FAILURE: OPERATION SUCCEEDED\r\n");
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe {
        asm!("ud2", options(noreturn));
    }
}
