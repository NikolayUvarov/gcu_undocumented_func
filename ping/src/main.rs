#![no_std]
#![no_main]
use core::panic::PanicInfo;
use libmind::abi::{BootInfo, SyscallMailbox, CAP_WRITE};
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
    draw_string(info.fb_ptr, info.stride, 40, 40, b"[ PING / CLIENT ]", 0x00FF0000);

    let shared_vaddr = sys::mem_alloc(mb, 4096).expect("ALLOC FAILED");
    let mem_cap_slot = sys::mem_share(mb, shared_vaddr, 4096).expect("MEM SHARE FAILED");
    let reply_ep_slot = sys::endpoint_create(mb).expect("EP CREATE FAILED");

    let server_ep_slot = 1;
    let mut counter = 1000;
    let mut n_buf = [0u8; 30];

    loop {
        for y in 70..200 { for x in 40..400 { unsafe { core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), 0x00111111); } } }

        draw_string(info.fb_ptr, info.stride, 40, 70, b"PREPARING SHARED MEMORY...", 0x00AAAAAA);
        
        let msg = b"HELLO FROM PING! ZERO-COPY IPC SUCCESS! COUNT: ";
        unsafe { core::ptr::copy_nonoverlapping(msg.as_ptr(), shared_vaddr as *mut u8, msg.len()); }
        let n_len = usize_to_str(counter, &mut n_buf);
        unsafe { core::ptr::copy_nonoverlapping(n_buf.as_ptr(), (shared_vaddr + msg.len()) as *mut u8, n_len); }
        unsafe { core::ptr::write_volatile((shared_vaddr + msg.len() + n_len) as *mut u8, 0); }

        draw_string(info.fb_ptr, info.stride, 40, 100, b"SENDING CAPS TO SERVER", 0x00FFFFFF);
        if sys::ipc_send(mb, server_ep_slot, reply_ep_slot, CAP_WRITE, 0).is_err() {
            draw_string(info.fb_ptr, info.stride, 40, 130, b"SEND ERROR", 0xFF0000);
            sys::check_keys_and_wait(mb, 1000); continue;
        }
        if sys::ipc_send(mb, server_ep_slot, mem_cap_slot, 0, 0).is_err() {
            draw_string(info.fb_ptr, info.stride, 40, 130, b"SEND ERROR", 0xFF0000);
            sys::check_keys_and_wait(mb, 1000); continue;
        }

        if sys::ipc_recv(mb, reply_ep_slot, 0).is_err() {
            draw_string(info.fb_ptr, info.stride, 40, 160, b"RECV ERROR", 0xFF0000);
        } else {
            draw_string(info.fb_ptr, info.stride, 40, 160, b"SERVER CONFIRMED RECEIPT!", 0x00FF00);
        }

        counter += 1;
        sys::check_keys_and_wait(mb, 1500); 
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
