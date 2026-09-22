#![no_std]
#![no_main]
use core::panic::PanicInfo;
use core::arch::asm;
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, SyscallMailbox, SYSCALL_PORT_IN, SYSCALL_IRQ_WAIT, SYSCALL_INPUT_EVENT};

fn port_in(mb: *mut SyscallMailbox, cap_slot: usize, port: u16) -> u8 {
    unsafe {
        core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_PORT_IN);
        core::ptr::write_volatile(&mut (*mb).arg1, cap_slot);
        core::ptr::write_volatile(&mut (*mb).arg2, port as usize);
        asm!("int 0x80", options(nostack));
        core::ptr::read_volatile(&(*mb).result) as u8
    }
}

fn wait_irq(mb: *mut SyscallMailbox, cap_slot: usize) {
    unsafe {
        core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IRQ_WAIT);
        core::ptr::write_volatile(&mut (*mb).arg1, cap_slot);
        asm!("int 0x80", options(nostack));
    }
}

fn send_input_event(mb: *mut SyscallMailbox, app_code: u8, shell_code: u8, is_bg: bool) {
    unsafe {
        core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_INPUT_EVENT);
        core::ptr::write_volatile(&mut (*mb).arg1, app_code as usize);
        core::ptr::write_volatile(&mut (*mb).arg2, shell_code as usize);
        core::ptr::write_volatile(&mut (*mb).msg[0], if is_bg { 1 } else { 0 });
        asm!("int 0x80", options(nostack));
    }
}

struct KbdState {
    shift: bool,
    control: bool,
    extended: bool,
}

impl KbdState {
    const fn new() -> Self {
        Self { shift: false, control: false, extended: false }
    }

    fn process(&mut self, code: u8) -> (u8, u8, bool) {
        if code == 0xe0 || code == 0xe1 {
            self.extended = true;
            return (0, 0, false);
        }
        if code & 0x7f == 0x1d {
            self.control = code & 0x80 == 0;
            self.extended = false;
            return (0, 0, false);
        }
        if self.extended {
            self.extended = false;
            return (0, 0, false);
        }
        if code & 0x7f == 0x2a || code & 0x7f == 0x36 {
            self.shift = code & 0x80 == 0;
            return (0, 0, false);
        }
        if code & 0x80 != 0 {
            return (0, 0, false);
        }
        if self.control && code == 0x2c { // Ctrl+Z
            return (0, 0, true);
        }
        const NORMAL: &[u8] = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
        const SHIFT: &[u8] = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";
        let shell = if self.shift { SHIFT } else { NORMAL }.get(code as usize).copied().unwrap_or(0);
        (code, shell, false)
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_info: &BootInfo, mb_ptr: *mut SyscallMailbox) -> () {
    let port60_cap = 1;
    let port64_cap = 2;
    let irq1_cap = 3;
    let mut state = KbdState::new();

    loop {
        wait_irq(mb_ptr, irq1_cap);

        let status = port_in(mb_ptr, port64_cap, 0x64);
        if status & 1 != 0 {
            let scancode = port_in(mb_ptr, port60_cap, 0x60);
            if status & 0x20 == 0 {
                let (app, shell, bg) = state.process(scancode);
                if app != 0 || shell != 0 || bg {
                    send_input_event(mb_ptr, app, shell, bg);
                }
            }
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
