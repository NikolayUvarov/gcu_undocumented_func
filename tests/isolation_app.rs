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
                // Обычное приложение не может пользоваться чужими привилегиями без мандатов.
                let image = _start as *const () as usize;
                let checks = [
                    (abi::SYSCALL_INPUT_EVENT, b'x' as usize, b'x' as usize, abi::ERR_RIGHTS),
                    (abi::SYSCALL_COMPOSITOR_PULL, 9, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, abi::SLOT_RTC, 0x70, abi::ERR_RIGHTS),
                    (abi::SYSCALL_PORT_IN, 31, 0x60, abi::ERR_RIGHTS),
                    (abi::SYSCALL_IRQ_WAIT, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_MAP, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_PHYS, abi::SLOT_RTC, 0, abi::ERR_RIGHTS),
                    (abi::SYSCALL_MEM_SHARE, image, 4096, abi::ERR_INVALID), // код не делится
                    (abi::SYSCALL_MEM_SHARE, 0x80_0100_1000, 4096, abi::ERR_INVALID), // стек тоже
                    (abi::SYSCALL_IPC_RECV, abi::SLOT_RTC, 0, abi::ERR_RIGHTS), // только запись в чужой сервис
                    (abi::SYSCALL_IPC_REPLY, 0, 0, abi::ERR_INVALID),
                ];
                for (number, a, b, expected) in checks {
                    if call(mb, number, a, b) != expected {
                        asm!("ud2", options(noreturn));
                    }
                }
                let block = call(mb, abi::SYSCALL_ALLOC, 8192, 0);
                if block == 0 || call(mb, abi::SYSCALL_MEM_SHARE, block + 4096, 4096) != abi::ERR_INVALID || call(mb, abi::SYSCALL_MEM_SHARE, block, 3 * 4096) != abi::ERR_INVALID {
                    asm!("ud2", options(noreturn));
                }
                let slot = call(mb, abi::SYSCALL_MEM_SHARE, block, 0);
                if slot < abi::SLOT_DYNAMIC || call(mb, abi::SYSCALL_FREE, block, 0) != 0 || call(mb, abi::SYSCALL_CAP_DROP, slot, 0) != 0 {
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
