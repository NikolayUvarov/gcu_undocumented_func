#![no_std]
#![no_main]

use mind::abi::BootInfo;
use mind::font::FONT;

fn os_print(msg: &[u8]) { mind::process::log(msg) }
fn get_os_ticks() -> usize { mind::time::rdtsc() as usize }
// The character of a key event (0x1B for Esc), or 0.
fn get_os_key() -> u8 { mind::input::read_key().and_then(|k| k.char()).filter(|c| c.is_ascii()).map_or(0, |c| c as u8) }

static mut MAIN_SP: u64 = 0;
static mut THREAD_SP: u64 = 0;
static mut WORK_COUNTER: usize = 0;

#[repr(align(16))]
struct ThreadStack { data: [u64; 1024] }
static mut THREAD_STACK: ThreadStack = ThreadStack { data: [0; 1024] };

// Saves the callee-saved registers on the current stack, stores its pointer in `old_sp`, switches to `new_sp` and
// restores the registers saved there.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
extern "sysv64" fn yield_task(_old_sp: *mut u64, _new_sp: u64) {
    core::arch::naked_asm!("push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15", "mov [rdi], rsp", "mov rsp, rsi", "pop r15", "pop r14", "pop r13", "pop r12", "pop rbp", "pop rbx", "ret");
}
#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
extern "C" fn yield_task(_old_sp: *mut u64, _new_sp: u64) {
    core::arch::naked_asm!("sub sp, sp, #96", "stp x19, x20, [sp]", "stp x21, x22, [sp, #16]", "stp x23, x24, [sp, #32]",
        "stp x25, x26, [sp, #48]", "stp x27, x28, [sp, #64]", "stp x29, x30, [sp, #80]", "mov x9, sp", "str x9, [x0]", "mov sp, x1",
        "ldp x19, x20, [sp]", "ldp x21, x22, [sp, #16]", "ldp x23, x24, [sp, #32]", "ldp x25, x26, [sp, #48]",
        "ldp x27, x28, [sp, #64]", "ldp x29, x30, [sp, #80]", "add sp, sp, #96", "ret");
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
    #[cfg(target_arch = "x86_64")] {
        // A SysV function starts with RSP % 16 == 8 after its return address.
        sp -= 8;
        sp -= 8; *(sp as *mut u64) = background_task as *const () as u64;
        for _ in 0..6 { sp -= 8; *(sp as *mut u64) = 0; }
    }
    #[cfg(target_arch = "aarch64")] {
        // The frame yield_task restores: x19-x28, x29 = 0, x30 = the entry; sp stays 16-byte aligned.
        sp -= 96;
        for i in 0..12 { *((sp + 8 * i) as *mut u64) = 0; }
        *((sp + 88) as *mut u64) = background_task as *const () as u64;
    }
    core::ptr::write_volatile(core::ptr::addr_of_mut!(THREAD_SP), sp);
}

const SIN_TABLE: [isize; 36] = [0, 17, 34, 50, 64, 76, 86, 93, 98, 100, 98, 93, 86, 76, 64, 50, 34, 17, 0, -17, -34, -50, -64, -76, -86, -93, -98, -100, -98, -93, -86, -76, -64, -50, -34, -17];

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


mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("app — graphics demo: an animation on its own screen.\nUsage: app (or boot)\nCtrl+Z: back to the shell, the demo keeps running; Esc: exit.");
    unsafe { init_thread(); }
    
    os_print(b"\r\n========================================\r\n");
    os_print(b"[APP] STARTED. CTRL+Z: SHELL, ESC: EXIT.\r\n");
    os_print(b"========================================\r\n\r\n");

    let mut frame_counter: usize = 0; 
    let cx = (info.width as isize) / 2; let cy = (info.height as isize) / 2;
    let total_pixels = info.height * info.stride;

    let mut color_theme: u32 = 0x0000FFFF; // Cyan by default
    let mut last_seen_key: u8 = 0;
    for i in 0..total_pixels { unsafe { core::ptr::write_volatile(info.fb_ptr.add(i), 0x00111111); } }

    loop {
        unsafe { yield_task(core::ptr::addr_of_mut!(MAIN_SP), core::ptr::read_volatile(core::ptr::addr_of!(THREAD_SP))); }

        let os_key = get_os_key();
        if os_key != 0 {
            if os_key < 0x80 { 
                last_seen_key = os_key;
                match os_key {
                    b' ' => color_theme = 0x00FF0000,
                    b'b' => color_theme = 0x000000FF,
                    b'g' => color_theme = 0x0000FF00,
                    _ => {}
                }

                // Esc from the PS/2 keyboard or the UART
                if os_key == 0x1B {
                    os_print(b"[SYSTEM] ESC PRESSED. EXITING APP...\r\n");
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

        let os_time = get_os_ticks() / 10_000_000; 
        let mut os_buf = [0u8; 30]; let os_len = usize_to_str(os_time, &mut os_buf);
        draw_string(info.fb_ptr, info.stride, 40, 130, b"OS TICKS:", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 140, 130, &os_buf[0..os_len], 0x00FFFF00);

        let mut k_buf = [0u8; 30]; let k_len = usize_to_str(last_seen_key as usize, &mut k_buf);
        draw_string(info.fb_ptr, info.stride, 40, 160, b"LAST KEY:", 0x00FFFFFF); draw_string(info.fb_ptr, info.stride, 140, 160, &k_buf[0..k_len], 0x00FFFF00);

        frame_counter = frame_counter.wrapping_add(1);
        mind::time::sleep(30);
    }
}
