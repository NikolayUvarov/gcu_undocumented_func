#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;
use core::ffi::c_void;

#[path = "../../common/abi.rs"]
mod abi;
use abi::{BootInfo, SyscallMailbox};
#[path = "../../common/wait.rs"]
mod wait;

fn os_print(mb: *mut SyscallMailbox, msg: &[u8]) {
    unsafe { (*mb).syscall_num = 3; (*mb).arg1 = msg.as_ptr() as usize; (*mb).arg2 = msg.len(); asm!("int 0x80"); }
}
fn get_os_ticks(mb: *mut SyscallMailbox) -> usize {
    unsafe { (*mb).syscall_num = 1; asm!("int 0x80"); (*mb).result }
}
fn get_os_key(mb: *mut SyscallMailbox) -> u8 {
    unsafe { (*mb).syscall_num = 2; asm!("int 0x80"); (*mb).result as u8 }
}

static mut MAIN_SP: u64 = 0;
static mut THREAD_SP: u64 = 0;
static mut WORK_COUNTER: usize = 0;

#[repr(align(16))]
struct ThreadStack { data: [u64; 1024] }
static mut THREAD_STACK: ThreadStack = ThreadStack { data: [0; 1024] };

#[unsafe(naked)]
extern "sysv64" fn yield_task(_old_sp: *mut u64, _new_sp: u64) {
    core::arch::naked_asm!("push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15", "mov [rdi], rsp", "mov rsp, rsi", "pop r15", "pop r14", "pop r13", "pop r12", "pop rbp", "pop rbx", "ret");
}

fn background_task() {
    loop {
        for _ in 0..500 { unsafe { let ptr = core::ptr::addr_of_mut!(WORK_COUNTER); core::ptr::write_volatile(ptr, core::ptr::read_volatile(ptr) + 1); } }
        unsafe { yield_task(core::ptr::addr_of_mut!(THREAD_SP), core::ptr::read_volatile(core::ptr::addr_of!(MAIN_SP))); }
    }
}

unsafe fn init_thread() {
    let stack_ptr = core::ptr::addr_of_mut!(THREAD_STACK.data) as *mut u64;
    let mut sp = stack_ptr.add(1024) as u64;
    // A SysV function starts with RSP % 16 == 8 after its return address.
    sp -= 8;
    sp -= 8; *(sp as *mut u64) = background_task as *const () as u64;
    for _ in 0..6 { sp -= 8; *(sp as *mut u64) = 0; }
    core::ptr::write_volatile(core::ptr::addr_of_mut!(THREAD_SP), sp);
}

const SIN_TABLE: [isize; 36] = [0, 17, 34, 50, 64, 76, 86, 93, 98, 100, 98, 93, 86, 76, 64, 50, 34, 17, 0, -17, -34, -50, -64, -76, -86, -93, -98, -100, -98, -93, -86, -76, -64, -50, -34, -17];
#[path = "../../common/font.rs"]
mod font;
use font::FONT;

fn draw_char(fb: *mut u32, stride: usize, px: usize, py: usize, ascii: u8, color: u32) {
    let idx = if ascii >= 32 && ascii <= 95 { (ascii - 32) as usize } else if ascii >= 97 && ascii <= 122 { (ascii - 97 + 33) as usize } else { 0 };
    let bitmap = FONT[idx];
    for row in 0..8 {
        let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
        for col in 0..8 { if (row_data & (1 << (7 - col))) != 0 { unsafe { core::ptr::write_volatile(fb.add((py + row) * stride + (px + col)), color); } } }
    }
}
fn draw_string(fb: *mut u32, stride: usize, start_x: usize, start_y: usize, text: &[u8], color: u32) {
    let mut cx = start_x; for &b in text { draw_char(fb, stride, cx, start_y, b, color); cx += 8; }
}
fn usize_to_str(mut val: usize, buf: &mut [u8]) -> usize {
    if val == 0 { buf[0] = b'0'; return 1; }
    let mut idx = 0; let mut temp = [0u8; 30];
    while val > 0 && idx < 30 { temp[idx] = b'0' + (val % 10) as u8; val /= 10; idx += 1; }
    for i in 0..idx { buf[i] = temp[idx - 1 - i]; }
    idx
}

