#![no_std]
#![no_main]

use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::table::boot::{AllocateType, MemoryType};

#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub app_ptr: *const u8,
}

#[entry]
fn main(_image: Handle, system_table: SystemTable<Boot>) -> Status {
    let kernel_bytes = include_bytes!("kernel.bin");
    let app_bytes = include_bytes!("app.bin");

    // Изолируем заимствование (borrow) в отдельный блок памяти (scope)
    let (boot_info, kernel_addr) = {
        let boot_services = system_table.boot_services();

        let kernel_pages = (kernel_bytes.len() / 4096) + 1;
        let kernel_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, kernel_pages).unwrap();
        unsafe { core::ptr::copy_nonoverlapping(kernel_bytes.as_ptr(), kernel_addr as *mut u8, kernel_bytes.len()); }

        let app_pages = (app_bytes.len() / 4096) + 1;
        let app_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, app_pages).unwrap();
        unsafe { core::ptr::copy_nonoverlapping(app_bytes.as_ptr(), app_addr as *mut u8, app_bytes.len()); }

        let gop_handle = boot_services.get_handle_for_protocol::<GraphicsOutput>().unwrap();
        let mut gop = boot_services.open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();
        let mode = gop.current_mode_info();
        
        // Возвращаем сырые указатели наружу. Сырым указателям плевать на Borrow Checker.
        (
            BootInfo {
                fb_ptr: gop.frame_buffer().as_mut_ptr(),
                width: mode.resolution().0,
                height: mode.resolution().1,
                stride: mode.stride(),
                app_ptr: app_addr as *const u8,
            },
            kernel_addr
        )
    }; // <--- Здесь boot_services и gop умирают, отпуская system_table

    // Теперь мы имеем право забрать владение таблицей
    let (_system_table, _memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);
    
    // Прыжок в Ядро ОС
    let kernel_entry: extern "sysv64" fn(&BootInfo) -> ! = unsafe { core::mem::transmute(kernel_addr) };
    kernel_entry(&boot_info);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// --- Фикс для линкера (заглушка стандартной библиотеки C) ---
// UEFI использует 16-битные символы (UCS-2). Линкер ищет функцию wcslen.
#[no_mangle]
pub extern "C" fn wcslen(mut s: *const u16) -> usize {
    let mut len = 0;
    unsafe {
        while *s != 0 {
            len += 1;
            s = s.add(1);
        }
    }
    len
}
