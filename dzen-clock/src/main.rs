#![no_std]
#![no_main]

use core::{arch::asm, ffi::c_void, panic::PanicInfo};
#[path = "../../common/abi.rs"]
mod abi;
mod cycle;
mod face;
#[path = "../../common/font.rs"]
mod font;
mod view;
#[path = "../../common/wait.rs"]
mod wait;
use abi::{BootInfo, SyscallMailbox, SYSCALL_RTC_TIME, SYSCALL_UPTIME};
use cycle::{point_at, Cycle, OrbitMode};
use face::{time_text, Face};
use view::View;

fn os_read(mailbox: *mut SyscallMailbox, number: usize) -> usize {
    unsafe {
        (*mailbox).syscall_num = number;
        asm!("int 0x80");
        (*mailbox).result
    }
}

fn print(mailbox: *mut SyscallMailbox, text: &[u8]) {
    unsafe {
        (*mailbox).syscall_num = 3;
        (*mailbox).arg1 = text.as_ptr() as usize;
        (*mailbox).arg2 = text.len();
        asm!("int 0x80");
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mailbox: *mut SyscallMailbox) {
    print(
        mailbox,
        b"\r\n[DZEN-CLOCK] STARTED. D: DIGITS, C: ORBIT, P: 10S TICKS, H: TEXT, CTRL+Z: SHELL, ESC: EXIT.\r\n",
    );
    let view = View::new(info);
    view.clear();
    view.hints(true);
    view.face(Face::DARK);
    view.digital(b"--:--:--", true);
    let mut previous_face = Face::DARK;
    let mut previous_time = None;
    let mut show_digits = true;
    let mut show_hints = true;
    let mut orbit_mode = OrbitMode::Off;
    let mut previous_mode = OrbitMode::Off;
    let mut cycle = Cycle::new();
    let mut previous_dot = None;
    let mut waiting = false;
    loop {
        let key = os_read(mailbox, 2);
        if key == 0x01 || key == 0x1B {
            print(mailbox, b"[DZEN-CLOCK] RETURNING TO KERNEL.\r\n");
            return;
        }
        // PS/2 D make code, or the UART's ASCII D/d (the shared input ABI).
        if key == 0x20 || key == b'd' as usize || key == b'D' as usize {
            show_digits = !show_digits;
            let text = previous_time.map(time_text).unwrap_or(*b"--:--:--");
            view.digital(&text, show_digits);
            print(
                mailbox,
                if show_digits {
                    b"[DZEN-CLOCK] DIGITS ON\r\n"
                } else {
                    b"[DZEN-CLOCK] DIGITS OFF\r\n"
                },
            );
        }
        // PS/2 H make code, or UART ASCII H/h.
        if key == 0x23 || key == b'h' as usize || key == b'H' as usize {
            show_hints = !show_hints;
            view.hints(show_hints);
            print(
                mailbox,
                if show_hints {
                    b"[DZEN-CLOCK] TEXT ON\r\n"
                } else {
                    b"[DZEN-CLOCK] TEXT OFF\r\n"
                },
            );
        }
        // PS/2 C/P make codes, or UART ASCII C/c/P/p.
        let requested = match key {
            0x2E | 0x63 | 0x43 => Some(OrbitMode::Simple),
            0x19 | 0x70 | 0x50 => Some(OrbitMode::Ticks),
            _ => None,
        };
        if let Some(requested) = requested {
            orbit_mode = orbit_mode.toggle(requested);
            print(
                mailbox,
                match orbit_mode {
                    OrbitMode::Off => b"[DZEN-CLOCK] ORBIT OFF\r\n",
                    OrbitMode::Simple => b"[DZEN-CLOCK] ORBIT SIMPLE\r\n",
                    OrbitMode::Ticks => b"[DZEN-CLOCK] ORBIT 10S TICKS\r\n",
                },
            );
        }
        let seconds = os_read(mailbox, SYSCALL_RTC_TIME);
        let now = os_read(mailbox, SYSCALL_UPTIME);
        cycle.observe(seconds, now);
        if let Some(current) = Face::at(seconds) {
            if current != previous_face {
                view.face(current);
                previous_face = current;
                print(mailbox, b"[DZEN-CLOCK] ");
                print(mailbox, &time_text(seconds));
                print(mailbox, b"\r\n");
            }
            if previous_time != Some(seconds) && show_digits {
                view.digital(&time_text(seconds), true);
            }
            previous_time = Some(seconds);
            waiting = false;
        } else if previous_time.is_none() && !waiting {
            // An RTC update in progress is transient: retain the last valid face.
            print(mailbox, b"[DZEN-CLOCK] WAITING FOR RTC...\r\n");
            waiting = true;
        }
        let dot = if orbit_mode != OrbitMode::Off {
            cycle
                .phase(now)
                .map(|phase| point_at(phase, view.half() * 3 / 4))
        } else {
            None
        };
        if dot != previous_dot || orbit_mode != previous_mode {
            view.cycle(previous_dot, dot, previous_mode, orbit_mode);
            previous_dot = dot;
            previous_mode = orbit_mode;
        }
        // Sleep instead of polling continuously, including when digits are hidden.
        wait::wait(mailbox, 100);
    }
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut c_void, value: i32, len: usize) -> *mut c_void {
    for i in 0..len {
        core::ptr::write_volatile((dest as *mut u8).add(i), value as u8);
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    for i in 0..len {
        core::ptr::write_volatile(
            (dest as *mut u8).add(i),
            core::ptr::read_volatile((src as *const u8).add(i)),
        );
    }
    dest
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
