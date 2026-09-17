#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub app_ptr: *const u8,
}

const SIN_TABLE: [isize; 36] = [0, 17, 34, 50, 64, 76, 86, 93, 98, 100, 98, 93, 86, 76, 64, 50, 34, 17, 0, -17, -34, -50, -64, -76, -86, -93, -98, -100, -98, -93, -86, -76, -64, -50, -34, -17];

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    let mut time: usize = 0;
    loop {
        unsafe { core::ptr::write_bytes(info.fb_ptr, 0, info.height * info.stride * 4); }
        let cx = (info.width as isize) / 2;
        let cy = (info.height as isize) / 2;
        let size: isize = 150;

        let angle = time % 36;
        let sin_a = SIN_TABLE[angle];
        let cos_a = SIN_TABLE[(angle + 9) % 36];

        for y in (cy - 200)..(cy + 200) {
            for x in (cx - 200)..(cx + 200) {
                if x < 0 || x >= info.width as isize || y < 0 || y >= info.height as isize { continue; }
                let dx = x - cx;
                let dy = y - cy;
                let rx = (dx * cos_a - dy * sin_a) / 100;
                let ry = (dx * sin_a + dy * cos_a) / 100;
                
                if rx > -size && rx < size && ry > -size && ry < size {
                    let pixel_index = (y as usize * info.stride + x as usize) * 4;
                    unsafe {
                        info.fb_ptr.add(pixel_index).write(0);
                        info.fb_ptr.add(pixel_index + 1).write(255);
                        info.fb_ptr.add(pixel_index + 2).write(0);
                        info.fb_ptr.add(pixel_index + 3).write(255);
                    }
                }
            }
        }
        time += 1;
        for _ in 0..1_000_000 { unsafe { asm!("nop"); } }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
