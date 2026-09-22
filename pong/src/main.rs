#![no_std]
#![no_main]
use core::panic::PanicInfo;
use core::arch::asm;
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, SyscallMailbox, SYSCALL_IPC_SEND, SYSCALL_IPC_RECV, SYSCALL_ENDPOINT_CREATE, SYSCALL_SPAWN, CAP_WRITE, CAP_GRANT, SYSCALL_WAIT, SYSCALL_FREE, SYSCALL_MEM_MAP};
#[path = "../../common/font.rs"] mod font;
use font::FONT;

fn draw_char(fb: *mut u32, stride: usize, px: usize, py: usize, ascii: u8, color: u32) { let idx = if ascii >= 32 && ascii <= 95 { (ascii - 32) as usize } else if ascii >= 97 && ascii <= 122 { (ascii - 97 + 33) as usize } else { 0 }; let bitmap = FONT[idx]; for row in 0..8 { let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF; for col in 0..8 { if (row_data & (1 << (7 - col))) != 0 { unsafe { core::ptr::write_volatile(fb.add((py + row) * stride + (px + col)), color); } } } } }
fn draw_string(fb: *mut u32, stride: usize, start_x: usize, start_y: usize, text: &[u8], color: u32) { let mut cx = start_x; for &b in text { draw_char(fb, stride, cx, start_y, b, color); cx += 8; } }

fn wait_and_check_keys(mb: *mut SyscallMailbox, ms: usize) {
    loop { unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 2); asm!("int 0x80", options(nostack)); } let key = unsafe { core::ptr::read_volatile(&(*mb).result) }; if key == 0 { break; } if key == 0x01 || key == 0x1B { unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 7); asm!("int 0x80", options(nostack)); } } }
    unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_WAIT); core::ptr::write_volatile(&mut (*mb).arg1, ms); asm!("int 0x80", options(nostack)); }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mb_ptr: *mut SyscallMailbox) -> () {
    let total_pixels = info.height * info.stride;
    for i in 0..total_pixels { unsafe { core::ptr::write_volatile(info.fb_ptr.add(i), 0x00111111); } }
    draw_string(info.fb_ptr, info.stride, 40, 40, b"[ PONG / SUPERVISOR ]", 0x0000FF00);

    unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_ENDPOINT_CREATE); asm!("int 0x80", options(nostack)); }
    let my_ep_slot = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) };
    
    let child_name = b"ping";
    unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_SPAWN); core::ptr::write_volatile(&mut (*mb_ptr).arg1, child_name.as_ptr() as usize); core::ptr::write_volatile(&mut (*mb_ptr).arg2, child_name.len()); core::ptr::write_volatile(&mut (*mb_ptr).msg[0], my_ep_slot); core::ptr::write_volatile(&mut (*mb_ptr).msg[1], (CAP_WRITE | CAP_GRANT) as usize); asm!("int 0x80", options(nostack)); }

    loop {
        for y in 70..230 { for x in 40..600 { unsafe { core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), 0x00111111); } } }

        draw_string(info.fb_ptr, info.stride, 40, 70, b"WAITING FOR CAPS...", 0x00888888);

        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_IPC_RECV); core::ptr::write_volatile(&mut (*mb_ptr).arg1, my_ep_slot); core::ptr::write_volatile(&mut (*mb_ptr).arg2, 5); asm!("int 0x80", options(nostack)); }
        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_IPC_RECV); core::ptr::write_volatile(&mut (*mb_ptr).arg1, my_ep_slot); core::ptr::write_volatile(&mut (*mb_ptr).arg2, 6); asm!("int 0x80", options(nostack)); }

        draw_string(info.fb_ptr, info.stride, 40, 100, b"MAPPING SHARED MEMORY...", 0x00AAAAAA);
        
        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_MEM_MAP); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 6); asm!("int 0x80", options(nostack)); }
        let mapped_vaddr = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) };

        let mut len = 0;
        unsafe { while core::ptr::read_volatile((mapped_vaddr + len) as *const u8) != 0 { len += 1; } }
        let shared_str = unsafe { core::slice::from_raw_parts(mapped_vaddr as *const u8, len) };

        draw_string(info.fb_ptr, info.stride, 40, 130, b"READ FROM SHARED RAM:", 0xFF00FF);
        draw_string(info.fb_ptr, info.stride, 40, 160, shared_str, 0x00FFFF00);

        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_FREE); core::ptr::write_volatile(&mut (*mb_ptr).arg1, mapped_vaddr); asm!("int 0x80", options(nostack)); }
        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_IPC_SEND); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 5); core::ptr::write_volatile(&mut (*mb_ptr).arg2, 0); asm!("int 0x80", options(nostack)); }

        wait_and_check_keys(mb_ptr, 1000); 
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
