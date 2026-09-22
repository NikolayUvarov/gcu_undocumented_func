#![no_std]
#![no_main]
use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, SyscallMailbox, RTC_UNAVAILABLE, SYSCALL_ENDPOINT_CREATE, SYSCALL_IPC_SEND, SYSCALL_IPC_RECV, SYSCALL_WAIT};
#[path = "../../common/font.rs"] mod font;
use font::FONT;

const BACKGROUND: u32 = 0x001E1E2E; const FOREGROUND: u32 = 0x00A6E3A1;
fn draw_text(info: &BootInfo, x: usize, y: usize, text: &[u8], scale: usize, color: u32) { for (index, &ch) in text.iter().enumerate() { let glyph = FONT[ch.saturating_sub(32).min(63) as usize]; for row in 0..8 { for col in 0..8 { let pixel = if glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0 { color } else { BACKGROUND }; for dy in 0..scale { for dx in 0..scale { let px = x + (index * 8 + col) * scale + dx; let py = y + row * scale + dy; if px < info.width && py < info.height { unsafe { core::ptr::write_volatile(info.fb_ptr.add(py * info.stride + px), pixel); } } } } } } } }
fn time_text(seconds: usize) -> [u8; 8] { let hour = seconds / 3600; let minute = (seconds / 60) % 60; let second = seconds % 60; [ b'0' + (hour / 10) as u8, b'0' + (hour % 10) as u8, b':', b'0' + (minute / 10) as u8, b'0' + (minute % 10) as u8, b':', b'0' + (second / 10) as u8, b'0' + (second % 10) as u8 ] }
fn wait_and_check(mb: *mut SyscallMailbox, ms: usize) -> bool { loop { unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 2); asm!("int 0x80", options(nostack)); } let key = unsafe { core::ptr::read_volatile(&(*mb).result) }; if key == 0 { break; } if key == 0x01 || key == 0x1B { unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 7); asm!("int 0x80", options(nostack)); } return true; } } unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_WAIT); core::ptr::write_volatile(&mut (*mb).arg1, ms); asm!("int 0x80", options(nostack)); } false }

fn get_rtc_time(mb: *mut SyscallMailbox, reply_ep_slot: usize) -> usize {
    let rtc_ep_slot = 2; // Ядро выдает мандат на RTC всем клиентам в слот 2
    loop {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_SEND); core::ptr::write_volatile(&mut (*mb).arg1, rtc_ep_slot); core::ptr::write_volatile(&mut (*mb).arg2, 0); core::ptr::write_volatile(&mut (*mb).msg[0], reply_ep_slot); core::ptr::write_volatile(&mut (*mb).msg[1], abi::CAP_WRITE as usize); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res == 0 { break; }
        wait_and_check(mb, 10);
    }
    unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_RECV); core::ptr::write_volatile(&mut (*mb).arg1, reply_ep_slot); core::ptr::write_volatile(&mut (*mb).arg2, 0); asm!("int 0x80", options(nostack)); }
    unsafe { core::ptr::read_volatile(&(*mb).msg[0]) }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mailbox: *mut SyscallMailbox) {
    for y in 0..info.height { for x in 0..info.width { unsafe { core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), BACKGROUND); } } }
    draw_text(info, 24, 24, b"CLOCK (IPC RTC)", 2, FOREGROUND);
    let scale = (info.width / 80).min(info.height / 32).clamp(1, 8); let x = info.width.saturating_sub(64 * scale) / 2; let y = info.height.saturating_sub(8 * scale) / 2;
    draw_text(info, x, y, b"--:--:--", scale, FOREGROUND);
    
    unsafe { core::ptr::write_volatile(&mut (*mailbox).syscall_num, SYSCALL_ENDPOINT_CREATE); asm!("int 0x80", options(nostack)); }
    let reply_ep_slot = unsafe { core::ptr::read_volatile(&(*mailbox).result) };
    
    let mut previous_time = None;
    loop {
        if wait_and_check(mailbox, 100) { return; }
        let seconds = get_rtc_time(mailbox, reply_ep_slot);
        if seconds == RTC_UNAVAILABLE || seconds >= 24 * 3600 {
            if previous_time.is_none() { previous_time = Some(RTC_UNAVAILABLE); }
            continue;
        }
        if previous_time != Some(seconds) {
            let text = time_text(seconds);
            draw_text(info, x, y, &text, scale, FOREGROUND);
            previous_time = Some(seconds);
        }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
