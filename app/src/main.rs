#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;
use core::ffi::c_void;

#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub app_ptr: *const u8,
}

const SIN_TABLE: [isize; 36] = [
    0, 17, 34, 50, 64, 76, 86, 93, 98, 100, 98, 93, 86, 76, 64, 50, 34, 17, 
    0, -17, -34, -50, -64, -76, -86, -93, -98, -100, -98, -93, -86, -76, -64, -50, -34, -17
];

const FONT: [u64; 64] = [
    0x0000000000000000, 0x1818181818001800, 0x6C6C000000000000, 0x36367F367F363600,
    0x183E603C067C1800, 0x60660C1830660600, 0x386C6C386CA6CC78, 0x1818300000000000,
    0x0C18303030180C00, 0x30180C0C0C183000, 0x00663CFF3C660000, 0x0018187E18180000,
    0x0000000000181830, 0x0000007E00000000, 0x0000000000181800, 0x060C183060C08000,
    0x3C666E7666663C00, 0x1838181818187E00, 0x3C66061C30607E00, 0x3C66061C06663C00,
    0x1C3C6CccFE0C0C00, 0x7E607C0606663C00, 0x3C607C6666663C00, 0x7E060C1830303000,
    0x3C66663C66663C00, 0x3C66663E06063C00, 0x0018180000181800, 0x0018180000181830,
    0x060C1830180C0600, 0x00007E007E000000, 0x30180C060C183000, 0x3C66060C18001800,
    0x3C666E6E60663C00, 0x183C66667E666600, 0x7C66667C66667C00, 0x3C66606060663C00,
    0x786C6666666C7800, 0x7E60607C60607E00, 0x7E60607C60606000, 0x3C66606E66663E00,
    0x6666667E66666600, 0x3E18181818183E00, 0x0606060606663C00, 0x666C7870786C6600,
    0x6060606060607E00, 0xC6EEDBc6c6c6c600, 0x66767E7E6E666600, 0x3C66666666663C00,
    0x7C66667C60606000, 0x3C6666666E3C0200, 0x7C66667C6C666600, 0x3C66603C06663C00,
    0x7E18181818181800, 0x6666666666663C00, 0x66666666663C1800, 0xC6C6C6D6FEEEC600,
    0x66663C183C666600, 0x6666663C18181800, 0x7E060C1830607E00, 0x3C30303030303C00,
    0x6030180C06030100, 0x3C0C0C0C0C0C3C00, 0x183C660000000000, 0x00000000000000FF 
];

fn draw_char(fb: *mut u32, stride: usize, px: usize, py: usize, ascii: u8, color: u32) {
    let idx = if ascii >= 32 && ascii <= 95 { (ascii - 32) as usize } 
              else if ascii >= 97 && ascii <= 122 { (ascii - 97 + 33) as usize } 
              else { 0 };
    let bitmap = FONT[idx];
    for row in 0..8 {
        let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
        for col in 0..8 {
            if (row_data & (1 << (7 - col))) != 0 {
                unsafe { core::ptr::write_volatile(fb.add((py + row) * stride + (px + col)), color); }
            }
        }
    }
}

fn draw_string(fb: *mut u32, stride: usize, start_x: usize, start_y: usize, text: &[u8], color: u32) {
    let mut cx = start_x;
    for &b in text {
        draw_char(fb, stride, cx, start_y, b, color);
        cx += 8;
    }
}

fn usize_to_str(mut val: usize, buf: &mut [u8]) -> usize {
    if val == 0 { buf[0] = b'0'; return 1; }
    let mut idx = 0;
    let mut temp = [0u8; 20];
    while val > 0 && idx < 20 {
        temp[idx] = b'0' + (val % 10) as u8;
        val /= 10;
        idx += 1;
    }
    for i in 0..idx { buf[i] = temp[idx - 1 - i]; }
    idx
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let s_u8 = s as *mut u8;
    for i in 0..n { core::ptr::write_volatile(s_u8.add(i), c as u8); }
    s
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d_u8 = dest as *mut u8;
    let s_u8 = src as *const u8;
    for i in 0..n { core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i))); }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let s1_u8 = s1 as *const u8;
    let s2_u8 = s2 as *const u8;
    for i in 0..n {
        let a = core::ptr::read_volatile(s1_u8.add(i));
        let b = core::ptr::read_volatile(s2_u8.add(i));
        if a != b { return (a as i32) - (b as i32); }
    }
    0
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    let mut frame: usize = 0;
    let cx = (info.width as isize) / 2;
    let cy = (info.height as isize) / 2; // Исправлена опечатка переменной 'height'
    let size: isize = 120;
    let fb_u32 = info.fb_ptr as *mut u32; // Приводим указатель к 32-битному
    let total_pixels = info.height * info.stride;

    loop {
        // Очистка экрана
        for i in 0..total_pixels {
            unsafe { core::ptr::write_volatile(fb_u32.add(i), 0x001E1E2E); }
        }

        let mut t_temp = frame / 4; // Плавное вращение (1 сдвиг на 4 кадра)
        while t_temp >= 36 { t_temp -= 36; }
        let angle = t_temp;

        let sin_a = SIN_TABLE[angle];
        let mut cos_angle = angle + 9;
        if cos_angle >= 36 { cos_angle -= 36; }
        let cos_a = SIN_TABLE[cos_angle];

        // Рендер квадрата
        for y in (cy - 150)..(cy + 150) {
            for x in (cx - 150)..(cx + 150) {
                if x < 0 || x >= info.width as isize || y < 0 || y >= info.height as isize { continue; }
                let dx = x - cx;
                let dy = y - cy;
                
                let rx = (dx * cos_a - dy * sin_a) / 100;
                let ry = (dx * sin_a + dy * cos_a) / 100;
                
                if rx > -size && rx < size && ry > -size && ry < size {
                    let offset = y as usize * info.stride + x as usize;
                    unsafe { core::ptr::write_volatile(fb_u32.add(offset), 0x00A6E3A1); }
                }
            }
        }

        // Вывод текста и счетчиков
        draw_string(fb_u32, info.stride, 40, 40, b"USERSPACE APP RUNNING", 0x00FFFFFF);
        
        let mut num_buf = [0u8; 20];
        let len = usize_to_str(frame, &mut num_buf);
        draw_string(fb_u32, info.stride, 40, 70, b"FRAMES:", 0x00A6E3A1);
        draw_string(fb_u32, info.stride, 100, 70, &num_buf[0..len], 0x00F9E2AF);

        frame += 1;
        
        // Надежная пауза (чтобы не вращалось мгновенно)
        for _ in 0..10_000_000 { unsafe { asm!("nop"); } }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
