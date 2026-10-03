#![no_std]
#![no_main]
use core::panic::PanicInfo;
use libmind::abi::{BootInfo, SyscallMailbox, CAP_WRITE, CAP_GRANT};
use libmind::sys;
#[path = "../../common/font.rs"] mod font;
use font::FONT;

fn draw_char(fb: *mut u32, stride: usize, px: usize, py: usize, ascii: u8, color: u32) { let idx = if ascii >= 32 && ascii <= 95 { (ascii - 32) as usize } else if ascii >= 97 && ascii <= 122 { (ascii - 97 + 33) as usize } else { 0 }; let bitmap = FONT[idx]; for row in 0..8 { let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF; for col in 0..8 { if (row_data & (1 << (7 - col))) != 0 { unsafe { core::ptr::write_volatile(fb.add((py + row) * stride + (px + col)), color); } } } } }
fn draw_string(fb: *mut u32, stride: usize, start_x: usize, start_y: usize, text: &[u8], color: u32) { let mut cx = start_x; for &b in text { draw_char(fb, stride, cx, start_y, b, color); cx += 8; } }
fn usize_to_str(mut val: usize, buf: &mut [u8]) -> usize { if val == 0 { buf[0] = b'0'; return 1; } let mut idx = 0; let mut temp = [0u8; 30]; while val > 0 && idx < 30 { temp[idx] = b'0' + (val % 10) as u8; val /= 10; idx += 1; } for i in 0..idx { buf[i] = temp[idx - 1 - i]; } idx }

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mb: *mut SyscallMailbox) -> () {
    let total_pixels = info.height * info.stride;
    for i in 0..total_pixels { unsafe { core::ptr::write_volatile(info.fb_ptr.add(i), 0x00111111); } }
    draw_string(info.fb_ptr, info.stride, 40, 40, b"[ PONG / SUPERVISOR ]", 0x0000FF00);

    let my_ep_slot = sys::endpoint_create(mb).expect("EP CREATE FAILED");
    
    let mut n_buf = [0u8; 30]; 
    draw_string(info.fb_ptr, info.stride, 40, 70, b"CREATED EP SLOT:", 0x00AAAAAA);
    let n_len = usize_to_str(my_ep_slot, &mut n_buf);
    draw_string(info.fb_ptr, info.stride, 200, 70, &n_buf[0..n_len], 0x00FFFF00);

    sys::spawn(mb, b"ping", my_ep_slot, CAP_WRITE | CAP_GRANT).expect("SPAWN FAILED");

    loop {
        for y in 100..230 { for x in 40..600 { unsafe { core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), 0x00111111); } } }

        draw_string(info.fb_ptr, info.stride, 40, 100, b"WAITING FOR CAPS...", 0x00888888);

        if sys::ipc_recv(mb, my_ep_slot, 5).is_err() || sys::ipc_recv(mb, my_ep_slot, 6).is_err() {
            draw_string(info.fb_ptr, info.stride, 40, 130, b"RECV ERROR", 0xFF0000);
            sys::check_keys_and_wait(mb, 1000); continue;
        }

        draw_string(info.fb_ptr, info.stride, 40, 130, b"MAPPING SHARED MEMORY...", 0x00AAAAAA);
        
        let mapped_vaddr = sys::mem_map(mb, 6).expect("MEM MAP FAILED");

        let mut len = 0;
        unsafe { while core::ptr::read_volatile((mapped_vaddr + len) as *const u8) != 0 { len += 1; } }
        let shared_str = unsafe { core::slice::from_raw_parts(mapped_vaddr as *const u8, len) };

        draw_string(info.fb_ptr, info.stride, 40, 160, b"READ FROM SHARED RAM:", 0xFF00FF);
        draw_string(info.fb_ptr, info.stride, 40, 190, shared_str, 0x00FFFF00);

        sys::mem_free(mb, mapped_vaddr).expect("FREE FAILED");
        sys::ipc_send(mb, 5, 0, 0, 0).expect("REPLY FAILED");

        sys::check_keys_and_wait(mb, 1000); 
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
