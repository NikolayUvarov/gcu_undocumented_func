#![no_std]
#![no_main]

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;

#[path = "../../common/abi.rs"]
mod abi;
use abi::{BootInfo, SyscallMailbox, RTC_UNAVAILABLE, SYSCALL_RTC_TIME};
#[path = "../../common/font.rs"]
mod font;
use font::FONT;

const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;
// Only the polling interval uses TSC. Displayed time always comes from the RTC.
const POLL_INTERVAL_TSC: usize = 25_000_000;

fn os_read(mailbox: *mut SyscallMailbox, syscall_num: usize) -> usize {
    unsafe {
        (*mailbox).syscall_num = syscall_num;
        asm!("int 0x80");
        (*mailbox).result
    }
}

fn os_print(mailbox: *mut SyscallMailbox, text: &[u8]) {
    unsafe {
        (*mailbox).syscall_num = 3;
        (*mailbox).arg1 = text.as_ptr() as usize;
        (*mailbox).arg2 = text.len();
        asm!("int 0x80");
    }
}

fn draw_text(info: &BootInfo, x: usize, y: usize, text: &[u8], scale: usize, color: u32) {
    for (index, &ch) in text.iter().enumerate() {
        let glyph = FONT[ch.saturating_sub(32).min(63) as usize];
        for row in 0..8 {
            for col in 0..8 {
                let pixel = if glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0 {
                    color
                } else {
                    BACKGROUND
                };
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x + (index * 8 + col) * scale + dx;
                        let py = y + row * scale + dy;
                        if px < info.width && py < info.height {
                            unsafe {
                                core::ptr::write_volatile(
                                    info.fb_ptr.add(py * info.stride + px),
                                    pixel,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn time_text(seconds: usize) -> [u8; 8] {
    let hour = seconds / 3600;
    let minute = (seconds / 60) % 60;
    let second = seconds % 60;
    [
        b'0' + (hour / 10) as u8,
        b'0' + (hour % 10) as u8,
        b':',
        b'0' + (minute / 10) as u8,
        b'0' + (minute % 10) as u8,
        b':',
        b'0' + (second / 10) as u8,
        b'0' + (second % 10) as u8,
    ]
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mailbox: *mut SyscallMailbox) {
    os_print(mailbox, b"\r\n[CLOCK] STARTED. PRESS ESC TO EXIT.\r\n");
    for y in 0..info.height {
        for x in 0..info.width {
            unsafe {
                core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), BACKGROUND);
            }
        }
    }
    draw_text(info, 24, 24, b"CLOCK", 2, FOREGROUND);
    draw_text(info, 24, 56, b"ESC: RETURN TO SHELL", 1, 0x00FFFFFF);

    let scale = (info.width / 80).min(info.height / 32).clamp(1, 8);
    let x = info.width.saturating_sub(64 * scale) / 2;
    let y = info.height.saturating_sub(8 * scale) / 2;
    draw_text(info, x, y, b"--:--:--", scale, FOREGROUND);
    let mut previous_time = None;
    let mut last_poll = os_read(mailbox, 1).wrapping_sub(POLL_INTERVAL_TSC);

    loop {
        let key = os_read(mailbox, 2);
        if key == 0x01 || key == 0x1B {
            os_print(mailbox, b"[CLOCK] RETURNING TO KERNEL.\r\n");
            return;
        }
        let ticks = os_read(mailbox, 1);
        if ticks.wrapping_sub(last_poll) < POLL_INTERVAL_TSC {
            core::hint::spin_loop();
            continue;
        }
        last_poll = ticks;

        let seconds = os_read(mailbox, SYSCALL_RTC_TIME);
        // An RTC update in progress is transient: keep the last valid display.
        if seconds == RTC_UNAVAILABLE || seconds >= 24 * 3600 {
            if previous_time.is_none() {
                os_print(mailbox, b"[CLOCK] WAITING FOR RTC...\r\n");
                previous_time = Some(RTC_UNAVAILABLE);
            }
            continue;
        }
        if previous_time != Some(seconds) {
            let text = time_text(seconds);
            draw_text(info, x, y, &text, scale, FOREGROUND);
            os_print(mailbox, b"[CLOCK] ");
            os_print(mailbox, &text);
            os_print(mailbox, b"\r\n");
            previous_time = Some(seconds);
        }
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
