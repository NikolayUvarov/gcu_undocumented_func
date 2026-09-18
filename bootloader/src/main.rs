#![no_std]
#![no_main]

use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::table::boot::{AllocateType, MemoryType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType};

#[path = "../../common/abi.rs"]
mod abi;
use abi::{BootInfo, ProgramImage};
mod elf_reloc;

fn keep_program(services: &uefi::table::boot::BootServices, data: &[u8]) -> ProgramImage {
    let address = services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA,
        data.len().div_ceil(4096)).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), address as *mut u8, data.len()); }
    ProgramImage { data: address as *const u8, len: data.len() }
}

#[repr(C)]
struct Elf64_Ehdr {
    e_ident: [u8; 16], e_type: u16, e_machine: u16, e_version: u32, e_entry: u64,
    e_phoff: u64, e_shoff: u64, e_flags: u32, e_ehsize: u16, e_phentsize: u16,
    e_phnum: u16, e_shentsize: u16, e_shnum: u16, e_shstrndx: u16,
}

#[repr(C)]
struct Elf64_Phdr {
    p_type: u32, p_flags: u32, p_offset: u64, p_vaddr: u64, p_paddr: u64,
    p_filesz: u64, p_memsz: u64, p_align: u64,
}

fn load_elf(boot_services: &uefi::table::boot::BootServices, file_data: &[u8]) -> u64 {
    let ehdr = unsafe { &*(file_data.as_ptr() as *const Elf64_Ehdr) };
    if ehdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] { panic!("Invalid ELF magic"); }

    let phdrs = unsafe { core::slice::from_raw_parts(file_data.as_ptr().add(ehdr.e_phoff as usize) as *const Elf64_Phdr, ehdr.e_phnum as usize) };

    let mut min_vaddr = u64::MAX;
    let mut max_vaddr = 0;

    for phdr in phdrs {
        if phdr.p_type == 1 {
            if phdr.p_vaddr < min_vaddr { min_vaddr = phdr.p_vaddr; }
            let end = phdr.p_vaddr + phdr.p_memsz;
            if end > max_vaddr { max_vaddr = end; }
        }
    }

    let total_size = max_vaddr - min_vaddr;
    let pages = ((total_size as usize) / 4096) + 1;
    let base_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages).unwrap();

    for phdr in phdrs {
        if phdr.p_type == 1 {
            let dest = (base_addr + phdr.p_vaddr - min_vaddr) as *mut u8;
            let src = unsafe { file_data.as_ptr().add(phdr.p_offset as usize) };
            unsafe {
                core::ptr::copy_nonoverlapping(src, dest, phdr.p_filesz as usize);
                if phdr.p_memsz > phdr.p_filesz {
                    core::ptr::write_bytes(dest.add(phdr.p_filesz as usize), 0, (phdr.p_memsz - phdr.p_filesz) as usize);
                }
            }
        }
    }
    let image = unsafe { core::slice::from_raw_parts_mut(base_addr as *mut u8, total_size as usize) };
    for phdr in phdrs {
        if phdr.p_type == 2 { // PT_DYNAMIC
            elf_reloc::apply(image, min_vaddr, base_addr, phdr.p_vaddr, phdr.p_filesz as usize)
                .expect("Invalid ELF relocations");
        }
    }
    base_addr + ehdr.e_entry - min_vaddr
}

#[entry]
fn main(_image: Handle, system_table: SystemTable<Boot>) -> Status {
    let (boot_info, kernel_entry) = {
        let boot_services = system_table.boot_services();

        let sfs_handle = boot_services.get_handle_for_protocol::<SimpleFileSystem>().unwrap();
        let mut sfs = boot_services.open_protocol_exclusive::<SimpleFileSystem>(sfs_handle).unwrap();
        let mut root = sfs.open_volume().unwrap();

        let file_buf_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1024).unwrap();
        let file_buf = unsafe { core::slice::from_raw_parts_mut(file_buf_addr as *mut u8, 1024 * 4096) };

        let mut name_buf = [0u16; 32];
        
        let k_name = uefi::CStr16::from_str_with_buf("kernel.elf", &mut name_buf).unwrap();
        let k_handle = root.open(k_name, FileMode::Read, FileAttribute::empty()).unwrap();
        let mut k_file = match k_handle.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
        let k_size = k_file.read(file_buf).unwrap();
        let kernel_entry = load_elf(boot_services, &file_buf[..k_size]);

        let a_name = uefi::CStr16::from_str_with_buf("app.elf", &mut name_buf).unwrap();
        let a_handle = root.open(a_name, FileMode::Read, FileAttribute::empty()).unwrap();
        let mut a_file = match a_handle.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
        let a_size = a_file.read(file_buf).unwrap();
        let app = keep_program(boot_services, &file_buf[..a_size]);

        let a2_name = uefi::CStr16::from_str_with_buf("app2.elf", &mut name_buf).unwrap();
        let a2_handle = root.open(a2_name, FileMode::Read, FileAttribute::empty()).unwrap();
        let mut a2_file = match a2_handle.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
        let a2_size = a2_file.read(file_buf).unwrap();
        let app2 = keep_program(boot_services, &file_buf[..a2_size]);

        let clock_name = uefi::CStr16::from_str_with_buf("clock.elf", &mut name_buf).unwrap();
        let clock_handle = root.open(clock_name, FileMode::Read, FileAttribute::empty()).unwrap();
        let mut clock_file = match clock_handle.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
        let clock_size = clock_file.read(file_buf).unwrap();
        let clock = keep_program(boot_services, &file_buf[..clock_size]);

        // Reserve RAM while UEFI still owns the memory map. Runtime allocations
        // (including each program's private image/stack/screen) stay in this arena.
        let heap_len = 64 * 1024 * 1024;
        let heap_ptr = boot_services.allocate_pages(AllocateType::AnyPages,
            MemoryType::LOADER_DATA, heap_len / 4096).unwrap() as *mut u8;

        let gop_handle = boot_services.get_handle_for_protocol::<GraphicsOutput>().unwrap();
        let mut gop = boot_services.open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();
        let mode = gop.current_mode_info();

        (
            BootInfo {
                fb_ptr: gop.frame_buffer().as_mut_ptr().cast(), width: mode.resolution().0,
                height: mode.resolution().1, stride: mode.stride(),
                programs: [app, app2, clock], heap_ptr, heap_len,
            },
            kernel_entry
        )
    };

    let (_system_table, _memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);
    let kernel_start: extern "sysv64" fn(&BootInfo) -> ! = unsafe { core::mem::transmute(kernel_entry as usize) };
    kernel_start(&boot_info);
}

#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
#[no_mangle] pub extern "C" fn wcslen(mut s: *const u16) -> usize { let mut len = 0; unsafe { while *s != 0 { len += 1; s = s.add(1); } } len }
