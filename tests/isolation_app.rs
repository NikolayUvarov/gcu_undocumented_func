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
unsafe fn ipc(mb: *mut SyscallMailbox, number: usize, a: usize, b: usize, msg: [usize; 4]) -> usize {
    (*mb).msg = msg;
    call(mb, number, a, b)
}
unsafe fn fail() -> ! { asm!("ud2", options(noreturn)) }
// Sleeps the full time: WAIT ends early while keys are queued, so typed input is drained meanwhile.
unsafe fn sleep(mb: *mut SyscallMailbox, ms: usize) {
    let start = call(mb, abi::SYSCALL_UPTIME, 0, 0);
    loop {
        let elapsed = call(mb, abi::SYSCALL_UPTIME, 0, 0) - start;
        if elapsed >= ms { return; }
        while call(mb, abi::SYSCALL_READ_KEY, 0, 0) != 0 {}
        call(mb, abi::SYSCALL_WAIT, ms - elapsed, 0);
    }
}
const SECONDS: usize = 1000 << abi::IPC_TIMEOUT_SHIFT; // a timeout in IPC's arg2 (issue 172)
// Starts a copy of this program through loader with `endpoint` (write/grant) in its INIT slot: a launch session of
// idl/loader.wit 1.1, encoded by hand (begin(name, args) in a buffer, grant(session, slot, cap), commit(session)).
unsafe fn spawn_child(mb: *mut SyscallMailbox, endpoint: usize) {
    let page = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
    core::ptr::write_volatile(page as *mut [u8; 8], [4, 0, b'a', b'p', b'p', b'2', 0, 0]);
    let shared = call(mb, abi::SYSCALL_MEM_SHARE, page, 0);
    let begun = ipc(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_LOADER, 0, [shared, 0, 3 | 1 << 8 | 8 << 16, 0]) == 0 && (*mb).msg[2] == 4 << 16;
    call(mb, abi::SYSCALL_CAP_REVOKE, shared, 0); call(mb, abi::SYSCALL_CAP_DROP, shared, 0);
    let session = core::ptr::read_volatile(page as *const u32) as usize;
    call(mb, abi::SYSCALL_FREE, page, 0);
    if !begun { fail(); }
    let rights = (abi::CAP_WRITE | abi::CAP_GRANT) as usize;
    if ipc(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_LOADER, 0, [endpoint, rights, 4 | 1 << 8 | session << 16 | abi::SLOT_INIT << 48, 0]) != 0
        || (*mb).msg[2] != 0 { fail(); }
    if ipc(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_LOADER, 0, [0, 0, 5 | 1 << 8 | session << 16, 0]) != 0 || (*mb).msg[2] & 0xFF != 0 { fail(); }
}
// A child's first call: its reply carries the mode (and maybe a capability, received in slot 2).
unsafe fn child(mb: *mut SyscallMailbox) {
    let hello = ipc(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_INIT, 2, [0; 4]);
    if hello == abi::ERR_PEER { return; } // case 'f': the parent died while the call was still queued
    if hello != 0 { fail(); }
    let (mode, got_cap, number) = ((*mb).msg[2], (*mb).msg[0], (*mb).msg[3]);
    match mode {
        1 => if ipc(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 10 * SECONDS, [0, 0, 1, number]) != 0 { fail(); },
        2 => {
            if ipc(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_INIT, 200 << abi::IPC_TIMEOUT_SHIFT, [0, 0, 3, 0]) != abi::ERR_TIMEOUT { fail(); }
            if ipc(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 0, [0, 0, 4, 0]) != 0 { fail(); }
        }
        3 | 5 => {
            let address = call(mb, abi::SYSCALL_MEM_MAP, 2, 0);
            if got_cap != 1 || address >= abi::ERR_FIRST { fail(); }
            let value = core::ptr::read_volatile(address as *const usize);
            if ipc(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 0, [0, 0, if mode == 3 { value } else { 6 }, 0]) != 0 { fail(); }
            // Mode 5 keeps reading a lease until the owner revokes it (a page fault ends the task).
            if mode == 5 { loop { core::ptr::read_volatile(address as *const usize); } }
        }
        6 => {}
        // Whether a capability came with the reply (a refused move delivers none).
        7 => if ipc(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 0, [0, 0, 100 + got_cap, 0]) != 0 { fail(); },
        4 => if ipc(mb, abi::SYSCALL_IPC_SEND, abi::SLOT_INIT, 0, [abi::SLOT_INIT, (abi::CAP_WRITE | abi::CAP_GRANT) as usize, 5, 0]) != 0 { fail(); },
        _ => fail(),
    }
}
// Receives one hello call and replies with the mode and an optional capability.
unsafe fn greet(mb: *mut SyscallMailbox, endpoint: usize, mode: usize, cap: usize, mask: usize) {
    if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[1] & abi::MSG_FLAG_CALL == 0 { fail(); }
    if ipc(mb, abi::SYSCALL_IPC_REPLY, 0, 0, [cap, mask, mode, 0]) != 0 { fail(); }
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
        // A copy started by one of the cases below (it has an endpoint in its INIT slot): see `child`.
        if call(mb, abi::SYSCALL_CAP_INFO, abi::SLOT_INIT, 0) == abi::CAP_KIND_ENDPOINT {
            child(mb);
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
            b'l' => {
                // A lease whose capability was dropped after mapping still ends when the owner revokes.
                let mint = |handle: usize, mask: u8| { (*mb).msg = [0; 4]; call(mb, abi::SYSCALL_CAP_MINT, handle, mask as usize) };
                let memory = call(mb, abi::SYSCALL_MEM_SHARE, call(mb, abi::SYSCALL_ALLOC, 4096, 0), 0);
                let lease = mint(memory, abi::CAP_READ | abi::CAP_WRITE);
                let address = call(mb, abi::SYSCALL_MEM_MAP, lease, 0);
                core::ptr::write_volatile(address as *mut u64, 42);
                call(mb, abi::SYSCALL_CAP_DROP, lease, 0);
                call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0);
                let _ = core::ptr::read_volatile(address as *const u64);
            }
            b'm' | b'v' => {
                // 'm': a read-only mint maps read-only, so a write faults. 'v': revoking a lease unmaps it, so a read faults.
                let mint = |handle: usize, mask: u8| { (*mb).msg = [0; 4]; call(mb, abi::SYSCALL_CAP_MINT, handle, mask as usize) };
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
                // Input arrives as event words: the UART newline typed after the key is a press carrying its byte,
                // decoded as Enter by the shell; READ_KEY and READ_INPUT drain the same queue.
                let mut event = 0;
                for _ in 0..20 { event = call(mb, abi::SYSCALL_READ_INPUT, 0, 0); if event != 0 { break; } call(mb, abi::SYSCALL_WAIT, 10, 0); }
                if !matches!(abi::event_byte(event), b'\n' | b'\r') || !abi::event_pressed(event) || !matches!(abi::event_key(event), 0 | abi::KEY_ENTER) || call(mb, abi::SYSCALL_READ_KEY, 0, 0) != 0 { fail(); }
                // A regular application cannot use others' privileges without capabilities.
                let image = _start as *const () as usize;
                let checks = [
                    (abi::SYSCALL_INPUT_EVENT, b'x' as usize, b'x' as usize, abi::ERR_RIGHTS),
                    (abi::SYSCALL_COMPOSITOR_PULL, 9, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, abi::SLOT_RTC, 0x70, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, 31, 0x60, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_OUT_BLOCK, abi::SLOT_RTC, 0x1F0, abi::ERR_RIGHTS), // no port range, no buffer
                    (abi::SYSCALL_IRQ_WAIT, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_REBOOT, 0, 0, abi::ERR_RIGHTS), // only process control resets the machine
                    (abi::SYSCALL_BOOT_CONFIRM, 0, 0, abi::ERR_RIGHTS), // only init (the platform privilege) ends a trial boot
                    (abi::SYSCALL_DEVICE_CONFIG, abi::SLOT_RTC, 0, abi::ERR_RIGHTS), // configuration space only through a BAR capability
                    (abi::SYSCALL_PLATFORM_CAP, abi::PLATFORM_DEVICE_MSIX, 0, abi::ERR_RIGHTS), // MSI-X vectors only through the platform privilege
                    (abi::SYSCALL_MEM_MAP, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_PHYS, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_SHARE, image, 4096, abi::ERR_INVALID), // code is not shareable
                    (abi::SYSCALL_MEM_SHARE, 0x80_0100_1000, 4096, abi::ERR_INVALID), // nor is the stack
                    (abi::SYSCALL_IPC_RECV, abi::SLOT_RTC, 0, abi::ERR_RIGHTS), // write-only access to another's service
                    (abi::SYSCALL_IPC_REPLY, 0, 0, abi::ERR_INVALID),
                    (abi::SYSCALL_SPAWN, image, 4, abi::ERR_RIGHTS), // spawning is for holders of the spawn privilege
                    (abi::SYSCALL_PLATFORM_CAP, abi::PLATFORM_PORTS, 0x60, abi::ERR_RIGHTS), // bootstrap authority is init's
                    (abi::SYSCALL_DEVICE_FIND, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEMORY_RESERVE, 0, 0, abi::ERR_RIGHTS), // the recovery reserve is init's policy (issue 169)
                    (abi::SYSCALL_TASK_KILL, 1, 0, abi::ERR_RIGHTS), // process control is the shell's
                    (abi::SYSCALL_FOCUS, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_HALT, 0, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_TASK_LIST, 0, 0, abi::ERR_RIGHTS), // observation needs the observe privilege (MC-10.2)
                    (abi::SYSCALL_IPC_CALL, abi::SLOT_SYSINFO, 0, abi::ERR_INVALID), // sysmon only for programs that request it (loader sessions)
                    (abi::SYSCALL_IPC_CALL, abi::SLOT_LIFECYCLE, 0, abi::ERR_INVALID),
                    (abi::SYSCALL_STAT, abi::STAT_TASKS, 0, abi::ERR_RIGHTS), // statistics need the observe privilege
                    (abi::SYSCALL_DEVICE_STATE, 0, abi::DEVICE_STOP, abi::ERR_RIGHTS), // stopping a device needs one of its BARs
                    (abi::SYSCALL_TASK_WATCH, 1, abi::SLOT_RTC, abi::ERR_RIGHTS), // only a lifecycle owner watches, on its own endpoint
                ];
                for (number, a, b, expected) in checks {
                    if call(mb, number, a, b) != expected {
                        asm!("ud2", options(noreturn));
                    }
                }
                // MIND IDL: the rtc service checks requests against idl/rtc.wit (status 0x81 version, 0x80 invalid).
                let rtc = |word: usize, extra: usize| { let raw = mb; (*raw).msg = [0, 0, word, extra]; if call(raw, abi::SYSCALL_IPC_CALL, abi::SLOT_RTC, 0) != 0 { usize::MAX } else { (*raw).msg[2] & 0xFF } };
                if rtc(1 | 2 << 8, 0) != 0x81 || rtc(9 | 1 << 8, 0) != 0x80 || rtc(1 | 1 << 8 | 1 << 40, 0) != 0x80 || rtc(1 | 1 << 8, 1) != 0x80 || rtc(1 | 1 << 8, 0) > 1 {
                    asm!("ud2", options(noreturn));
                }
                // MIND IDL v0.2: loader rejects buffer calls with a length past the bound or the buffer, or without a buffer.
                let page = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                let buffer = call(mb, abi::SYSCALL_MEM_SHARE, page, 0);
                core::ptr::write_volatile(page as *mut [u8; 4], [0x88, 0x13, b'a', b'b']); // a name of 5000 bytes
                let loader = |word: usize, cap: usize| { let raw = mb; (*raw).msg = [cap, 0, word, 0]; if call(raw, abi::SYSCALL_IPC_CALL, abi::SLOT_LOADER, 0) != 0 { usize::MAX } else { (*raw).msg[2] & 0xFF } };
                if loader(2 | 1 << 8 | 10 << 16, buffer) != 0x80 || loader(1 | 1 << 8 | 8192 << 16, buffer) != 0x80 || loader(1 | 1 << 8, 0) != 0x80 || loader(1 | 1 << 8, buffer) != 0 {
                    fail();
                }
                call(mb, abi::SYSCALL_CAP_DROP, buffer, 0); call(mb, abi::SYSCALL_FREE, page, 0);
                // VFS v2 (idl/vfs.wit, encoded by hand): an application's client reads only. It opens the RAM disk's
                // root, but creating a file there is denied (status 2, error case 4: denied).
                let page = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                let shared = call(mb, abi::SYSCALL_MEM_SHARE, page, 0);
                let put = |at: usize, bytes: &[u8]| for (i, &c) in bytes.iter().enumerate() { core::ptr::write_volatile((page as *mut u8).add(at + i), c); };
                put(0, &[3, 0]); put(2, b"ram"); // root(name: string<16>)
                (*mb).msg = [shared, 0, 1 | 2 << 8 | 5 << 16, 0];
                let opened = call(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_VFS, 0) == 0 && (*mb).msg[2] == 4 << 16;
                let root = core::ptr::read_volatile(page as *const [u8; 4]);
                put(0, &root); put(4, &[5, 0]); put(6, b"x.txt"); put(11, &[1 | 2]); // open(dir, path, mode: write | create)
                (*mb).msg = [shared, 0, 3 | 2 << 8 | 12 << 16, 0];
                let denied = call(mb, abi::SYSCALL_IPC_CALL, abi::SLOT_VFS, 0) == 0 && (*mb).msg[2] & 0xFF == 2 && (*mb).msg[3] == 4;
                if !opened || !denied { fail(); }
                call(mb, abi::SYSCALL_CAP_DROP, shared, 0); call(mb, abi::SYSCALL_FREE, page, 0);
                // Derivation: a mint is never wider than its source; revoking a capability removes its descendants only.
                let mint = |handle: usize, mask: usize, offset: usize, length: usize| { (*mb).msg = [offset, length, 0, 0]; call(mb, abi::SYSCALL_CAP_MINT, handle, mask) };
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
                // Revocation reaches a grandchild whose parent was dropped; a mapping cannot be re-shared as a new root.
                let child = mint(memory, abi::CAP_READ as usize, 0, 0);
                let grandchild = mint(child, abi::CAP_READ as usize, 0, 0);
                call(mb, abi::SYSCALL_CAP_DROP, child, 0);
                let view = call(mb, abi::SYSCALL_MEM_MAP, half, 0);
                if call(mb, abi::SYSCALL_MEM_SHARE, view, 0) != abi::ERR_INVALID || call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0) != 2
                    || call(mb, abi::SYSCALL_CAP_INFO, grandchild, 0) != abi::CAP_KIND_NONE || call(mb, abi::SYSCALL_CAP_INFO, half, 0) != abi::CAP_KIND_NONE {
                    asm!("ud2", options(noreturn));
                }
                call(mb, abi::SYSCALL_CAP_DROP, memory, 0); call(mb, abi::SYSCALL_FREE, pages, 0);
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
                let timeout = 30 << abi::IPC_TIMEOUT_SHIFT; // in arg2 above the receive slot (issue 172)
                let start = call(mb, abi::SYSCALL_UPTIME, 0, 0);
                let raw = mb; (*raw).msg = [0, 0, 7, 7];
                if call(mb, abi::SYSCALL_IPC_SEND, endpoints[1], timeout) != abi::ERR_TIMEOUT
                    || call(mb, abi::SYSCALL_IPC_CALL, endpoints[1], timeout) != abi::ERR_TIMEOUT
                    || call(mb, abi::SYSCALL_IPC_RECV, endpoints[1], timeout) != abi::ERR_TIMEOUT
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
                // Issue 171: the table grows past its first 96 slots. 300 read-only children of one shared page get slots
                // beyond 255 (the old handle encoding's end), a revoke removes them all, and a reused slot's old handle fails.
                let block = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                let memory = call(mb, abi::SYSCALL_MEM_SHARE, block, 0);
                let (mut highest, mut last) = (0, 0);
                for _ in 0..300 {
                    (*mb).msg = [0; 4];
                    let child = call(mb, abi::SYSCALL_CAP_MINT, memory, abi::CAP_READ as usize);
                    if child & abi::HANDLE_SLOT_MASK < abi::SLOT_DYNAMIC || call(mb, abi::SYSCALL_CAP_INFO, child, 0) != abi::CAP_KIND_MEMORY { asm!("ud2", options(noreturn)); }
                    highest = highest.max(child & abi::HANDLE_SLOT_MASK); last = child;
                }
                if highest <= 255 || call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0) != 300 || call(mb, abi::SYSCALL_CAP_INFO, last, 0) != abi::CAP_KIND_NONE { asm!("ud2", options(noreturn)); }
                (*mb).msg = [0; 4];
                let again = call(mb, abi::SYSCALL_CAP_MINT, memory, abi::CAP_READ as usize);
                if again == last || call(mb, abi::SYSCALL_CAP_INFO, last, 0) != abi::CAP_KIND_NONE { asm!("ud2", options(noreturn)); }
                call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0); call(mb, abi::SYSCALL_CAP_DROP, memory, 0); call(mb, abi::SYSCALL_FREE, block, 0);
                print(mb, b"CAPABILITY CHECKS OK\r\n");
                return;
            }
            b'f' => {
                // The child's first call stays queued; we exit without receiving, so it must fail with ERR_PEER (MC-6.4).
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                spawn_child(mb, endpoint);
                sleep(mb, 500);
                print(mb, b"PARENT EXITS\r\n");
                return;
            }
            b'q' => {
                // Senders wait in order (MC-2.5, MC-5.2, 000-KRN-0010): five children, more than the former bound of
                // four, are let go one after another while we do not receive; none is refused, and their messages come
                // in the order they were sent.
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                for _ in 0..5 { spawn_child(mb, endpoint); }
                let mut replies = [0usize; 5];
                for reply in replies.iter_mut() {
                    if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 { fail(); }
                    *reply = call(mb, abi::SYSCALL_IPC_SAVE_REPLY, 0, 0);
                }
                for (number, reply) in replies.into_iter().enumerate() {
                    if ipc(mb, abi::SYSCALL_IPC_REPLY, reply, 0, [0, 0, 1, number]) != 0 { fail(); }
                    sleep(mb, 400); // the child sends and waits before the next is let go
                }
                for number in 0..5 {
                    if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 1 || (*mb).msg[3] != number { fail(); }
                }
                print(mb, b"QUEUE ORDER OK\r\n");
                return;
            }
            b'j' => {
                // A reply after the caller's timeout fails with ERR_PEER.
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                spawn_child(mb, endpoint);
                greet(mb, endpoint, 2, 0, 0);
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 3 { fail(); }
                sleep(mb, 400);
                if ipc(mb, abi::SYSCALL_IPC_REPLY, 0, 0, [0, 0, 9, 0]) != abi::ERR_PEER { fail(); }
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 4 { fail(); }
                print(mb, b"LATE REPLY OK\r\n");
                return;
            }
            b'z' => {
                // A memory object moves to the child: our handle dies, the child reads the contents.
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                let block = call(mb, abi::SYSCALL_ALLOC, 4096, 0);
                core::ptr::write_volatile(block as *mut usize, 0x0B1EC7);
                let object = call(mb, abi::SYSCALL_MEM_DETACH, block, 0);
                // Mapped, it does not move: our mapping would be a second writer (issue 167). We keep it.
                let mapped = call(mb, abi::SYSCALL_MEM_MAP, object, 0);
                if mapped >= abi::ERR_FIRST { fail(); }
                spawn_child(mb, endpoint);
                greet(mb, endpoint, 7, object, abi::CAP_TRANSFER_MOVE);
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 100 { fail(); }
                if call(mb, abi::SYSCALL_CAP_INFO, object, 0) != abi::CAP_KIND_MEMORY { fail(); }
                // Unmapped, it moves.
                if call(mb, abi::SYSCALL_FREE, mapped, 0) != 0 { fail(); }
                spawn_child(mb, endpoint);
                greet(mb, endpoint, 3, object, abi::CAP_TRANSFER_MOVE);
                if call(mb, abi::SYSCALL_CAP_INFO, object, 0) != abi::CAP_KIND_NONE { fail(); }
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 0x0B1EC7 { fail(); }
                print(mb, b"MOVE OK\r\n");
                return;
            }
            b'i' => {
                // Badges: the child holds a capability we labelled 0x42; its call arrives with that badge. A badge is
                // set once: a different one is refused, a child of the badged capability keeps it.
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                let badged = ipc(mb, abi::SYSCALL_CAP_MINT, endpoint, (abi::CAP_WRITE | abi::CAP_GRANT) as usize, [0, 0, 0x42, 0]);
                let again = ipc(mb, abi::SYSCALL_CAP_MINT, badged, abi::CAP_WRITE as usize, [0, 0, 0x43, 0]);
                let kept = ipc(mb, abi::SYSCALL_CAP_MINT, badged, abi::CAP_WRITE as usize, [0; 4]);
                let badge = |handle: usize| { call(mb, abi::SYSCALL_CAP_INFO, handle, 0); (*mb).arg2 };
                if again != abi::ERR_INVALID || badge(kept) != 0x42 || badge(endpoint) != 0 { fail(); }
                spawn_child(mb, badged);
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).arg2 != 0x42 { fail(); }
                if ipc(mb, abi::SYSCALL_IPC_REPLY, 0, 0, [0, 0, 6, 0]) != 0 { fail(); }
                print(mb, b"BADGE OK\r\n");
                return;
            }
            b'b' => {
                // Revoking our endpoint removes the child's copy and the copy waiting in its blocked send (through
                // loader's dropped copy, a ghost node).
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                spawn_child(mb, endpoint);
                greet(mb, endpoint, 4, 0, 0);
                sleep(mb, 300);
                if call(mb, abi::SYSCALL_CAP_REVOKE, endpoint, 0) != 2 { fail(); }
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 | 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 5 || (*mb).msg[0] != 0 { fail(); }
                print(mb, b"REVOKE PENDING OK\r\n");
                return;
            }
            b'x' => {
                // A lease mapped by a child that keeps reading it ends at revoke: the child faults.
                let memory = call(mb, abi::SYSCALL_MEM_SHARE, call(mb, abi::SYSCALL_ALLOC, 4096, 0), 0);
                let endpoint = call(mb, abi::SYSCALL_ENDPOINT_CREATE, 0, 0);
                spawn_child(mb, endpoint);
                greet(mb, endpoint, 5, memory, 0);
                if ipc(mb, abi::SYSCALL_IPC_RECV, endpoint, 3 * SECONDS, [0; 4]) != 0 || (*mb).msg[2] != 6 { fail(); }
                sleep(mb, 100);
                if call(mb, abi::SYSCALL_CAP_REVOKE, memory, 0) != 1 { fail(); }
                sleep(mb, 300);
                print(mb, b"LEASE REVOKED\r\n");
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