#[no_mangle] pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void { let s_u8 = s as *mut u8; for i in 0..n { core::ptr::write_volatile(s_u8.add(i), c as u8); } s }
#[no_mangle] pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void { let d_u8 = dest as *mut u8; let s_u8 = src as *const u8; for i in 0..n { core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i))); } dest }
#[no_mangle] pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 { let s1_u8 = s1 as *const u8; let s2_u8 = s2 as *const u8; for i in 0..n { let a = core::ptr::read_volatile(s1_u8.add(i)); let b = core::ptr::read_volatile(s2_u8.add(i)); if a != b { return (a as i32) - (b as i32); } } 0 }

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mb_ptr: *mut SyscallMailbox) -> () {
    unsafe { init_thread(); }
    
    os_print(mb_ptr, b"\r\n========================================\r\n");
    os_print(mb_ptr, b"[APP] STARTED. CTRL+Z: SHELL, ESC: EXIT.\r\n");
    os_print(mb_ptr, b"========================================\r\n\r\n");

    let mut frame_counter: usize = 0; 
    let cx = (info.width as isize) / 2; let cy = (info.height as isize) / 2;
    let total_pixels = info.height * info.stride;

    let mut color_theme: u32 = 0x0000FFFF; // Бирюзовый по умолчанию
    let mut last_seen_key: u8 = 0;
    for i in 0..total_pixels { unsafe { core::ptr::write_volatile(info.fb_ptr.add(i), 0x00111111); } }

    loop {
        unsafe { yield_task(core::ptr::addr_of_mut!(MAIN_SP), core::ptr::read_volatile(core::ptr::addr_of!(THREAD_SP))); }

        let os_key = get_os_key(mb_ptr);
        if os_key != 0 {
            if os_key < 0x80 { 
                last_seen_key = os_key;
                match os_key {
                    0x39 | 0x20 => color_theme = 0x00FF0000, 
                    0x1C | 0x0D => color_theme = 0x000000FF, 
                    0x22 | 0x67 => color_theme = 0x0000FF00, 
                    _ => {}
                }
                
                // ВЫХОД ПО ESC (0x01 = PS/2 клавиатура, 0x1B = MSYS2 COM-порт)
                if os_key == 0x01 || os_key == 0x1B {
                    os_print(mb_ptr, b"[SYSTEM] ESC PRESSED. EXITING APP...\r\n");
                    break; 
                }
            }
        } else if os_key >= 0x80 { last_seen_key = os_key; }

        // Erase only the square's drawing area and changing counters.
        for y in (cy - 150).max(0)..(cy + 150).min(info.height as isize) {
            for x in (cx - 150).max(0)..(cx + 150).min(info.width as isize) {
                unsafe { core::ptr::write_volatile(info.fb_ptr.add(y as usize * info.stride + x as usize), 0x00111111); }
            }
        }
        for y in 40..168.min(info.height) { for x in 40..340.min(info.width) {
            unsafe { core::ptr::write_volatile(info.fb_ptr.add(y * info.stride + x), 0x00111111); }
        } }

        let size: isize = 120;
        let t_temp = frame_counter % 36;
        let sin_a = SIN_TABLE[t_temp];
        let mut cos_angle = t_temp + 9; if cos_angle >= 36 { cos_angle -= 36; }
        let cos_a = SIN_TABLE[cos_angle];

        for y in (cy - 150)..(cy + 150) {
            for x in (cx - 150)..(cx + 150) {
                if x < 0 || x >= info.width as isize || y < 0 || y >= info.height as isize { continue; }
                let dx = x - cx; let dy = y - cy;
                let rx = (dx * cos_a - dy * sin_a) / 100; let ry = (dx * sin_a + dy * cos_a) / 100;
                
                if rx > -size && rx < size && ry > -size && ry < size {
                    unsafe { core::ptr::write_volatile(info.fb_ptr.add(y as usize * info.stride + x as usize), color_theme); }
                }
            }
        }

        draw_string(info.fb_ptr, info.stride, 40, 40, b"USERSPACE APP (ELF)", color_theme);
        draw_string(info.fb_ptr, info.stride, 40, 190, b"CTRL+Z: SHELL / ESC: EXIT", 0x00FFFFFF);

        let mut f_buf = [0u8; 30]; let f_len = usize_to_str(frame_counter, &mut f_buf);
        draw_string(info.fb_ptr, info.stride, 40, 70, b"FRAMES:", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 120, 70, &f_buf[0..f_len], 0x00FFFF00);

        let current_work = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(WORK_COUNTER)) };
        let mut w_buf = [0u8; 30]; let w_len = usize_to_str(current_work, &mut w_buf);
        draw_string(info.fb_ptr, info.stride, 40, 100, b"WORK  :", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 120, 100, &w_buf[0..w_len], 0x00FFFF00);

        let os_time = get_os_ticks(mb_ptr) / 10_000_000; 
        let mut os_buf = [0u8; 30]; let os_len = usize_to_str(os_time, &mut os_buf);
        draw_string(info.fb_ptr, info.stride, 40, 130, b"OS TICKS:", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 140, 130, &os_buf[0..os_len], 0x00FFFF00);

        let mut k_buf = [0u8; 30]; let k_len = usize_to_str(last_seen_key as usize, &mut k_buf);
        draw_string(info.fb_ptr, info.stride, 40, 160, b"LAST KEY:", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 140, 160, &k_buf[0..k_len], 0x00FFFF00);

        frame_counter = frame_counter.wrapping_add(1);
        wait::wait(mb_ptr, 30);
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
