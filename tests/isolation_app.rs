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
        if call(mb, abi::SYSCALL_CAP_INFO, abi::SLOT_INIT, 0) == abi::CAP_KIND_ENDPOINT {
            // Started by the 'k' case with a badged endpoint: one message through it, then exit.
            (*mb).msg = [0, 0, 0xBAD6E, 0];
            call(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 0);
            return;
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
                let mint = |handle: usize, mask: u8| { (*mb).msg[0] = 0; (*mb).msg[1] = 0; (*mb).msg[2] = 0; call(mb, abi::SYSCALL_CAP_MINT, handle, mask as usize) };
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
                    (abi::SYSCALL_STAT, abi::STAT_TASKS, 0, abi::ERR_RIGHTS), // observation needs the observe privilege (MC-10.2)
                    (abi::SYSCALL_TASK_LIST, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_IPC_CALL, abi::SLOT_SYSINFO, 0, abi::ERR_INVALID), // sysmon only for programs that request it (loader v1)
                    (abi::SYSCALL_IPC_CALL, abi::SLOT_LIFECYCLE, 0, abi::ERR_INVALID),
                ];
                for (number, a, b, expected) in checks {
                    if call(mb, number, a, b) != expected {
                        asm!("ud2", options(noreturn));
                    }
                }
                // MIND IDL: the rtc service checks requests against idl/rtc.wit (status 0x81 version, 0x80 invalid).
                let rtc = |word: usize| { let raw = mb; (*raw).msg = [0, 0, word, 0]; if call(raw, abi::SYSCALL_IPC_CALL, abi::SLOT_RTC, 0) != 0 { usize::MAX } else { (*raw).msg[2] & 0xFF } };
                if rtc(1 | 2 << 8) != 0x81 || rtc(9 | 1 << 8) != 0x80 || rtc(1 | 1 << 8 | 1 << 40) != 0x80 || rtc(1 | 1 << 8) > 1 {
                    asm!("ud2", options(noreturn));
                }
                // Derivation: a mint is never wider than its source; revoking a capability removes its descendants only.
                let mint = |handle: usize, mask: usize, offset: usize, length: usize| { (*mb).msg[0] = offset; (*mb).msg[1] = length; (*mb).msg[2] = 0; call(mb, abi::SYSCALL_CAP_MINT, handle, mask) };
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
                // Deadlines: send, call and receive on an endpoint nobody serves time out and leave nothing queued.
                let timed = |handle: usize| handle | 30 << abi::IPC_TIMEOUT_SHIFT;
                let start = call(mb, abi::SYSCALL_UPTIME, 0, 0);
                let raw = mb; (*raw).msg = [0, 0, 7, 7];
                if call(mb, abi::SYSCALL_IPC_SEND, timed(endpoints[1]), 0) != abi::ERR_TIMEOUT
                    || call(mb, abi::SYSCALL_IPC_CALL, timed(endpoints[1]), 0) != abi::ERR_TIMEOUT
                    || call(mb, abi::SYSCALL_IPC_RECV, timed(endpoints[1]), 0) != abi::ERR_TIMEOUT
                    || call(mb, abi::SYSCALL_UPTIME, 0, 0) - start < 90 {
                    asm!("ud2", options(noreturn));
                }
                // A keeper (no read right) cannot receive but can mint a receiver of the same endpoint.
                let keeper = mint(endpoints[0], (abi::CAP_KEEP | abi::CAP_WRITE) as usize, 0, 0);
                let reader = mint(keeper, abi::CAP_READ as usize, 0, 0);
                if rights(endpoints[0]) != (abi::CAP_READ | abi::CAP_WRITE | abi::CAP_GRANT | abi::CAP_KEEP) as usize
                    || rights(keeper) != (abi::CAP_KEEP | abi::CAP_WRITE) as usize || rights(reader) != abi::CAP_READ as usize
                    || call(mb, abi::SYSCALL_IPC_RECV, keeper, 0) != abi::ERR_RIGHTS {
                    asm!("ud2", options(noreturn));
                }
                // Endpoint badges: set once on a child of an unbadged capability, kept by its children, reported by
                // CAP_INFO and delivered with every message sent through it.
                let badge_mint = |handle: usize, mask: usize, badge: usize| { (*mb).msg = [0, 0, badge, 0]; call(mb, abi::SYSCALL_CAP_MINT, handle, mask) };
                let badge_of = |handle: usize| { if call(mb, abi::SYSCALL_CAP_INFO, handle, 0) == abi::CAP_KIND_ENDPOINT { (*mb).arg2 } else { usize::MAX } };
                let endpoint = endpoints[2];
                let badged = badge_mint(endpoint, (abi::CAP_WRITE | abi::CAP_GRANT) as usize, 0x1234);
                let inherited = badge_mint(badged, abi::CAP_WRITE as usize, 0);
                if endpoint >= abi::ERR_FIRST || badged >= abi::ERR_FIRST || inherited >= abi::ERR_FIRST || badge_of(endpoint) != 0 || badge_of(badged) != 0x1234 || badge_of(inherited) != 0x1234
                    || badge_mint(badged, abi::CAP_WRITE as usize, 0x5678) != abi::ERR_INVALID // a badge cannot be changed
                    || badge_mint(endpoint, abi::CAP_WRITE as usize, 0x10000) != abi::ERR_INVALID { // 16 bits
                    asm!("ud2", options(noreturn));
                }
                // A copy of this program gets the badged endpoint in its INIT slot from the loader and sends through it.
                let mut name = [0u8; 16]; name[..4].copy_from_slice(b"app2");
                (*mb).msg = [badged, abi::CAP_WRITE as usize, usize::from_le_bytes(name[..8].try_into().unwrap()), 0];
                let child = if call(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_LOADER, 0) == 0 { (*mb).msg[2] } else { usize::MAX };
                let received = call(mb, abi::SYSCALL_IPC_RECV, endpoint | 3000 << abi::IPC_TIMEOUT_SHIFT, 0);
                if child >= abi::ERR_FIRST || received != 0 || (*mb).arg1 != child || (*mb).msg[2] != 0xBAD6E
                    || (*mb).msg[1] >> abi::MSG_BADGE_SHIFT & abi::BADGE_MAX != 0x1234 {
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, badged, 0); call(mb, abi::SYSCALL_CAP_DROP, inherited, 0);
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
                // VFS v2 (idl/vfs.wit, encoded by hand): an application's client reads only. It opens the RAM disk's
                // root, but creating a file there is denied (status 2, error 4 = denied).
                let page = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                let shared = call(mb, abi::SYSCALL_MEM_SHARE, page, 0);
                let string = |text: &[u8]| { let b = page as *mut u8; *b = text.len() as u8; *b.add(1) = 0; for (i, &c) in text.iter().enumerate() { *b.add(2 + i) = c; } 2 + text.len() };
                let n = string(b"ram");
                (*mb).msg = [shared, 0, 1 | 2 << 8 | n << 16, 0];
                let opened = call(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_VFS, 0) == 0 && (*mb).msg[2] & 0xFF == 0;
                let root = (*mb).msg[2] >> 16 & 0xFFFF_FFFF;
                let n = string(b"x.txt");
                (*mb).msg = [shared, 0, 3 | 2 << 8 | n << 16, root | ((abi::VFS_MODE_WRITE | abi::VFS_MODE_CREATE) as usize) << 32];
                let denied = call(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_VFS, 0) == 0 && (*mb).msg[2] & 0xFF == 2 && (*mb).msg[2] >> 16 & 0xFF == 4;
                if !opened || !denied { asm!("ud2", options(noreturn)); }
                call(mb, abi::SYSCALL_CAP_DROP, shared, 0); call(mb, abi::SYSCALL_FREE, page, 0);
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
