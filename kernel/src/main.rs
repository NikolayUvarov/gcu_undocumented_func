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

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    let mut time: usize = 0;
    loop {
        let mut status: u8;
        let mut scancode: u8 = 0;
        unsafe {
            asm!("in al, 0x64", out("al") status);
            if (status & 1) == 1 { asm!("in al, 0x60", out("al") scancode); }
        }
        
        // 0x39 - сканкод Пробела
        if scancode == 0x39 {
            let app_entry: extern "sysv64" fn(&BootInfo) -> ! = unsafe { core::mem::transmute(info.app_ptr) };
            app_entry(info);
        }

        let wave = (time % 100) as isize;
        let pulse = if wave < 50 { wave } else { 100 - wave };
        let radius = 100 + pulse;
        
        unsafe { core::ptr::write_bytes(info.fb_ptr, 0, info.height * info.stride * 4); }

        let cx = (info.width as isize) / 2;
        let cy = (info.height as isize) / 2;

        for y in 0..(info.height as isize) {
            for x in 0..(info.width as isize) {
                let dx = x - cx;
                let dy = y - cy;
                
                if dx * dx + dy * dy <= radius * radius {
                    let pixel_index = (y as usize * info.stride + x as usize) * 4;
                    unsafe {
                        info.fb_ptr.add(pixel_index).write(255);       // B
                        info.fb_ptr.add(pixel_index + 1).write(100);   // G
                        info.fb_ptr.add(pixel_index + 2).write(pulse as u8 * 2); // R
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
