#![no_std]
#![no_main]

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;

#[path = "../../common/abi.rs"]
mod abi;
use abi::{BootInfo, SyscallMailbox};
#[path = "../../common/font.rs"]
mod font;
use font::FONT;

const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;
// TSC pacing for this demo; actual frame duration depends on the CPU frequency.
const FRAME_INTERVAL_TSC: usize = 50_000_000;

fn os_print(mb: *mut SyscallMailbox, message: &[u8]) {
    unsafe {
        (*mb).syscall_num = 3;
        (*mb).arg1 = message.as_ptr() as usize;
        (*mb).arg2 = message.len();
        asm!("int 0x80");
    }
}

fn os_read(mb: *mut SyscallMailbox, syscall_num: usize) -> usize {
    unsafe {
        (*mb).syscall_num = syscall_num;
        asm!("int 0x80");
        (*mb).result
    }
}

fn decimal(mut value: usize, buffer: &mut [u8; 20]) -> &[u8] {
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return &buffer[start..];
        }
    }
}

fn fill_rect(info: &BootInfo, x: usize, y: usize, width: usize, height: usize, color: u32) {
    for py in y..y.saturating_add(height).min(info.height) {
        for px in x..x.saturating_add(width).min(info.width) {
            unsafe { core::ptr::write_volatile(info.fb_ptr.add(py * info.stride + px), color) };
        }
    }
}

fn draw_text(info: &BootInfo, x: usize, y: usize, text: &[u8], color: u32) {
    for (index, &ch) in text.iter().enumerate() {
        let glyph = FONT[ch.to_ascii_uppercase().saturating_sub(32).min(63) as usize];
        for row in 0..8 {
            for col in 0..8 {
                let px = x + index * 8 + col;
                let py = y + row;
                if px < info.width
                    && py < info.height
                    && glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0
                {
                    unsafe {
                        core::ptr::write_volatile(info.fb_ptr.add(py * info.stride + px), color)
                    };
                }
            }
        }
    }
}

fn bounce(frame: usize, limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    let phase = frame % (limit * 2);
    if phase <= limit {
        phase
    } else {
        limit * 2 - phase
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mailbox: *mut SyscallMailbox) {
    os_print(
        mailbox,
        b"\r\n[APP2] HELLO FROM THE SECOND ELF PROGRAM!\r\n",
    );
    os_print(
        mailbox,
        b"[APP2] BOUNCING SQUARE AND FRAME COUNTER. PRESS ESC TO EXIT.\r\n",
    );

    // All state is local so every RUN APP2 starts a fresh animation and counter.
    let mut frame: usize = 0;
    let mut last_frame_ticks = os_read(mailbox, 1).wrapping_sub(FRAME_INTERVAL_TSC);
    let top = 120.min(info.height);
    let size = 64
        .min(info.width.saturating_sub(48))
        .min(info.height.saturating_sub(top + 24));
    let travel_x = info.width.saturating_sub(size + 48);
    let travel_y = info.height.saturating_sub(top + size + 24);

    loop {
        // The existing input syscall returns either a PS/2 scancode or a UART byte.
        let key = os_read(mailbox, 2) as u8;
        if key == 0x01 || key == 0x1B {
            os_print(mailbox, b"[APP2] ESC PRESSED. RETURNING TO KERNEL.\r\n");
            return;
        }

        let ticks = os_read(mailbox, 1);
        if ticks.wrapping_sub(last_frame_ticks) < FRAME_INTERVAL_TSC {
            core::hint::spin_loop();
            continue;
        }
        last_frame_ticks = ticks;
        let mut frame_buffer = [0u8; 20];
        let frame_text = decimal(frame, &mut frame_buffer);
        let mut tick_buffer = [0u8; 20];
        let tick_text = decimal(ticks, &mut tick_buffer);

        fill_rect(info, 0, 0, info.width, info.height, BACKGROUND);
        draw_text(info, 24, 24, b"SECOND APP (ELF) - RUN APP2", FOREGROUND);
        draw_text(info, 24, 48, b"ESC: RETURN TO SHELL", 0x00FFFFFF);
        draw_text(info, 24, 72, b"FRAMES:", FOREGROUND);
        draw_text(info, 96, 72, frame_text, 0x00FFFFFF);
        draw_text(info, 24, 96, b"TSC:", FOREGROUND);
        draw_text(info, 96, 96, tick_text, 0x00FFFFFF);
        fill_rect(
            info,
            24 + bounce(frame, travel_x),
            top + bounce(frame, travel_y),
            size,
            size,
            FOREGROUND,
        );

        // Log periodically instead of flooding the UART on every redraw.
        if frame % 30 == 0 {
            os_print(mailbox, b"[APP2] FRAME=");
            os_print(mailbox, frame_text);
            os_print(mailbox, b" TSC=");
            os_print(mailbox, tick_text);
            os_print(mailbox, b"\r\n");
        }

        frame = frame.wrapping_add(1);
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
